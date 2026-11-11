<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# ERASE

Delete a file.

## Syntax

```text
ERASE FILE [=] 'path'.
```

One file, named in full; no wildcards. A missing file is an error.

## Example

```text
ERASE FILE='old_extract.txt'.
```

`statscraft help ERASE` prints the same text in the terminal. [All commands](README.md)
