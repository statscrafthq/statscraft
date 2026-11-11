<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# PERMISSIONS

Make a file read-only, or writeable for its owner.

## Syntax

```text
PERMISSIONS FILE='path' /PERMISSIONS READONLY | WRITEABLE.
```

READONLY takes the write permission from everyone; WRITEABLE gives it to the owner.

On a file system without such permissions READONLY sets the read-only flag.

## Example

```text
PERMISSIONS FILE='final.sav' /PERMISSIONS READONLY.
```

`statscraft help PERMISSIONS` prints the same text in the terminal. [All commands](README.md)
