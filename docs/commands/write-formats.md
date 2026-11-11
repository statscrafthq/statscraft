<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# WRITE FORMATS

Set the write format of variables: how WRITE writes them.

## Syntax

```text
WRITE FORMATS varlist (format) [[/] varlist (format) ...].
```

As FORMATS, but the print format stays as it is. Saved with the data file.

## Example

```text
WRITE FORMATS salary (F10.2).
```

`statscraft help WRITE FORMATS` prints the same text in the terminal. [All commands](README.md)
