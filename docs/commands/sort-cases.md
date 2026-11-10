<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# SORT CASES

Sort the cases by the values of variables.

## Syntax

```text
SORT CASES [BY] varlist [(A | D)] [varlist [(A | D)] ...].
```

A is ascending (the default), D descending; a direction covers the variables before it.

System-missing sorts lowest. Cases with equal keys keep their order.

## Example

```text
SORT CASES BY region (A) age (D).
```

`statscraft help SORT CASES` prints the same text in the terminal. [All commands](README.md)
