<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# STRING

Declare new string variables, filled with empty strings.

## Syntax

```text
STRING names (Awidth) [/ names (Awidth) ...].
```

A string variable must exist before COMPUTE or IF can assign text to it.

## Example

```text
STRING full_name (A40) / code1 code2 (A8).
```

`statscraft help STRING` prints the same text in the terminal. [All commands](README.md)
