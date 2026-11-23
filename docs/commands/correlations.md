<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# CORRELATIONS

Pearson correlations with significance and case counts.

## Syntax

```text
CORRELATIONS [/VARIABLES=]varlist [WITH varlist] [/varlist ...]
             [/PRINT=TWOTAIL | ONETAIL  SIG | NOSIG  FULL | LOWER | LNODIAG]
             [/STATISTICS=DESCRIPTIVES XPROD ALL] [/MISSING=PAIRWISE | LISTWISE [INCLUDE]]
             [/MATRIX=OUT(* | 'file' | dataset)] [/CI [METHOD=FISHER] [BIAS=FALSE | TRUE] [CILEVEL=95]].
```

CI adds two rows to each coefficient, its lower and upper limit: Fisher's transformation with the standard error 1 / sqrt(W - 3) (W the sum of the weights of the pair, above 3), BIAS=TRUE subtracting r / (2 (W - 1)) from the transformed value first; with ONETAIL one bound is -1 or 1 (the lower limit for a positive r, the upper for a negative or zero r) and the other is found at the one-tailed level.

MATRIX writes the last list without WITH as a matrix data file (ROWTYPE\_ MEAN, STDDEV, N and CORR, VARNAME\_; N is one record under LISTWISE, a record per variable under PAIRWISE unless every pair has the same count; one set per SPLIT FILE group); the tables are still shown, and with \* the file replaces the active dataset.

A list alone gives a square matrix, WITH a rectangular one (left variables in rows). NOSIG marks coefficients significant at 0.05 with \* and at 0.01 with \*\*. XPROD adds cross-products and covariances; DESCRIPTIVES a table of means, standard deviations and counts. PAIRWISE (default) uses the cases valid on both variables of a pair; LISTWISE the cases valid on every variable of every list. Weighted when a weight variable is set.

## Example

```text
GET FILE='survey.sav'.
CORRELATIONS /VARIABLES=age spend satisfaction.
CORRELATIONS /VARIABLES=age spend WITH satisfaction /PRINT=ONETAIL NOSIG.
CORRELATIONS /VARIABLES=age spend satisfaction /MISSING=LISTWISE /STATISTICS=DESCRIPTIVES.
```

The example reads the sample data file [survey.sav](../sample/survey.sav), an invented customer survey; save it in the folder you run the syntax from.

## Example output

### Correlations

|  |  | Age in years | Monthly spend in euros | Overall satisfaction with the service |
| --- | --- | --- | --- | --- |
| Age in years | Pearson Correlation | 1 | -0.14 | -0.14 |
|  | Sig. (2-tailed) |  | 0.02 | 0.02 |
|  | N | 300 | 284 | 277 |
| Monthly spend in euros | Pearson Correlation | -0.14 | 1 | -0.16 |
|  | Sig. (2-tailed) | 0.02 |  | 0.01 |
|  | N | 284 | 284 | 261 |
| Overall satisfaction with the service | Pearson Correlation | -0.14 | -0.16 | 1 |
|  | Sig. (2-tailed) | 0.02 | 0.01 |  |
|  | N | 277 | 261 | 277 |

`statscraft help CORRELATIONS` prints the same text in the terminal. [All commands](README.md)
