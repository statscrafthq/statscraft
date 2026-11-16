<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# TSPLOT

Chart of time series over the cases, as chart data.

## Syntax

```text
TSPLOT [VARIABLES=] series [/DIFF[=n]] [/SDIFF[=n]] [/PERIOD=n] [/LN | /NOLOG]
       [/ID=variable] [/MARK=variable | date] [/SPLIT=UNIFORM | SCALE]
       [/FORMAT=[NOFILL | BOTTOM] [NOREFERENCE | REFERENCE[(value)]] | NOJOIN | JOIN | HILO]
       [/APPLY].
```

The cases run along the horizontal axis, labeled by ID (else TSET ID, else DATE\_, else the case number). Each series may be logged first (LN; values not above 0 become missing), then differenced DIFF times and over the period SDIFF times (DIFF or SDIFF alone: once); the period is PERIOD, TSET PERIOD or that of the DATE variables, and without one SDIFF is skipped with a warning. With one series, BOTTOM fills the area under the line (only when no value is missing or negative) and REFERENCE adds a flat line at the value or at the series mean. With several, JOIN links the values of each case and HILO shows only the highest and lowest of each case (up to three series). MARK names the cases where a variable changes, or a date given with DATE keywords, in the footnote, which also lists the transformations. Under SPLIT FILE each group gets a chart; the reference mean is the group's. USE limits the cases. APPLY without a name repeats the previous TSPLOT with the subcommands given now replacing its own.

## Example

```text
TSPLOT VARIABLES=river_level rainfall /FORMAT=JOIN /MARK=dam_open.
```

`statscraft help TSPLOT` prints the same text in the terminal. [All commands](README.md)
