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

CI adds two rows to each coefficient, its lower and upper limit: Fisher's transformation with the standard error 1 / sqrt(W - 3) (W the sum of the weights of the pair, above 3), BIAS=TRUE subtracting r / (2 (W - 1)) from the transformed value first; with ONETAIL one bound is -1 or 1 (the lower limit for a positive r, the upper for a negative or zero r) and the other is found at the one-tailed level. METHOD=EXACT is not supported yet.

MATRIX writes the last list without WITH as a matrix data file (ROWTYPE\_ MEAN, STDDEV, N and CORR, VARNAME\_; N is one record under LISTWISE, a record per variable under PAIRWISE unless every pair has the same count; one set per SPLIT FILE group); the tables are still shown, and with \* the file replaces the active dataset.

A list alone gives a square matrix, WITH a rectangular one (left variables in rows). NOSIG marks coefficients significant at 0.05 with \* and at 0.01 with \*\*. XPROD adds cross-products and covariances; DESCRIPTIVES a table of means, standard deviations and counts. PAIRWISE (default) uses the cases valid on both variables of a pair; LISTWISE the cases valid on every variable of every list. Weighted when a weight variable is set.

## Example

```text
CORRELATIONS /VARIABLES=age income WITH satisfaction /PRINT=ONETAIL NOSIG.
```

`statscraft help CORRELATIONS` prints the same text in the terminal. [All commands](README.md)
