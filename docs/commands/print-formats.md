<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# PRINT FORMATS

Set the print format of variables: how procedures and PRINT show them.

## Syntax

```text
PRINT FORMATS varlist (format) [[/] varlist (format) ...].
```

As FORMATS, but the write format stays as it is. A string variable keeps its width.

## Example

```text
PRINT FORMATS salary (DOLLAR8) / rate (PCT5.1).
```

`statscraft help PRINT FORMATS` prints the same text in the terminal. [All commands](README.md)
