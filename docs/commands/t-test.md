<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# T-TEST

One-sample, independent-samples and paired-samples t tests with effect sizes.

## Syntax

```text
T-TEST /TESTVAL=value /VARIABLES=varlist
T-TEST /GROUPS=var[(value) | (value, value)] /VARIABLES=varlist
T-TEST /PAIRS=varlist [WITH varlist [(PAIRED)]] [/varlist ...]
       [/CRITERIA=CI(0.95)] [/MISSING=ANALYSIS | LISTWISE [INCLUDE]]
       [/ES DISPLAY(TRUE | FALSE) STANDARDIZER(SD | CORRECTED_SD | AVERAGE)].
```

GROUPS without values compares 1 with 2; one value splits at it (at or above against below).

The independent test shows Levene's test and rows for equal and unequal variances. PAIRS without WITH pairs every variable with every later one; WITH pairs each left with each right one, and (PAIRED) the n-th with the n-th. ANALYSIS (default) leaves out cases missing in each analysis, LISTWISE those missing on any variable of the test type. Effect sizes: Cohen's d and Hedges' correction (and Glass's delta for groups) with confidence intervals; they use weights rounded to whole numbers. Weighted when a weight variable is set.

## Example

```text
GET FILE='survey.sav'.
T-TEST GROUPS=contacted(0, 1) /VARIABLES=satisfaction spend.
T-TEST /TESTVAL=40 /VARIABLES=spend.
T-TEST GROUPS=gender(1, 2) /VARIABLES=age /CRITERIA=CI(0.99).
```

The example reads the sample data file [survey.sav](../sample/survey.sav), an invented customer survey; save it in the folder you run the syntax from.

## Example output

### Group Statistics

|  |  | N | Mean | Std. Deviation | Std. Error Mean |
| --- | --- | --- | --- | --- | --- |
| Overall satisfaction with the service | No | 144 | 3.13 | 1.02 | 0.08 |
|  | Yes | 133 | 3.18 | 1.31 | 0.11 |
| Monthly spend in euros | No | 146 | 39.40 | 14.01 | 1.16 |
|  | Yes | 138 | 37.56 | 12.30 | 1.05 |

`statscraft help T-TEST` prints the same text in the terminal. [All commands](README.md)
