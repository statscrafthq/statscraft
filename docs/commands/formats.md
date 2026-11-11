<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# FORMATS

Set the display format of variables.

## Syntax

```text
FORMATS varlist (format) [[/] varlist (format) ...].
```

format: type, width and optional decimals, for example F8.2, A20, DATE11.

Output shows values in the format: DATE11 28-OCT-1990, DATE9 28-OCT-90, ADATE10 10/28/1990, TIME8 01:02:34, TIME5 01:02 (what the width cannot show is cut off), CCA to CCE as SET defines.

Procedures show the mean, median, mode, minimum, maximum and percentiles of a date, time or custom currency variable in its format; a spread of dates as days and time (DTIME).

## Example

```text
FORMATS income (COMMA10.2) / age (F3).
```

`statscraft help FORMATS` prints the same text in the terminal. [All commands](README.md)
