<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# PARTIAL CORR

Partial correlations of variables controlling for others, with significance.

## Syntax

```text
PARTIAL CORR [VARIABLES=]varlist [WITH varlist] BY controls [(order ...)] [/...]
             [/SIGNIFICANCE=TWOTAIL | ONETAIL] [/STATISTICS=NONE CORR DESCRIPTIVES BADCORR ALL]
             [/FORMAT=MATRIX | CONDENSED | SERIAL] [/MISSING=LISTWISE | ANALYSIS  EXCLUDE | INCLUDE].
```

A list alone gives a square matrix, WITH a rectangular one. Without orders the partials control for every control variable at once; an order k gives one block per set of k controls (up to five orders, 25 lists). Coefficients come from the zero-order Pearson correlations step by step, and are tested with t on M - k - 2 df, M the smallest weighted count of the correlations used. A coefficient that cannot be computed is empty. CORR shows the zero-order correlations first, BADCORR only when one of them is empty; DESCRIPTIVES the mean, standard deviation and N. CONDENSED shows the coefficients alone, marked \* at 0.05 and \*\* at 0.01; SERIAL shows each pair once. LISTWISE (default) uses the cases valid on every variable of the list, controls included; ANALYSIS each pair's valid cases. Weighted when a weight variable is set. Not supported yet: MATRIX.

## Example

```text
PARTIAL CORR /VARIABLES=satisfaction recommend BY age /STATISTICS=CORR /SIGNIFICANCE=ONETAIL.
```

`statscraft help PARTIAL CORR` prints the same text in the terminal. [All commands](README.md)
