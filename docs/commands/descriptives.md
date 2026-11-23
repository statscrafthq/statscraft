<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# DESCRIPTIVES

Summary statistics of numeric variables, optionally saved as z-scores.

## Syntax

```text
DESCRIPTIVES [/]VARIABLES=varlist [/STATISTICS=stat ...] [/SORT=[key] [(A | D)]]
             [/MISSING=VARIABLE | LISTWISE [INCLUDE]] [/SAVE].
```

stat: MEAN SEMEAN STDDEV VARIANCE KURTOSIS SKEWNESS RANGE MINIMUM MAXIMUM SUM DEFAULT ALL.

DEFAULT is MEAN STDDEV MINIMUM MAXIMUM; N is always shown. Weighted when a weight variable is set.

SORT lists the variables by key (MEAN SEMEAN STDDEV VARIANCE KURTOSIS SKEWNESS RANGE MIN MAX SUM or NAME), ascending (A, the default) or descending (D); without a key it is the mean. A variable without the statistic goes last. The key need not be among the statistics shown.

MISSING=VARIABLE (the default) leaves out the missing values of each variable on its own; LISTWISE leaves out a case missing on any listed variable; INCLUDE counts user-missing values.

The row Valid N (listwise) counts the cases valid on every listed variable. String variables get no statistics; a warning names them. var (name) in the list adds a z-score under that name; SAVE adds one for every variable, named Z plus the variable name (ZSC001 to ZSC099, STDZ01 to STDZ09, ZZZZ01 to ZZZZ09, ZQZQ01 to ZQZQ09 when that name is taken), labeled Zscore: plus the label.

## Example

```text
GET FILE='survey.sav'.
DESCRIPTIVES age spend satisfaction.
DESCRIPTIVES spend /STATISTICS=MEAN SEMEAN STDDEV MIN MAX.
DESCRIPTIVES age spend /SAVE.
```

The example reads the sample data file [survey.sav](../sample/survey.sav), an invented customer survey; save it in the folder you run the syntax from.

## Example output

### Descriptive Statistics

| Variable | Label | N | Mean | Std Dev | Minimum | Maximum |
| --- | --- | --- | --- | --- | --- | --- |
| age | Age in years | 300 | 45.74 | 13.70 | 18 | 84 |
| spend | Monthly spend in euros | 284 | 38.51 | 13.21 | 13.63 | 94.52 |
| satisfaction | Overall satisfaction with the service | 277 | 3.16 | 1.16 | 1 | 5 |
| Valid N (listwise) |  | 261 |  |  |  |  |
| Missing N (listwise) |  | 39 |  |  |  |  |

`statscraft help DESCRIPTIVES` prints the same text in the terminal. [All commands](README.md)
