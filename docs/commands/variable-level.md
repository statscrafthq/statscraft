<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# VARIABLE LEVEL

Set the measurement level of variables.

## Syntax

```text
VARIABLE LEVEL varlist (SCALE | ORDINAL | NOMINAL) [[/] varlist (...) ...].
```

## Example

```text
VARIABLE LEVEL age (SCALE) / q1 TO q5 (ORDINAL).
```

`statscraft help VARIABLE LEVEL` prints the same text in the terminal. [All commands](README.md)
