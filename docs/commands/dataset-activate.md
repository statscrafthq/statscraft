<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# DATASET ACTIVATE

Make a named dataset the active one.

## Syntax

```text
DATASET ACTIVATE name [WINDOW=ASIS | FRONT].
```

The dataset that was active stays open when it has a name, else it is closed.

## Example

```text
DATASET ACTIVATE survey.
```

`statscraft help DATASET ACTIVATE` prints the same text in the terminal. [All commands](README.md)
