<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# GET

Read a data file and make it the active dataset.

## Syntax

```text
GET FILE='path' | handle [/KEEP=varlist] [/DROP=varlist] [/RENAME=(old = new) ...] [/MAP].
```

KEEP, DROP and RENAME work in the order written, as in ADD FILES. MAP lists the variables as the options before it leave them, each with its name in the file; it may come as often as you like.

## Example

```text
GET FILE='survey.sav'.
```

`statscraft help GET` prints the same text in the terminal. [All commands](README.md)
