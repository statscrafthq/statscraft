<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# VALUE LABELS

Replace the value labels of variables.

## Syntax

```text
VALUE LABELS varlist value 'label' [value 'label' ...] [/ varlist ...].
```

Values are numbers or quoted strings. A varlist with no labels removes its labels.

## Example

```text
VALUE LABELS gender 1 'Male' 2 'Female' / plan 'b' 'Basic' 'p' 'Premium'.
```

`statscraft help VALUE LABELS` prints the same text in the terminal. [All commands](README.md)
