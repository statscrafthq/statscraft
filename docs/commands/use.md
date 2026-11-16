<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# USE

Set the historical period of the time series procedures, or use all cases.

## Syntax

```text
USE ALL.
USE [start] [THRU [end]] [/PERMANENT].
```

A start or end is a case number or a date written with DATE keywords and values (YEAR 2019 MONTH 3; the highest date variable must be among them); FIRST and LAST mean the ends of the file. The two kinds do not mix. The range acts as a filter for the procedures that follow, while CREATE ignores it and EXSMOOTH and ARIMA estimate on it and forecast after it (see PREDICT). A date start is the first case of that date, a date end the last. ALL or THRU alone uses every case again. USE turns FILTER off; FILTER, SORT CASES and DATE turn USE off. /PERMANENT deletes the cases outside the range.

## Example

```text
USE YEAR 2016 THRU YEAR 2021.
```

`statscraft help USE` prints the same text in the terminal. [All commands](README.md)
