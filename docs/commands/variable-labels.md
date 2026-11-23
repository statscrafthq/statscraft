<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# VARIABLE LABELS

Set the labels of variables.

## Syntax

```text
VARIABLE LABELS var 'label' [[/] var 'label' ...].
```

A label replaces the variable's earlier one and is shown in place of the name in the output. An empty label ('') clears it. Scratch variables (names that start with \#) take no label.

## Example

```text
VARIABLE LABELS spend 'Monthly bill in euros' / age_group 'Age band'.
```

`statscraft help VARIABLE LABELS` prints the same text in the terminal. [All commands](README.md)
