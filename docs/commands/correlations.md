<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# CORRELATIONS

Pearson correlations with significance and case counts.

## Syntax

```text
CORRELATIONS [/VARIABLES=]varlist [WITH varlist] [/varlist ...]
             [/PRINT=TWOTAIL | ONETAIL  SIG | NOSIG  FULL | LOWER | LNODIAG]
             [/STATISTICS=DESCRIPTIVES XPROD ALL] [/MISSING=PAIRWISE | LISTWISE [INCLUDE]].
```

A list alone gives a square matrix, WITH a rectangular one (left variables in rows). NOSIG marks coefficients significant at 0.05 with \* and at 0.01 with \*\*. XPROD adds cross-products and covariances; DESCRIPTIVES a table of means, standard deviations and counts. PAIRWISE (default) uses the cases valid on both variables of a pair; LISTWISE the cases valid on every variable of every list. Weighted when a weight variable is set.

## Example

```text
CORRELATIONS /VARIABLES=age income WITH satisfaction /PRINT=ONETAIL NOSIG.
```

`statscraft help CORRELATIONS` prints the same text in the terminal. [All commands](README.md)
