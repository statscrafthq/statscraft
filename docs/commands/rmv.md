<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# RMV

Copies of series with the missing values filled in by interpolation, a mean, a median or a trend.

## Syntax

```text
RMV new names = function(series) [/new names = function(...) ...].
```

function: LINT(series); MEAN(series \[, span\]); MEDIAN(series \[, span\]); SMEAN(series); TREND(series). span is a whole number of 1 or more, or ALL.

One new variable per series, in order, so the names on the left must match the count of series; v1 TO v4 names numbered variables. A new name that exists is replaced, also when it is the series itself. Valid values are copied. LINT draws a line between the nearest valid values before and after a gap, and leaves a gap at either end alone. MEAN and MEDIAN use the span nearest valid values on each side (2 without a span) and leave the gap alone when a side has fewer; ALL uses every valid value, as SMEAN does with the mean.

TREND fits a line to the valid values against the case position and uses it. An estimate never rests on another estimate. A user-missing value is missing unless TSET MISSING=INCLUDE.

With SPLIT FILE each group is a series of its own; USE is not used. A table lists each new variable with the number of values replaced, its first and last valid case, its number of valid values and its function.

## Example

```text
RMV tidy_load = LINT(hourly_load) /tidy_temp = MEDIAN(temp, 3).
```

`statscraft help RMV` prints the same text in the terminal. [All commands](README.md)
