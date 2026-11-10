<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# DATASET NAME

Give the active dataset a name, so that it stays open when another one becomes active.

## Syntax

```text
DATASET NAME name [WINDOW=ASIS | FRONT].
```

A dataset that had the name before is closed. WINDOW is accepted and has no effect.

## Example

```text
DATASET NAME survey.
```

`statscraft help DATASET NAME` prints the same text in the terminal. [All commands](README.md)
