<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# VARIABLE WIDTH

Set how many characters wide variables show in the data grid.

## Syntax

```text
VARIABLE WIDTH varlist (n) [[/] varlist (n) ...].
```

n: a whole number of characters, at least 1. Saved with the data; formats and output stay as they are.

## Example

```text
VARIABLE WIDTH comment (40) / id age (5).
```

`statscraft help VARIABLE WIDTH` prints the same text in the terminal. [All commands](README.md)
