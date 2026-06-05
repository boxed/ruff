//! Support for the `monkey-patched-attributes` analysis option: a per-class
//! mapping that declares the type of attributes that were added to a class at
//! runtime (e.g. by a third-party framework or a project-level monkey patch).
//!
//! Configured in `pyproject.toml` (or `ty.toml`) under
//! `[tool.ty.analysis.monkey-patched-attributes]`, as a table whose keys are
//! dotted paths of the form `module.Class.attr` and whose values are type
//! expressions for the attribute — a dotted path to a class (its instance
//! type is used) or a `Callable[[...], ...]`. The mapping is attached to
//! [`crate::AnalysisSettings`] and consulted by `infer_attribute_load` when
//! the receiver is an instance of the named class (or a subclass), or the
//! class object itself.

use rustc_hash::FxHashMap;

use ruff_db::files::File;
use ruff_python_ast as ast;
use ruff_python_parser::parse_expression;
use ty_module_resolver::{ModuleName, resolve_module, resolve_module_confident};

use crate::Db;
use crate::place::{builtins_symbol, imported_symbol};
use crate::types::{Parameter, Parameters, Signature, Type};

/// A parsed `monkey-patched-attributes` mapping, indexed by attribute name for
/// O(1) lookup. The per-attribute list preserves the configuration order from
/// TOML so the first matching class wins deterministically.
#[derive(Debug, Default, Clone, PartialEq, Eq, get_size2::GetSize)]
pub struct MonkeyPatchedAttributesMap {
    by_attr: FxHashMap<String, Vec<MonkeyPatchEntry>>,
}

#[derive(Debug, Clone, PartialEq, Eq, get_size2::GetSize)]
struct MonkeyPatchEntry {
    /// Dotted path to the class on which the attribute lives (e.g.
    /// `django.http.HttpRequest` or, for builtins, `int`).
    class_path: String,
    /// Dotted path to the attribute's type (e.g. `mypkg.User` or `int`).
    type_path: String,
}

impl MonkeyPatchedAttributesMap {
    /// Build a map from an iterator of `(dotted_key, type_path)` pairs. Each
    /// dotted key has the form `module.Class.attr`; the last segment becomes
    /// the attribute name and the prefix becomes the class path. Entries with
    /// fewer than two segments are silently skipped because they cannot
    /// identify a class.
    pub fn from_entries<I, K, V>(iter: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: Into<String>,
    {
        let mut by_attr: FxHashMap<String, Vec<MonkeyPatchEntry>> = FxHashMap::default();
        for (key, type_path) in iter {
            let key = key.as_ref();
            let Some((class_path, attr_name)) = key.rsplit_once('.') else {
                continue;
            };
            if class_path.is_empty() || attr_name.is_empty() {
                continue;
            }
            by_attr
                .entry(attr_name.to_owned())
                .or_default()
                .push(MonkeyPatchEntry {
                    class_path: class_path.to_owned(),
                    type_path: type_path.into(),
                });
        }
        Self { by_attr }
    }

    pub fn is_empty(&self) -> bool {
        self.by_attr.is_empty()
    }
}

/// If a `monkey-patched-attributes` entry applies to `receiver` for the given
/// `attr_name`, resolve it to the configured `Type`. Returns `None` if no
/// entry matches (either no class match, or the configured type path cannot
/// be resolved).
pub(crate) fn resolve_monkey_patched_attribute<'db>(
    db: &'db dyn Db,
    file: File,
    receiver: Type<'db>,
    attr_name: &str,
) -> Option<Type<'db>> {
    // Hold the borrow on `db.analysis_settings(file)` only long enough to
    // clone the candidate entries. Further db calls invalidate that borrow.
    let candidates: Vec<(String, String)> = {
        let entries = db
            .analysis_settings(file)
            .monkey_patched_attributes
            .by_attr
            .get(attr_name)?;
        entries
            .iter()
            .map(|e| (e.class_path.clone(), e.type_path.clone()))
            .collect()
    };

    for (class_path, type_path) in candidates {
        let Some(class_instance) = resolve_dotted_instance(db, Modules::File(file), &class_path)
        else {
            continue;
        };
        // Match when the receiver is an instance of the configured class (or a
        // subclass) *or* when it is the class object itself (`type[Class]`), so
        // that a monkey patch written directly on the class — e.g.
        // `AnonymousUser.is_work_leader = lambda self: False` — is also accepted.
        let class_object = class_instance.to_meta_type(db);
        if receiver.is_assignable_to(db, class_instance)
            || receiver.is_assignable_to(db, class_object)
        {
            return resolve_config_type(db, Modules::File(file), &type_path);
        }
    }

    None
}

/// How dotted names are resolved to modules.
#[derive(Clone, Copy)]
enum Modules {
    /// Resolve relative to an importing file, allowing the "desperate"
    /// fallback (used while type-checking that file).
    File(File),
    /// Resolve without an importing file or desperate fallback (used to
    /// validate config entries up front, before any specific file is checked).
    Confident,
}

