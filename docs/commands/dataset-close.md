<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# DATASET CLOSE

Close a named dataset, or take the name off the active one.

## Syntax

```text
DATASET CLOSE name | * | ALL.
```

The active dataset stays open: closing it, or \*, only takes its name off. ALL closes every other dataset.

## Example

```text
DATASET CLOSE men.
```

`statscraft help DATASET CLOSE` prints the same text in the terminal. [All commands](README.md)
