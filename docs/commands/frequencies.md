<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# FREQUENCIES

Frequency tables, summary statistics and percentiles.

## Syntax

```text
FREQUENCIES [/]VARIABLES=varlist [/FORMAT=[NOTABLE | LIMIT(n)] [AVALUE | DVALUE | AFREQ | DFREQ]]
            [/MISSING=INCLUDE] [/STATISTICS=[stat ...]] [/PERCENTILES=p ...] [/NTILES=n]
            [/GROUPED=varlist [(width) | (boundary, ...)]] [/ORDER=ANALYSIS | VARIABLE]
            [/BARCHART=...] [/PIECHART=...] [/HISTOGRAM=...].
```

stat: MEAN SEMEAN MEDIAN MODE STDDEV VARIANCE SKEWNESS SESKEW KURTOSIS SEKURT RANGE MINIMUM MAXIMUM SUM DEFAULT ALL NONE. STATISTICS alone or DEFAULT is MEAN STDDEV MINIMUM MAXIMUM; /STATS works too.

A frequency table per variable: valid values, then missing ones, with percent, valid percent and cumulative percent. NOTABLE leaves the tables out, LIMIT(n) the tables with more than n values.

DVALUE, AFREQ and DFREQ order the values by descending value or by frequency (ties ascending by value); they are ignored when percentiles or a histogram are asked for. A Statistics table, one for all variables (ORDER=VARIABLE: one per variable, before its frequency table), shows the valid and missing counts and the statistics; it appears when statistics or percentiles are asked for, or when a frequency table is left out. Percentiles use (W + 1) p, the median is the 50th percentile and is left out with AFREQ and DFREQ; MODE shows the smallest of tied modes, with a note. NTILES=n adds the n - 1 points that cut the cases into n equal groups (n up to 1000); several PERCENTILES and NTILES add up.

GROUPED treats the values as groups for the median and percentiles: alone the values are group midpoints, (width) groups that wide around each value, (boundaries) the given groups; a value outside the boundaries falls in an open group, where percentiles are undefined (shown empty).

INCLUDE makes user-missing values valid. Weighted when a weight variable is set; a case with a missing, zero or negative weight is left out. String variables get the table and the counts only.

BARCHART, PIECHART and HISTOGRAM are accepted (BARCHART and HISTOGRAM not together) and not drawn yet, with a warning.

## Example

```text
FREQUENCIES VARIABLES=age region /FORMAT=LIMIT(10) /STATISTICS=MEAN MEDIAN MODE /NTILES=4.
```

`statscraft help FREQUENCIES` prints the same text in the terminal. [All commands](README.md)
