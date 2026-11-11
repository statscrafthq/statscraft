<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# CLEAR TRANSFORMATIONS

Undo the transformations and dictionary changes since the last data pass.

## Syntax

```text
CLEAR TRANSFORMATIONS.
```

The dataset goes back to how it was after the last command that read the data (a procedure, EXECUTE, SAVE, GET and the like) or the start of the run. Pending PRINT, WRITE and XSAVE never run. Settings, titles and files stay as they are.

## Example

```text
CLEAR TRANSFORMATIONS.
```

`statscraft help CLEAR TRANSFORMATIONS` prints the same text in the terminal. [All commands](README.md)
