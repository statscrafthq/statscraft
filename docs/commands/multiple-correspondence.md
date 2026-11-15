<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# MULTIPLE CORRESPONDENCE

Multiple correspondence (homogeneity) analysis: cases and categories of several nominal variables as points in a few dimensions.

## Syntax

```text
MULTIPLE CORRESPONDENCE [/VARIABLES=] varlist
   /ANALYSIS=varlist [(WEIGHT=n)] [varlist [(WEIGHT=n)] ...]
   [/DISCRETIZATION=[varlist [([GROUPING] [NCAT=7 | n] [DISTR=NORMAL | UNIFORM] | [EQINTV=width]
                               | RANKING | MULTIPLYING)] ...]]
   [/MISSING=[varlist | ALL [([PASSIVE] [MODEIMPU | EXTRACAT] | ACTIVE [MODEIMPU | EXTRACAT | RANDIMPU]
                               | LISTWISE)] ...]]
   [/SUPPLEMENTARY=[OBJECT(case numbers)] [VARIABLE(varlist)]]
   [/CONFIGURATION=[INITIAL | FIXED]('file' | dataset)] [/DIMENSION=2 | n]
   [/NORMALIZATION=VPRINCIPAL | OPRINCIPAL | SYMMETRICAL | INDEPENDENT | q]
   [/MAXITER=100 | n] [/CRITITER=.00001 | value]
   [/PRINT=[CORR] [DESCRIP[(varlist)]] [DISCRIM] [HISTORY] [OBJECT[([(labelvar)] varlist)]] [OCORR]
          [QUANT[(varlist)]] [NONE]]
   [/PLOT=[OBJECT[(varlist)][(len)]] [CATEGORY(varlist)[(len)]] [JOINTCAT[(varlist)][(len)]]
         [DISCRIM[(varlist)][(len)]] [BIPLOT[((varlist))][(varlist)][(len)]] [TRANS(var[(n)] ...)]
         [RESID(var[(n)] ...)[(len)]] [NDIM(first, last)] [NONE]]
   [/SAVE=[TRDATA[(root[(n)])]] [OBJECT[(root[(n)])]]]
   [/OUTFILE=[DISCRDATA('file' | dataset)] [TRDATA('file' | dataset)] [OBJECT('file' | dataset)]].
```

Each analysis variable holds categories coded 1, 2, 3 and so on; a code below 1, a system-missing and a user-missing value are missing. String values are coded in sorted order, and a numeric variable with fractions is cut into 7 groups (fewer if it has fewer values) shaped like a normal curve. DISCRETIZATION recodes on purpose, without case weights and with codes below 1 still valid: GROUPING into NCAT groups, NORMAL by the optimal cut points of a normal curve applied to z-scores, UNIFORM with counts near n/NCAT, or EQINTV bands of a given width from the smallest value; RANKING numbers the distinct values; MULTIPLYING rounds 10 times the z-score and shifts the lowest to 1. The analysis places each case (object) and each category in a few dimensions so that a category is the weighted mean of its objects and objects lie close to their own categories; the WEIGHT on ANALYSIS gives a variable more say, and WEIGHT cases count by their weight. MISSING PASSIVE (the default) leaves the variable out for that object; its MODEIMPU or EXTRACAT only fills the correlation tables. ACTIVE fills the value before the fit with the weighted mode (smallest code on ties), an extra category or a random category drawn by frequency from the session's generator. LISTWISE drops the case. A case without any active value becomes supplementary. Supplementary objects and variables are placed after the fit.

CONFIGURATION INITIAL starts the iterations from the coordinates in a file (one variable per dimension, one case per case of the data); FIXED keeps those coordinates, standardized and made uncorrelated, and fits every analysis variable as supplementary. Without a file the start is the exact solution of the eigenproblem, so the iterations only confirm it (with more than 600 categories in all, a fixed pseudo-random start instead). DIMENSION is lowered with a warning to what the data allow. The fit stops after MAXITER iterations or when it gains less than CRITITER.

NORMALIZATION q gives the object scores the power (1 + q)/2 of each singular value (the root of the inertia) and the categories the power (1 - q)/2, both measured from the variable principal solution: VPRINCIPAL -1 (objects standard, categories at their centroids), OPRINCIPAL 1, SYMMETRICAL 0, INDEPENDENT both principal. In every dimension the object with the largest absolute score is positive. SPLIT FILE does not apply.

Tables: Case Processing Summary, Iteration History (the last step, or every step with HISTORY: variance accounted for, its increase and the loss) and Model Summary (Cronbach's alpha, eigenvalue, inertia and percent of variance per dimension, the total and the mean) always; DESCRIP the frequencies, missing count and mode of each variable; DISCRIM the discrimination measures with the active total; CORR the correlations of the quantified variables in each dimension and OCORR of the codes, each with the eigenvalues of the active part; QUANT per variable the centroids, mass, inertia and both kinds of contributions; OBJECT the object scores (optionally labelled by a variable and with codes of others beside them) and the object contributions. Without PRINT, or with PRINT alone: DESCRIP DISCRIM CORR.

Charts (OBJECT and DISCRIM by default): objects, colored by the categories of each named variable; category points of each variable; all categories together (JOINTCAT); the discrimination measures; objects with centroids (BIPLOT, not with INDEPENDENT); quantification by code (TRANS) and quantified value against object score (RESID) for the first n dimensions.

Two dimensions give one scatter, more a scatterplot matrix, NDIM(a, b) one scatter of a against each of a+1 to b. A label length of 0 shows values and names instead of labels.

SAVE adds the quantified variables rootK\_D\_S (TRA by default, K the place on ANALYSIS, D the dimension, S the first free set number) and the object scores rootD\_S (OBSCO), both as the normalization says; excluded cases and passive missing values are system-missing. OUTFILE writes the discretized codes (DISCRDATA, under the variables' names), the quantified variables TRAK\_D and the object scores OBSCOD.

## Example

```text
MULTIPLE CORRESPONDENCE pet garden commute /ANALYSIS=pet garden(WEIGHT=2) commute /DISCRETIZATION=commute(GROUPING NCAT=4) /MISSING=garden(ACTIVE EXTRACAT) /PRINT=DISCRIM QUANT OBJECT((household)) /PLOT=JOINTCAT /SAVE=OBJECT.
```

`statscraft help MULTIPLE CORRESPONDENCE` prints the same text in the terminal. [All commands](README.md)
