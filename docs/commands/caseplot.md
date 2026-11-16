<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# CASEPLOT

Chart of time series with the cases down the vertical axis, as chart data.

## Syntax

```text
CASEPLOT [VARIABLES=] series [/DIFF[=n]] [/SDIFF[=n]] [/PERIOD=n] [/LN | /NOLOG]
         [/ID=variable] [/MARK=variable | date] [/SPLIT=UNIFORM | SCALE]
         [/FORMAT=[NOFILL | LEFT] [NOREFERENCE | REFERENCE[(value)]] | NOJOIN | JOIN | HILO]
         [/APPLY].
```

TSPLOT turned on its side: the cases run down the vertical axis and the values across.

LEFT fills the area between the axis and the line of a single series. Everything else is as for TSPLOT.

## Example

```text
CASEPLOT VARIABLES=hourly_load /ID=shift /FORMAT=LEFT.
```

`statscraft help CASEPLOT` prints the same text in the terminal. [All commands](README.md)
