<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# IF

Assign a value only for the cases where a condition holds.

## Syntax

```text
IF (condition) target = expression.
```

The parentheses around the condition are optional.

## Example

```text
IF (age >= 65 AND region = 2) senior_south = 1.
```

`statscraft help IF` prints the same text in the terminal. [All commands](README.md)
