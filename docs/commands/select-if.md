<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# SELECT IF

Keep only the cases where a condition holds.

## Syntax

```text
SELECT IF condition.
```

## Example

```text
SELECT IF age >= 18 AND NOT MISSING(q1).
```

`statscraft help SELECT IF` prints the same text in the terminal. [All commands](README.md)
