<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# CD

Change the folder relative file names start from.

## Syntax

```text
CD 'folder' | handle.
```

A relative folder starts from the current one, which is the folder of the syntax file at first. When the last part names no existing folder it is taken as a file name and left out.

## Example

```text
CD 'data/2026'.
```

`statscraft help CD` prints the same text in the terminal. [All commands](README.md)
