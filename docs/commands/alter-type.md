<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# ALTER TYPE

Change the type, string width or format of variables.

## Syntax

```text
ALTER TYPE varlist ([input =] format | AMIN [+ n[%]]) [varlist (...) ...] [/PRINT ...].
```

A string variable may get another width (A20); values longer than it are cut. A numeric variable may get another numeric format, or become a string holding its formatted values; a string may become numeric with an F, COMMA, DOT, E, DOLLAR, PCT or N format, or a date or time with a DATE, ADATE, EDATE, SDATE, JDATE, QYR, MOYR, WKYR, DATETIME, YMDHMS, TIME or MTIME format (ALTER TYPE visit (A12 = ADATE12)), read in the width of the format as DATA LIST reads it; a value that does not fit the format becomes system-missing. A type change drops value labels and missing values.

input = (A=, F=, A10=) limits the change to variables of that type and width.

AMIN gives a string variable the width of its longest value, plus n bytes or n percent of that width (rounded).

/PRINT=ALTEREDTYPES (the default) lists the variables whose format changed, with the old and the new format; ALTEREDVALUES lists the values that changed with the type or the width (every value when the type changed, the cut ones when only the width did), the first 25 of each variable, by case number; NONE shows no report and does not go with the others.

## Example

```text
ALTER TYPE comment (A500) code (A=AMIN).
```

`statscraft help ALTER TYPE` prints the same text in the terminal. [All commands](README.md)
