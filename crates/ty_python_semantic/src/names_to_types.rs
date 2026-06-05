//! Support for the `names-to-types` analysis option: a project-wide mapping
//! that declares the implicit type of names.
//!
//! Configured in `pyproject.toml` (or `ty.toml`) under
//! `[tool.ty.analysis.names-to-types]`, as a table whose keys are Python
//! identifiers and whose values are dotted paths to a class. The mapping is
//! attached to [`crate::AnalysisSettings`] and accessed per-file via
//! [`crate::Db::analysis_settings`].

use rustc_hash::FxHashMap;

use ruff_db::files::File;
use ty_module_resolver::{ModuleName, resolve_module};

use crate::Db;
use crate::place::{builtins_symbol, imported_symbol};
use crate::types::Type;

/// A set of `name -> dotted-type` entries that apply to a project.
#[derive(Debug, Default, Clone, PartialEq, Eq, get_size2::GetSize)]
pub struct NamesToTypesMap {
    entries: FxHashMap<String, String>,
}

impl NamesToTypesMap {
    pub fn from_entries<I, K, V>(iter: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        Self {
            entries: iter
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn get(&self, name: &str) -> Option<&str> {
        self.entries.get(name).map(String::as_str)
    }
}

/// Resolve a `name` to a `Type` using the names-to-types mapping that applies
/// to `file`, if any. Returns `None` if there is no entry for `name`, or if the
/// dotted path in the mapping cannot be resolved to a class.
pub(crate) fn resolve_implicit_declared_type<'db>(
    db: &'db dyn Db,
    file: File,
    name: &str,
) -> Option<Type<'db>> {
    let type_path = db.analysis_settings(file).names_to_types.get(name)?;
    // Cloning is necessary because the borrow from `db` is invalidated by the
    // subsequent `resolve_module` / `imported_symbol` calls (which also touch
    // the db).
    let type_path = type_path.to_owned();
    resolve_dotted_type(db, file, &type_path)
}

fn resolve_dotted_type<'db>(db: &'db dyn Db, file: File, dotted: &str) -> Option<Type<'db>> {
    let (module_part, symbol_name) = match dotted.rsplit_once('.') {
        Some((module, symbol)) => (Some(module), symbol),
        None => (None, dotted),
    };

    let class_type = if let Some(module_part) = module_part {
        let module_name = ModuleName::new(module_part)?;
        let module = resolve_module(db, file, &module_name)?;
        let module_file = module.file(db)?;
        imported_symbol(db, Some(module_file), symbol_name, None)
            .place
            .ignore_possibly_undefined()?
    } else {
        // Bare name: fall back to the `builtins` namespace.
        builtins_symbol(db, symbol_name)
            .place
            .ignore_possibly_undefined()?
    };

    class_type.to_instance(db)
}
