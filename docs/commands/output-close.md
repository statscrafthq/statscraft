<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# OUTPUT CLOSE

Close an output document, or all of them.

## Syntax

```text
OUTPUT CLOSE [NAME=]name | * | ALL.
```

\* closes the designated document. Nothing is saved; the next table starts a new document.

A name that no open document has is ignored with a warning.

## Example

```text
OUTPUT CLOSE *.
```

`statscraft help OUTPUT CLOSE` prints the same text in the terminal. [All commands](README.md)
