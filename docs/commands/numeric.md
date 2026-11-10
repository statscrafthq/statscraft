<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# NUMERIC

Declare new numeric variables, filled with system-missing.

## Syntax

```text
NUMERIC names [(format)] [/ names [(format)] ...].
```

Without a format the variables get F8.2.

## Example

```text
NUMERIC score1 score2 (F5.1) / flag.
```

`statscraft help NUMERIC` prints the same text in the terminal. [All commands](README.md)
