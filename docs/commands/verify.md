<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# VERIFY

Check the DATE variables and report the USE and PREDICT periods.

## Syntax

```text
VERIFY [[VARIABLES=]series].
```

A table with the first and last case of the file and of the USE and PREDICT periods, by case number and by DATE\_ text, then a table of the date variables: how many values are missing or out of range, and a flag with the first case where a variable is not nested in the next higher one, begins or ends a cycle off its usual value, or steps by more than it should.

With a numeric series, a third table gives its values at those cases and its missing count (user-missing values count as missing unless TSET MISSING=INCLUDE).

Run it after SELECT IF, SORT CASES or AGGREGATE have changed the file, before a time series procedure.

## Example

```text
DATE YEAR 2018 MONTH.
VERIFY sales.
```

`statscraft help VERIFY` prints the same text in the terminal. [All commands](README.md)
