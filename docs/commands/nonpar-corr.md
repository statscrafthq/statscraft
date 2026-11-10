<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# NONPAR CORR

Spearman's rho and Kendall's tau-b rank correlations with significance.

## Syntax

```text
NONPAR CORR [/VARIABLES=]varlist [WITH varlist] [/varlist ...]
            [/PRINT=SPEARMAN | KENDALL | BOTH  TWOTAIL | ONETAIL  SIG | NOSIG
                    FULL | LOWER | LNODIAG] [/MISSING=PAIRWISE | LISTWISE [INCLUDE]] [/SAMPLE].
```

A list alone gives a square matrix, WITH a rectangular one (left variables in rows). Ties get their average rank; rho is tested with t on N - 2 df, tau-b with the normal approximation and the tie-corrected variance. NOSIG marks coefficients significant at 0.05 with \* and at 0.01 with \*\*. PAIRWISE (default) uses the cases valid on both variables of a pair; LISTWISE the cases valid on every variable of the list, each list on its own. A weight variable repeats each case as often as its weight rounded to a whole number. SAMPLE changes nothing.

Not supported yet: MATRIX, CI.

## Example

```text
NONPAR CORR /VARIABLES=satisfaction recommend WITH age /PRINT=BOTH NOSIG.
```

`statscraft help NONPAR CORR` prints the same text in the terminal. [All commands](README.md)
