# Monkey-patched attributes

The `[tool.ty.analysis.monkey-patched-attributes]` table declares the type of attributes that exist
on a class at runtime but are not visible to static analysis (e.g. attributes installed by a
framework or added via an explicit monkey patch). Each key has the form
`module.path.Class.attribute`; the last segment is the attribute name and the prefix is the dotted
path to the class. The value is a type expression: usually a dotted path to a class (the resolved
type is its *instance* type), but it may also be a `Callable[[...], ...]` — see
[below](#the-value-may-be-a-callable-type-expression).

## Adds an attribute that does not exist on the class

```toml
[analysis.monkey-patched-attributes]
"mypkg.Foo.patched" = "builtins.int"
```

`mypkg/__init__.py`:

```py
class Foo:
    pass
```

```py
from mypkg import Foo

foo = Foo()
reveal_type(foo.patched)  # revealed: int
```

## Overrides an attribute that exists on the class

```toml
[analysis.monkey-patched-attributes]
"mypkg.Request.user" = "mypkg.User"
```

`mypkg/__init__.py`:

```py
class User:
    pass

class Request:
    user: int  # Overridden by the monkey-patched-attributes entry below
```

```py
from mypkg import Request

req = Request()
reveal_type(req.user)  # revealed: User
```

## Subclasses inherit the configured type

```toml
[analysis.monkey-patched-attributes]
"mypkg.Base.extra" = "builtins.str"
```

`mypkg/__init__.py`:

```py
class Base:
    pass

class Child(Base):
    pass
```

```py
from mypkg import Child

c = Child()
reveal_type(c.extra)  # revealed: str
```

## Bare class name resolves against `builtins`

```toml
[analysis.monkey-patched-attributes]
"int.is_super_cool" = "builtins.bool"
```

```py
n = 1
reveal_type(n.is_super_cool)  # revealed: bool
```

## Does not apply to unrelated classes

```toml
[analysis.monkey-patched-attributes]
"mypkg.Foo.patched" = "builtins.int"
```

`mypkg/__init__.py`:

```py
class Foo:
    pass

class Bar:
    pass
```

```py
from mypkg import Bar

b = Bar()
# error: [unresolved-attribute]
reveal_type(b.patched)  # revealed: Unknown
```

## Multiple entries are independent

```toml
[analysis.monkey-patched-attributes]
"mypkg.Foo.x" = "builtins.int"
"mypkg.Bar.x" = "builtins.str"
"mypkg.Foo.y" = "builtins.bytes"
```

`mypkg/__init__.py`:

```py
class Foo:
    pass

class Bar:
    pass
```

```py
from mypkg import Foo, Bar

foo = Foo()
reveal_type(foo.x)  # revealed: int
reveal_type(foo.y)  # revealed: bytes

bar = Bar()
reveal_type(bar.x)  # revealed: str
```

## Bare type name on the right-hand side resolves against `builtins`

```toml
[analysis.monkey-patched-attributes]
"mypkg.Foo.count" = "int"
```

`mypkg/__init__.py`:

```py
class Foo:
    pass
```

```py
from mypkg import Foo

foo = Foo()
reveal_type(foo.count)  # revealed: int
```

## Assignment to a configured attribute is accepted

The monkey-patched-attributes entry also applies to writes — `obj.attr = value` is accepted as long
as `value` is assignable to the configured type. No `unresolved-attribute` diagnostic is emitted
even when the class does not declare the attribute.

```toml
[analysis.monkey-patched-attributes]
"mypkg.Request.is_internal_host" = "builtins.bool"
```

`mypkg/__init__.py`:

```py
class Request:
    pass
```

```py
from mypkg import Request

req = Request()
req.is_internal_host = True
reveal_type(req.is_internal_host)  # revealed: bool
```

## Assignment of an incompatible value is reported

```toml
[analysis.monkey-patched-attributes]
"mypkg.Request.is_internal_host" = "builtins.bool"
```

`mypkg/__init__.py`:

```py
class Request:
    pass
```

```py
from mypkg import Request

req = Request()
req.is_internal_host = "not a bool"  # error: [invalid-assignment]
```

## Applies to the class object itself, not just instances

The mapping also matches when the receiver is the class object (`type[Class]`), so a monkey patch
written directly on the class — the common way to actually install one at runtime — is accepted both
for reads and for writes.

```toml
[analysis.monkey-patched-attributes]
"mypkg.AnonymousUser.is_work_leader" = "builtins.bool"
```

`mypkg/__init__.py`:

```py
class AnonymousUser:
    pass
```

```py
from mypkg import AnonymousUser

# Write on the class object is accepted (no `unresolved-attribute`)...
AnonymousUser.is_work_leader = True
# ...and an incompatible value is still reported.
AnonymousUser.is_work_leader = "nope"  # error: [invalid-assignment]

# Reads on both the class object and an instance see the configured type.
reveal_type(AnonymousUser.is_work_leader)  # revealed: bool
reveal_type(AnonymousUser().is_work_leader)  # revealed: bool
```

## Subclass objects inherit the configured type

```toml
[analysis.monkey-patched-attributes]
"mypkg.Base.extra" = "builtins.str"
```

`mypkg/__init__.py`:

```py
class Base:
    pass

class Child(Base):
    pass
```

```py
from mypkg import Child

reveal_type(Child.extra)  # revealed: str
```

## The value may be a `Callable` type expression

Besides a dotted path to a class, the value may be a type expression. The common case is a
`Callable[[args], return]` (useful for methods that are monkey-patched in), spelled as
`typing.Callable`, `collections.abc.Callable`, or a bare `Callable`. Argument and return types are
themselves resolved recursively.

```toml
[analysis.monkey-patched-attributes]
"mypkg.AnonymousUser.has_group" = "typing.Callable[[str], bool]"
"mypkg.AnonymousUser.is_leader" = "Callable[[], bool]"
"mypkg.AnonymousUser.describe" = "Callable[..., str]"
```

`mypkg/__init__.py`:

```py
class AnonymousUser:
    pass
```

```py
from mypkg import AnonymousUser

u = AnonymousUser()
reveal_type(u.has_group)  # revealed: (str, /) -> bool
reveal_type(u.has_group("admin"))  # revealed: bool
reveal_type(u.is_leader())  # revealed: bool

# `Callable[..., str]` accepts any arguments.
reveal_type(u.describe(1, 2, x=3))  # revealed: str

u.has_group(123)  # error: [invalid-argument-type]
```

## Assigning a method onto the class object is accepted

When the configured type is a callable, assigning a function/lambda onto the *class object* installs
a method: accessed on an instance, its first parameter (`self`) is bound. So the assignment is
accepted as long as the value's method-bound form (with `self` stripped) matches the configured
instance-side signature.

```toml
[analysis.monkey-patched-attributes]
"mypkg.AnonymousUser.is_team_lead" = "Callable[[], bool]"
"mypkg.AnonymousUser.has_group" = "Callable[[str], bool]"
```

`mypkg/__init__.py`:

```py
class AnonymousUser:
    pass
```

```py
from mypkg import AnonymousUser

# The lambdas carry `self`; bound as methods they match the configured types.
AnonymousUser.is_team_lead = lambda self: False
AnonymousUser.has_group = lambda self, group: False

# A value whose bound form is incompatible is still reported.
AnonymousUser.is_team_lead = lambda self, extra: False  # error: [invalid-assignment]
# A non-callable value is still reported.
AnonymousUser.is_team_lead = "nope"  # error: [invalid-assignment]

reveal_type(AnonymousUser().is_team_lead())  # revealed: bool
reveal_type(AnonymousUser().has_group("admin"))  # revealed: bool
```

## `None` in a type expression denotes `NoneType`

```toml
[analysis.monkey-patched-attributes]
"mypkg.Foo.callback" = "Callable[[int], None]"
```

`mypkg/__init__.py`:

```py
class Foo:
    pass
```

```py
from mypkg import Foo

reveal_type(Foo().callback(1))  # revealed: None
```
