<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# DATAFILE ATTRIBUTE

Set or delete custom attributes of the data file.

## Syntax

```text
DATAFILE ATTRIBUTE ATTRIBUTE=name('value') [name[n]('value') ...] [DELETE=name ...].
```

As VARIABLE ATTRIBUTE, for the file.

## Example

```text
DATAFILE ATTRIBUTE ATTRIBUTE=Version('2') Revised[1]('2026-11-10').
```

`statscraft help DATAFILE ATTRIBUTE` prints the same text in the terminal. [All commands](README.md)
