<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# DATA LIST

Define text data, inline or in a file, and read it as the new active dataset.

## Syntax

```text
DATA LIST [FILE='path'] [ENCODING='name'] [FIXED | FREE [(delimiters)] | LIST [(delimiters)]]
          [RECORDS=n] [SKIP=n] [END=var] /[record] var [TO var] columns [(type[,decimals])] | (formats) [/ ...].
```

FIXED (the default): columns such as 1-3, or formats such as (F2.0, 1X, A10) with Tn and nX.

FREE and LIST: var \[(format)\], values separated by blanks and commas or by the delimiters given as quoted characters or TAB; LIST reads one case per line. \* ends a run of F8.2 variables.

Without FILE the data follow between BEGIN DATA and END DATA; transformations may stand between. Inside INPUT PROGRAM each run reads the next case's records into the case being built; without variables it passes over a record. END (fixed data in an input program only) names a variable that is 0, and 1 once the data are used up.

Formats: F, N, E, COMMA, DOT, DOLLAR, PCT, Z (zoned decimal: 1234A is 12341, 1234J is -12341), PIBHEX (hexadecimal digits, the whole width), A, DATE, ADATE, EDATE, SDATE, JDATE (yyddd or yyyyddd), QYR (4Q90, Q4 1990), MOYR (Oct-1990), WKYR (43WK90, 43 1990), DATETIME, YMDHMS (year first), TIME (hours:minutes:seconds), MTIME (minutes:seconds), DTIME (days, then time). A date may be written with blanks, dashes, slashes, periods or commas between its parts, a month as a number, a Roman numeral or a name, or with no delimiter at all (28OCT1990, 10281990); a two-digit year falls in the 100 years from SET EPOCH; a time with no colons is hhmm or hhmmss.

ENCODING is how the text of FILE is encoded: UTF8 (the default, or the encoding a FILE HANDLE gives), a code page such as '1257' or 'windows-1252', UTF16LE, UTF16BE. Fixed columns and the widths of A formats count bytes in a UTF-8 file (a letter with an accent takes two), letters in a code page or UTF-16 file.

A value that cannot be read is system-missing, with a warning naming its line.

## Example

```text
DATA LIST LIST /id (F3) name (A12) score.
```

`statscraft help DATA LIST` prints the same text in the terminal. [All commands](README.md)
