# Test cases for skip-line-joining option
# This option preserves line breaks in comma-separated constructs

# Function definition with arguments on separate lines (no trailing comma)
def test(
    a,
    b
):
    pass


# Function definition on single line stays single line
def single_line(a, b):
    pass


# Function call with arguments on separate lines
result = func(
    arg1,
    arg2
)


# Function call on single line stays single line
result2 = func(arg1, arg2)


# List on multiple lines
items = [
    1,
    2,
    3
]


# List on single line stays single line
items2 = [1, 2, 3]


# Dict on multiple lines
data = {
    "a": 1,
    "b": 2
}


# Dict on single line stays single line
data2 = {"a": 1, "b": 2}


# Tuple on multiple lines
coords = (
    x,
    y
)


# Tuple on single line stays single line
coords2 = (x, y)


# Import on multiple lines
from module import (
    a,
    b
)


# Import on single line stays single line
from module import a, b


# Nested structure - inner list is on multiple lines, should stay that way
nested = {
    "key": [
        1,
        2
    ],
}


# List comprehension on multiple lines
list_comp = [
    x
    for x in range(10)
]


# List comprehension on single line stays single line
list_comp2 = [x for x in range(10)]


# Set comprehension on multiple lines
set_comp = {
    x
    for x in range(10)
}


# Set comprehension on single line stays single line
set_comp2 = {x for x in range(10)}


# Dict comprehension on multiple lines
dict_comp = {
    x: x * 2
    for x in range(10)
}


# Dict comprehension on single line stays single line
dict_comp2 = {x: x * 2 for x in range(10)}


# Parenthesized boolean expression on multiple lines
def __str__(self):
    return (
        self.get_short_name()
        or self.name
        or (self.location and str(self.location))
        or (self.customer and str(self.customer))
        or str(self.pk)
    )


# Parenthesized boolean expression on single line stays single line
def short_str(self):
    return self.name or self.pk


# Parenthesized boolean expression with 'and' on multiple lines
def is_valid(self):
    return (
        self.name
        and self.email
        and self.age > 18
    )


# Binary expression on multiple lines
result = (
    first_value
    + second_value
    + third_value
)
