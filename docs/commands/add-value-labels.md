<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# ADD VALUE LABELS

Add or change value labels, keeping the other labels of the variables.

## Syntax

```text
ADD VALUE LABELS varlist value 'label' [value 'label' ...] [/ varlist ...].
```

Values are numbers or quoted strings; a label may be joined from pieces with +.

## Example

```text
ADD VALUE LABELS q1 TO q5 9 'No answer' / plan 'x' 'Unknown'.
```

`statscraft help ADD VALUE LABELS` prints the same text in the terminal. [All commands](README.md)
