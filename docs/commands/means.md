<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# MEANS

Statistics of dependent variables within groups of control variables.

## Syntax

```text
MEANS [TABLES=]deps [BY controls [BY ...]] [/deps BY ...] [/CELLS=cell ...]
      [/STATISTICS=ANOVA | LINEARITY | NONE] [/MISSING=TABLE | INCLUDE | DEPENDENT].
```

cell: MEAN COUNT STDDEV MEDIAN GMEDIAN SEMEAN SUM MIN MAX RANGE VARIANCE KURT SEKURT SKEW SESKEW FIRST LAST NPCT SPCT NPCT(var) SPCT(var) HARMONIC GEOMETRIC DEFAULT ALL; DEFAULT is MEAN COUNT STDDEV. GMEDIAN is the grouped median: each value is the midpoint of its interval.

NPCT(var) and SPCT(var) give the percent of the N and of the sum of the cases in the same category of var, which must be a control of every table list (all cases on its Total rows).

One Report table per dependent and combination of controls: every group of the first control, the next nested inside, each level with a Total. A Case Processing Summary per table list.

A string dependent gets COUNT, FIRST and LAST only. ANOVA gives the analysis of variance by the first control with eta; LINEARITY adds the linear term and r (three or more numeric groups).

Without /MISSING each dependent with its controls leaves out its own missing cases; any /MISSING leaves out cases missing on any variable of the list (INCLUDE: user-missing values count; DEPENDENT: they count for the controls only). Weighted when a weight variable is set.

## Example

```text
GET FILE='survey.sav'.
MEANS spend BY region.
MEANS spend age BY age_group BY gender /CELLS=MEAN COUNT STDDEV.
MEANS satisfaction BY education /STATISTICS=ANOVA.
```

The example reads the sample data file [survey.sav](../sample/survey.sav), an invented customer survey; save it in the folder you run the syntax from.

## Example output

### Report: Monthly spend in euros by Region

| Region | Mean | N | Std. Deviation |
| --- | --- | --- | --- |
| North | 36.14 | 71 | 11.89 |
| South | 40.50 | 59 | 16.44 |
| East | 38.47 | 76 | 13.51 |
| West | 39.19 | 78 | 11.10 |
| Total | 38.51 | 284 | 13.21 |

`statscraft help MEANS` prints the same text in the terminal. [All commands](README.md)
