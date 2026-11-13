<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# CORRESPONDENCE

Correspondence analysis: rows and columns of a two-way table as points in a few dimensions.

## Syntax

```text
CORRESPONDENCE TABLE=rowvar(min,max) BY colvar(min,max) | ALL(rows,columns)
   [/SUPPLEMENTARY=[rowvar | ROW (values)] [colvar | COLUMN (values)]]
   [/EQUAL=[rowvar | ROW (values) (values) ...] [colvar | COLUMN (values) ...]]
   [/MEASURE=CHISQ | EUCLID] [/STANDARDIZE=RCMEAN | RMEAN | CMEAN | RSUM | CSUM]
   [/DIMENSION=2 | n] [/NORMALIZATION=SYMMETRICAL | PRINCIPAL | RPRINCIPAL | CPRINCIPAL | q]
   [/PRINT=[TABLE] [RPROFILES] [CPROFILES] [RPOINTS] [CPOINTS] [RCONF] [CCONF]
           [PERMUTATION[(n)]] [DEFAULT] [NONE]]
   [/PLOT=[BIPLOT[(len)]] [RPOINTS[(len)]] [CPOINTS[(len)]] [TRROWS[(len)]] [TRCOLUMNS[(len)]]
          [NDIM(first, last | MAX)] [NONE]]
   [/OUTFILE=[SCORE('file' | dataset)] [VARIANCE('file' | dataset)]]
```

Each case adds its weight (0 when negative) to one cell: every whole number in a range is a category, values are truncated to whole numbers, and a case outside a range or missing on either variable is left out. With ALL the first cases are the rows and the first variables the columns (ROWCAT\_ aside, whose values and labels name the rows); a missing or negative cell counts as 0 and WEIGHT does not apply. SUPPLEMENTARY rows and columns are placed in the solution after it is found and take no part in it; each EQUAL group is added up into one category whose point all its members share. Empty rows and columns are left out. CHISQ analyzes the standardized residuals of the table; EUCLID uses STANDARDIZE: RMEAN takes out the row means, CMEAN the column means, RCMEAN both (the same as CHISQ), RSUM and CSUM first bring the row or column totals to one value. The solution has at most one dimension fewer than the smaller of the active rows and columns; the Summary lists every dimension with a singular value above 0.

NORMALIZATION q gives the row scores the power (1 + q)/2 of each singular value and the column scores (1 - q)/2; PRINCIPAL gives both the full value, which rules out the biplot. In every dimension the active row with the largest absolute score scores positive.

Tables: Summary always (singular values, inertia, the chi-square of the table with its significance, the share of the inertia of each dimension, and the standard deviation of each singular value with the correlations between them); TABLE, RPOINTS and CPOINTS without PRINT or with DEFAULT: the table with its active margins and the Overview Row and Column Points (mass, scores, inertia, each point's share of a dimension and each dimension's share of a point); RPROFILES and CPROFILES the profiles; RCONF and CCONF the standard deviations of the active scores and their correlations across dimensions (delta method under multinomial sampling); PERMUTATION(n) the table sorted by the scores of each of the first n dimensions (1 by default).

Plots: BIPLOT (the default) the rows and columns together, RPOINTS and CPOINTS each side alone, one chart for each pair of dimensions; TRROWS and TRCOLUMNS the scores of the categories in each dimension. NDIM(a, b) plots dimension a against a+1 to b, and the transformation plots of a to b; without it, or without values, every pair. A number after a plot keyword cuts the labels to that length (60 at most). OUTFILE SCORE writes ROWTYPE\_, LEVEL\_, VARNAME\_ and DIM1, DIM2 and so on for every category; VARIANCE writes the covariances of the singular values (VARNAME\_ SINGULAR) and of the scores of each active point outside the EQUAL groups, one case per dimension (DIMNMBR\_).

## Example

```text
CORRESPONDENCE TABLE=district(1,5) BY transport(1,4) /SUPPLEMENTARY=district(5) /PRINT=DEFAULT RPROFILES RCONF /PLOT=BIPLOT.
```

`statscraft help CORRESPONDENCE` prints the same text in the terminal. [All commands](README.md)
