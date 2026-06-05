# Names-to-types mapping

The `[tool.ty.analysis.names-to-types]` table in `pyproject.toml` (or `ty.toml`) declares the
implicit type of names for all source files in the project. Each entry has the form
`name = "fully.qualified.Type"`. An entry behaves as if the user had written the equivalent
annotation `name: Type = ...` on the assignment.

## Basic mapping

```toml
[analysis.names-to-types]
foo = "builtins.int"
```

```py
foo = 1
reveal_type(foo)  # revealed: int

foo = "not an int"  # error: [invalid-assignment]
```

## Mapping uses a user-defined class

```toml
[analysis.names-to-types]
client = "mypkg.client.Client"
```

`mypkg/__init__.py`:

```py
```

`mypkg/client.py`:

```py
class Client:
    pass
```

```py
from mypkg.client import Client

client = Client()
reveal_type(client)  # revealed: Client

client = 1  # error: [invalid-assignment]
```

## Multiple names

```toml
[analysis.names-to-types]
foo = "builtins.int"
bar = "builtins.str"
```

```py
foo = 1
reveal_type(foo)  # revealed: int

bar = "hello"
reveal_type(bar)  # revealed: str

bar = 1  # error: [invalid-assignment]
```

## Explicit annotation in code overrides the mapping

If the source code provides its own annotation for a name, that annotation "wins" over the
`names-to-types` mapping — even when the mapping would have produced a different type.

```toml
[analysis.names-to-types]
foo = "builtins.int"
```

```py
# The explicit annotation makes `foo: str`, even though the mapping says `int`.
foo: str = "hello"

# Subsequent reassignments are checked against `str` (the explicit annotation),
# not against `int` from the mapping.
foo = "still a string"
foo = 1  # error: [invalid-assignment]
```

## Bare name resolves against `builtins`

```toml
[analysis.names-to-types]
n = "int"
```

```py
n = 0
reveal_type(n)  # revealed: int

n = "oops"  # error: [invalid-assignment]
```

## Unannotated parameters use the mapping

An unannotated parameter `def x(foo):` is interpreted as `def x(foo: Foo)` if the mapping has
`foo = "Foo"`. The implicit annotation is visible both inside the function body and at every
call site.

```toml
[analysis.names-to-types]
count = "builtins.int"
```

```py
def take(count):
    reveal_type(count)  # revealed: int

take(5)
take("oops")  # error: [invalid-argument-type]
```

## Explicit parameter annotation overrides the mapping

```toml
[analysis.names-to-types]
count = "builtins.int"
```

```py
def take(count: str):
    reveal_type(count)  # revealed: str

take("ok")
take(5)  # error: [invalid-argument-type]
```

## Lambda parameters also use the mapping

```toml
[analysis.names-to-types]
count = "builtins.int"
```

```py
f = lambda count: count
reveal_type(f(5))  # revealed: int
```

## Attribute access on a dynamic receiver uses the mapping

When the receiver of an attribute access has an unknown/dynamic type, the attribute's type is
normally `Unknown` too. If the attribute name appears in the mapping, we use the mapped type
instead.

```toml
[analysis.names-to-types]
user = "builtins.str"
```

```py
def handle(request):
    reveal_type(request.user)  # revealed: str
```

## Attribute access overrides a known attribute type

When the attribute name appears in the mapping, the mapped type overrides whatever the type checker
would otherwise infer for the attribute. This is useful when a framework's stub declares a broader
type than what a project actually uses (e.g., Django's `request.user` is typed as
`AbstractBaseUser | AnonymousUser`, but a project may always use a custom `User` subclass).

```toml
[analysis.names-to-types]
user = "builtins.bytes"
```

```py
class Request:
    user: int

req: Request = Request()
# The mapping wins over the declared attribute type.
reveal_type(req.user)  # revealed: bytes
```

## Variadic parameters are not affected by the mapping

The mapping is intentionally not applied to `*args` / `**kwargs`, since the parameter name there
refers to a tuple / dict rather than a single value.

```toml
[analysis.names-to-types]
args = "builtins.int"
kwargs = "builtins.int"
```

```py
def take(*args, **kwargs):
    reveal_type(args)  # revealed: tuple[Unknown, ...]
    reveal_type(kwargs)  # revealed: dict[str, Unknown]

take(1, 2, key="value")
```
