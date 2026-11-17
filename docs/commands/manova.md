<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# MANOVA

Analysis of variance and covariance for one or several dependents, with nested and pooled terms, partitions and chosen error terms.

## Syntax

```text
MANOVA dependents [BY factors (min,max) [factors (min,max) ...]] [WITH covariates]
   [/ERROR=WITHIN | RESIDUAL | WR | n] [/METHOD=[UNIQUE | SEQUENTIAL] [CONSTANT | NOCONSTANT] [QR | CHOLESKY]]
   [/CONTRAST(factor)=DEVIATION[(ref)] | SIMPLE[(ref)] | DIFFERENCE | HELMERT | REPEATED |
                      POLYNOMIAL[(metric)] | SPECIAL(matrix)] ... [/PARTITION(factor)[=(n, m*k ...)]] ...
   [/PRINT | NOPRINT=[CELLINFO[(MEANS SSCP COV COR ALL)]] [HOMOGENEITY[(BARTLETT COCHRAN BOXM ALL)]]
         [PARAMETERS[(ESTIM NEGSUM ALL)]] [SIGNIF(MULTIV UNIV EIGEN DIMENR HYPOTH STEPDOWN BRIEF)]
         [ERROR[(SSCP COV COR STDDEV)]]]
   [/OMEANS=[VARIABLES(varlist)] TABLES(factor [BY factor] | CONSTANT, ...)] [/PMEANS]
   [/ANALYSIS=dependents [WITH covariates]] [/MISSING=[LISTWISE] [EXCLUDE | INCLUDE]]
   [/DESIGN=term [term ...]] [/DESIGN ...].
```

term: effect \[+ effect ...\] \[= n\] \[VS WITHIN \| RESIDUAL \| WR \| n\], or CONSTANT, or MUPLUS before a term; effect: item \[BY item ...\] \[WITHIN item \[BY item ...\] ...\]; item: factor\[(n)\], a continuous variable or POOL(varlist).

Factors take every whole number of their range (values are cut to whole numbers); a case outside a range or missing on any listed variable is left out. Case weights are used as they are. Settings carry over to every later DESIGN; when the command does not end with DESIGN, a full factorial design (all main effects and interactions) is added. Each factor is coded by its contrast (DEVIATION by default, the last level left out): the design columns are the inverse of the matrix whose first row is the level mean and whose other rows are the contrasts, so each parameter estimates its contrast. In a term, a(n) keeps partition n of the factor's degrees of freedom (one each, or as PARTITION groups them); after WITHIN (or W) a factor stands for each of its levels, or level n alone for b(n); '+' pools effects into one test; '= n' makes the term error term n (1 to 10); VS (or AGAINST) names the error a term is tested against. A continuous variable on DESIGN must be a dependent or covariate that ANALYSIS leaves out. The constant is fitted first and shown only when written as CONSTANT (or when there are no factors); NOCONSTANT drops it unless written.

UNIQUE sums of squares adjust each term for all others; SEQUENTIAL for the terms before it.

Error terms: WITHIN, the pooled within-cell matrix (df N less the non-empty cells); RESIDUAL, the part of the cell model the design leaves out; WR, both together; n, a numbered term. The default is WITHIN, RESIDUAL when no cell holds two cases, WR when DESIGN has a continuous variable.

Covariates are taken out of every test by regression within the error term, which loses a df for each, and are tested as a regression with their coefficients.

Tables: Cases; with one dependent, Tests of Significance (each error term and the terms tested against it, the regression, (Model), (Total), R-squared); with several, Multivariate Tests of Significance (Pillais, Wilks, Hotellings with their F approximations, and Roys as the largest root over one plus it) and Univariate F-tests. SIGNIF adds Eigenvalues and Canonical Correlations (EIGEN), Dimension Reduction Analysis (DIMENR), the Hypothesis SSCP Matrix (HYPOTH), Roy-Bargmann Stepdown F-tests (STEPDOWN), or only a Wilks table (BRIEF); NOPRINT SIGNIF(MULTIV) or (UNIV) drops a default table. PARAMETERS gives each design column's estimate with its standard error from the term's error term, t, Sig. and 95% limits, NEGSUM minus their sum per term; ERROR the error matrices after the covariates. CELLINFO gives cell means with 95% intervals and the within-cell matrices; HOMOGENEITY Cochran's C, the Bartlett-Box F and Box's M; OMEANS weighted and unweighted means over the factors named; PMEANS observed, adjusted and estimated cell means with residuals. Not supported yet: repeated measures (WSFACTORS, WSDESIGN, MEASURE, MWITHIN), TRANSFORM, RENAME, RESIDUALS, POWER, CINTERVAL, PLOT, PCOMPS, DISCRIM, MATRIX, PRINT DESIGN and TRANSFORM, PARAMETERS ORTHO, COR, EFSIZE and OPTIMAL, SIGNIF SINGLEDF, PMEANS keywords, and groups of analyses on ANALYSIS.

## Example

```text
MANOVA score1 score2 BY method(1,3) school(1,2) WITH pretest /PRINT=SIGNIF(EIGEN) /DESIGN.
```

`statscraft help MANOVA` prints the same text in the terminal. [All commands](README.md)
