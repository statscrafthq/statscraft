<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# DATA LIST

Define text data, inline or in a file, and read it as the new active dataset.

## Syntax

```text
DATA LIST [FILE='path'] [FIXED | FREE [(delimiters)] | LIST [(delimiters)]] [RECORDS=n] [SKIP=n]
          /[record] var [TO var] columns [(type[,decimals])] | (formats) [/ ...].
```

FIXED (the default): columns such as 1-3, or formats such as (F2.0, 1X, A10) with Tn and nX.

FREE and LIST: var \[(format)\], values separated by blanks and commas or by the delimiters given as quoted characters or TAB; LIST reads one case per line. \* ends a run of F8.2 variables.

Without FILE the data follow between BEGIN DATA and END DATA.

Formats: F, N, E, COMMA, DOT, DOLLAR, PCT, A, DATE, ADATE, EDATE, SDATE.

A value that cannot be read is system-missing, with a warning naming its line.

## Example

```text
DATA LIST LIST /id (F3) name (A12) score.
```

`statscraft help DATA LIST` prints the same text in the terminal. [All commands](README.md)
