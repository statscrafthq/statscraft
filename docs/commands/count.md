<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# COUNT

Count, per case, how many variables hold one of the listed values.

## Syntax

```text
COUNT target = varlist (values) [varlist (values) ...] [/ target = ...].
```

values: values, low THRU high (with LOWEST or HIGHEST), MISSING, SYSMIS.

A value or range counts user-missing values too; a string variable takes quoted values only.

## Example

```text
COUNT channels = q5_1 TO q5_5 (1) / unanswered = q1 q2 (MISSING).
```

`statscraft help COUNT` prints the same text in the terminal. [All commands](README.md)
