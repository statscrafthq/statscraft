<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# VECTOR

Name a range of variables, or new variables, as a vector for indexed use.

## Syntax

```text
VECTOR name = first TO last [/ ...].
VECTOR prefix [, prefix ...] (n [format]) [/ ...].
```

The first form refers to existing variables, all numeric or all strings of one width; the second creates prefix1 to prefixn (F8.2, or the format given). v(i) reads an element in expressions and is a target of COMPUTE and IF. A vector lasts until the next procedure.

## Example

```text
VECTOR q = q1 TO q10 / new (5, F2).
```

`statscraft help VECTOR` prints the same text in the terminal. [All commands](README.md)
