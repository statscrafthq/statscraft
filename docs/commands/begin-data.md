<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# BEGIN DATA

Inline data for the DATA LIST before it, up to a line that holds END DATA.

## Syntax

```text
BEGIN DATA
```

data lines END DATA.

BEGIN DATA and END DATA each stand alone on their line. Every line between is data.

## Example

```text
BEGIN DATA
1 'Ann' 12.5
END DATA.
```

`statscraft help BEGIN DATA` prints the same text in the terminal. [All commands](README.md)
