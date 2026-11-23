<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# SYSFILE INFO

Show the dictionary of a data file without opening it as the active dataset.

## Syntax

```text
SYSFILE INFO [FILE=] 'path'.
```

A table of the file (label, cases, variables, documents) and the tables of DISPLAY DICTIONARY.

## Example

```text
SYSFILE INFO FILE='survey.sav'.
```

The example reads the sample data file [survey.sav](../sample/survey.sav), an invented customer survey; save it in the folder you run the syntax from.

`statscraft help SYSFILE INFO` prints the same text in the terminal. [All commands](README.md)
