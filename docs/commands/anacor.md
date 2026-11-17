<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# ANACOR

Correspondence analysis in the older syntax: CORRESPONDENCE with its own normalization names and defaults.

## Syntax

```text
ANACOR TABLE=rowvar(min,max) BY colvar(min,max) | ALL(rows,columns)
   [/DIMENSION=2 | n]
   [/NORMALIZATION=CANONICAL | PRINCIPAL | RPRINCIPAL | CPRINCIPAL | q]
   [/VARIANCES=[SINGULAR] [ROWS] [COLUMNS]]
   [/PRINT=[TABLE] [PROFILES] [SCORES] [CONTRIBUTIONS] [PERMUTATION] [DEFAULT] [NONE]]
   [/PLOT=[NDIM(first | ALL, last | MAX)] [ROWS[(len)]] [COLUMNS[(len)]] [JOINT[(len)]]
         [TRROWS] [TRCOLUMNS] [DEFAULT[(len)]] [NONE]]
   [/MATRIX OUT=[SCORE(* | 'file' | dataset)] [VARIANCE(* | 'file' | dataset)]]
```

The table is read and analyzed as CORRESPONDENCE reads it with MEASURE=CHISQ; messages name ANACOR. NORMALIZATION q gives the row scores the power (1 + q)/2 of each singular value and the column scores (1 - q)/2, q from -2 to 2: CANONICAL (or DEFAULT, the default) is 0, RPRINCIPAL 1, CPRINCIPAL -1; PRINCIPAL gives both the full value. The Summary always shows the singular values with their spread and correlations, so VARIANCES SINGULAR changes nothing; ROWS and COLUMNS add the spread and correlations of the row and column scores. PRINT: TABLE the table with its margins, PROFILES the row and column profiles, SCORES and CONTRIBUTIONS the overview of row and column points (mass, scores, inertia and both kinds of contributions), PERMUTATION the table sorted by the scores of each dimension; DEFAULT (and no PRINT) TABLE SCORES CONTRIBUTIONS.

PLOT: ROWS and COLUMNS (the default) each side alone, JOINT both together (not with PRINCIPAL), TRROWS and TRCOLUMNS the scores per category; labels are cut to 3 characters unless a length is given (20 for the transformation plots). NDIM(a, b) draws dimension a against a+1 to b, ALL every pair, MAX the last dimension; without it dimension 1 against 2. MATRIX OUT writes the scores and the variances as OUTFILE of CORRESPONDENCE does; an asterisk replaces the active dataset.

## Example

```text
ANACOR TABLE=region(1,4) BY channel(1,3) /NORMALIZATION=RPRINCIPAL /PRINT=SCORES PROFILES /PLOT=JOINT(6).
```

`statscraft help ANACOR` prints the same text in the terminal. [All commands](README.md)
