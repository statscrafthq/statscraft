<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# ALTER TYPE

Change the type, string width or format of variables.

## Syntax

```text
ALTER TYPE varlist ([input =] format | AMIN [+ n[%]]) [varlist (...) ...] [/PRINT ...].
```

A string variable may get another width (A20); values longer than it are cut. A numeric variable may get another numeric format, or become a string holding its formatted values; a string may become numeric with an F, COMMA or DOT format, a value that is no number becoming system-missing. A type change drops value labels and missing values.

input = (A=, F=, A10=) limits the change to variables of that type and width.

AMIN gives a string variable the width of its longest value, plus n bytes or n percent of that width (rounded).

/PRINT is accepted; no report is shown.

## Example

```text
ALTER TYPE comment (A500) code (A=AMIN).
```

`statscraft help ALTER TYPE` prints the same text in the terminal. [All commands](README.md)