/// Resolve a config entry's *type path* (the value of a
/// `monkey-patched-attributes` entry) to a [`Type`]. The value is parsed as a
/// type expression, so besides a plain dotted path it may be a
/// `Callable[[ArgType, ...], ReturnType]` (or `Callable[..., ReturnType]`),
/// with `typing.Callable` / `collections.abc.Callable` spellings accepted.
/// Argument and return types are themselves type expressions, resolved
/// recursively. Returns `None` if the expression uses an unsupported construct
/// or any referenced name cannot be resolved.
fn resolve_config_type<'db>(db: &'db dyn Db, modules: Modules, value: &str) -> Option<Type<'db>> {
    let parsed = parse_expression(value).ok()?;
    eval_type_expression(db, modules, parsed.expr())
}

fn eval_type_expression<'db>(
    db: &'db dyn Db,
    modules: Modules,
    expr: &ast::Expr,
) -> Option<Type<'db>> {
    match expr {
        // `None` denotes `NoneType` in a type expression.
        ast::Expr::NoneLiteral(_) => Some(Type::none(db)),
        ast::Expr::Name(_) | ast::Expr::Attribute(_) => {
            resolve_dotted_instance(db, modules, &dotted_name(expr)?)
        }
        ast::Expr::Subscript(subscript) if is_callable_ref(&subscript.value) => {
            eval_callable(db, modules, subscript)
        }
        _ => None,
    }
}

/// Build a `Callable` type from a `Callable[<params>, <return>]` subscript.
fn eval_callable<'db>(
    db: &'db dyn Db,
    modules: Modules,
    subscript: &ast::ExprSubscript,
) -> Option<Type<'db>> {
    // `Callable` always takes exactly two arguments: a parameter list and a
    // return type.
    let ast::Expr::Tuple(arguments) = &*subscript.slice else {
        return None;
    };
    let [params_expr, return_expr] = &arguments.elts[..] else {
        return None;
    };

    let parameters = match params_expr {
        // `Callable[..., R]`: the parameter list is gradual (unknown).
        ast::Expr::EllipsisLiteral(_) => Parameters::unknown(),
        // `Callable[[A, B], R]`: a concrete, positional-only parameter list.
        ast::Expr::List(list) => {
            let mut params = Vec::with_capacity(list.elts.len());
            for elt in &list.elts {
                let param_ty = eval_type_expression(db, modules, elt)?;
                params.push(Parameter::positional_only(None).with_annotated_type(param_ty));
            }
            Parameters::new(db, params)
        }
        _ => return None,
    };

    let return_ty = eval_type_expression(db, modules, return_expr)?;
    Some(Type::single_callable(
        db,
        Signature::new(parameters, return_ty),
    ))
}

/// Whether `expr` syntactically refers to `typing.Callable` (or the
/// `collections.abc` / `typing_extensions` spellings, or a bare `Callable`).
fn is_callable_ref(expr: &ast::Expr) -> bool {
    matches!(
        dotted_name(expr).as_deref(),
        Some(
            "Callable"
                | "typing.Callable"
                | "typing_extensions.Callable"
                | "collections.abc.Callable"
        )
    )
}

/// Reconstruct the dotted name from a `Name`/`Attribute` chain (e.g. `a.b.C`),
/// or `None` if the expression is not a simple dotted name.
fn dotted_name(expr: &ast::Expr) -> Option<String> {
    match expr {
        ast::Expr::Name(name) => Some(name.id.to_string()),
        ast::Expr::Attribute(attribute) => {
            let mut base = dotted_name(&attribute.value)?;
            base.push('.');
            base.push_str(attribute.attr.as_str());
            Some(base)
        }
        _ => None,
    }
}

fn resolve_dotted_instance<'db>(
    db: &'db dyn Db,
    modules: Modules,
    dotted: &str,
) -> Option<Type<'db>> {
    let (module_part, symbol_name) = split_dotted(dotted);

    let class_type = if let Some(module_part) = module_part {
        let module_name = ModuleName::new(module_part)?;
        let module = match modules {
            Modules::File(file) => resolve_module(db, file, &module_name)?,
            Modules::Confident => resolve_module_confident(db, &module_name)?,
        };
        let module_file = module.file(db)?;
        imported_symbol(db, Some(module_file), symbol_name, None)
            .place
            .ignore_possibly_undefined()?
    } else {
        builtins_symbol(db, symbol_name)
            .place
            .ignore_possibly_undefined()?
    };

    class_type.to_instance(db)
}

/// Whether a dotted *class path* (the key prefix of a `monkey-patched-attributes`
/// entry) resolves, without a file-specific "desperate" fallback. Used to
/// validate config entries up front.
pub fn dotted_path_resolves(db: &dyn Db, dotted: &str) -> bool {
    resolve_dotted_instance(db, Modules::Confident, dotted).is_some()
}

/// Whether a config entry's *type path* (a type expression such as a dotted
/// path or `Callable[[str], bool]`) resolves, without a file-specific
/// "desperate" fallback. Used to validate config entries up front.
pub fn config_type_resolves(db: &dyn Db, value: &str) -> bool {
    resolve_config_type(db, Modules::Confident, value).is_some()
}

fn split_dotted(dotted: &str) -> (Option<&str>, &str) {
    match dotted.rsplit_once('.') {
        Some((module, symbol)) => (Some(module), symbol),
        None => (None, dotted),
    }
}
