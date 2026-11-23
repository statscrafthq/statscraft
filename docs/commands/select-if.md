<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# SELECT IF

Keep only the cases where a condition holds.

## Syntax

```text
SELECT IF condition.
```

The command keeps the cases where the condition is true and drops the others, also those where it is missing. The cases are gone for every later command; after TEMPORARY the selection holds only for the next procedure. FILTER hides cases without deleting them. Like other transformations, it runs when the next procedure or EXECUTE reads the data.

## Example

```text
SELECT IF age >= 18 AND NOT MISSING(q1).
```

`statscraft help SELECT IF` prints the same text in the terminal. [All commands](README.md)
