<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# RENAME VARIABLES

Change the names of variables.

## Syntax

```text
RENAME VARIABLES (oldlist = newlist) [(oldlist = newlist) ...].
RENAME VARIABLES old = new [old = new ...].
```

All renames happen at once, so (a b = b a) swaps two names. new1 TO new3 counts up.

## Example

```text
RENAME VARIABLES (q1 q2 = satisfaction recommend) (v1 TO v3 = item1 TO item3).
```

`statscraft help RENAME VARIABLES` prints the same text in the terminal. [All commands](README.md)
