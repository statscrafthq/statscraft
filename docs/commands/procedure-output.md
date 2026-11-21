<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# PROCEDURE OUTPUT

Name the file the next procedures write their results to.

## Syntax

```text
PROCEDURE OUTPUT OUTFILE='file' | handle.
```

CROSSTABS /WRITE puts its cell counts in this file. The file stays in effect, for every procedure that asks for it, until another PROCEDURE OUTPUT replaces it. A relative name starts from the current folder (CD).

## Example

```text
PROCEDURE OUTPUT OUTFILE='cells.txt'.
```

`statscraft help PROCEDURE OUTPUT` prints the same text in the terminal. [All commands](README.md)
