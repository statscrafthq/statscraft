<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# FORMATS

Set the display format of variables.

## Syntax

```text
FORMATS varlist (format) [[/] varlist (format) ...].
```

format: type, width and optional decimals, for example F8.2, A20, DATE11.

## Example

```text
FORMATS income (COMMA10.2) / age (F3).
```

`statscraft help FORMATS` prints the same text in the terminal. [All commands](README.md)
