<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# DATASET COPY

Copy the active dataset, as it is now, into a named dataset.

## Syntax

```text
DATASET COPY name [WINDOW=MINIMIZED | HIDDEN | FRONT].
```

A dataset that had the name is replaced, with a warning. When the active dataset has the name, the copy takes it and the active dataset is left without a name.

## Example

```text
DATASET COPY men.
```

`statscraft help DATASET COPY` prints the same text in the terminal. [All commands](README.md)
