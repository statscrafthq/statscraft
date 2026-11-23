<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# COMPUTE

Assign the value of an expression to a variable.

## Syntax

```text
COMPUTE target = expression.
```

The command sets target to the value of the expression for every case. A new target is numeric with format F8.2; an existing one keeps its format and labels. A string target must exist first (STRING), and a longer text is cut to its width.

When a value the expression needs is missing (system-missing, or a user-missing value of the variable), the result is system-missing, except in functions that skip missing values, such as MEAN, SUM and NVALID, and in MISSING, SYSMIS and VALUE. Operators and functions are listed under the general rules on the command index and in statscraft help.

COMPUTE is a transformation: it runs when the next procedure or EXECUTE reads the data.

## Example

```text
COMPUTE bmi = weight / (height / 100) ** 2.
```

`statscraft help COMPUTE` prints the same text in the terminal. [All commands](README.md)
