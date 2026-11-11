<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# DATE

Add date variables that count the cases of a time series.

## Syntax

```text
DATE keyword [start [period]] [keyword ...] [BY n].
```

keyword: YEAR QUARTER MONTH (long periods), WEEK DAY HOUR MINUTE SECOND (short periods), CYCLE, OBS; any start of the word, MINUTE from MI. Long and short do not mix.

Each keyword makes a numeric variable named keyword\_; DATE\_ (a string) shows them all, and CYCLE\_ is added when the highest keyword has a period. Defaults: start 1 (HOUR, MINUTE, SECOND 0), period QUARTER 4, MONTH 12, DAY 7, HOUR 24, MINUTE and SECOND 60, none for the highest keyword except QUARTER. OBS needs a start and a period; CYCLE takes no period. QUARTER and MONTH give each other their start; with MONTH the quarter follows the month. BY n steps the lowest keyword and must divide its period. Earlier DATE variables are replaced. A table lists the new variables and their labels.

## Example

```text
DATE Y 1990 M 7.
```

`statscraft help DATE` prints the same text in the terminal. [All commands](README.md)
