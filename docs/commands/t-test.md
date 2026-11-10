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
T-TEST GROUPS=gender(1,2) /VARIABLES=age income /CRITERIA=CI(.90).
```

`statscraft help T-TEST` prints the same text in the terminal. [All commands](README.md)
