<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# IF

Assign a value only for the cases where a condition holds.

## Syntax

```text
IF (condition) target = expression.
```

The parentheses around the condition are optional. The command sets target only for the cases where the condition is true; where it is false or missing the target keeps its value. A new target starts as system-missing (numeric, F8.2) for every case, so cases the condition leaves out stay missing. A string target must exist first (STRING). Several IF commands in a row are checked one after another, so a later one may overwrite an earlier one.

## Example

```text
IF (age >= 65 AND region = 2) senior_south = 1.
```

`statscraft help IF` prints the same text in the terminal. [All commands](README.md)
