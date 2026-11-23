<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# STRING

Declare new string variables, filled with empty strings.

## Syntax

```text
STRING names (Awidth) [/ names (Awidth) ...].
```

Each name must be new, and every case starts as an empty string. A string variable must exist before COMPUTE or IF can assign text to it; a longer text is cut to the width.

## Example

```text
STRING region_code (A2).
```

`statscraft help STRING` prints the same text in the terminal. [All commands](README.md)
