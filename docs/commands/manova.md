<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# MANOVA

Analysis of variance and covariance for one or several dependents, with nested and pooled terms, partitions and chosen error terms.

## Syntax

```text
MANOVA dependents [BY factors (min,max) [factors (min,max) ...]] [WITH covariates] [(covariates)]
   [/WSFACTORS=name [name ...] (levels) ...] [/WSDESIGN=term [term ...]] [/MEASURE=names]
   [/TRANSFORM[(varlist [/varlist ...])]=[ORTHONORM] [CONTRAST | BASIS] type] [/RENAME=name | * ...]
   [/ERROR=WITHIN | RESIDUAL | WR | n] [/METHOD=[UNIQUE | SEQUENTIAL] [CONSTANT | NOCONSTANT] [QR | CHOLESKY]]
   [/CONTRAST(factor)=DEVIATION[(ref)] | SIMPLE[(ref)] | DIFFERENCE | HELMERT | REPEATED |
                      POLYNOMIAL[(metric)] | SPECIAL(matrix)] ... [/PARTITION(factor)[=(n, m*k ...)]] ...
   [/PRINT | NOPRINT=[CELLINFO[(MEANS SSCP COV COR ALL)]] [HOMOGENEITY[(BARTLETT COCHRAN BOXM ALL)]]
         [PARAMETERS[(ESTIM NEGSUM ORTHO COR EFSIZE ALL)]] [DESIGN[(OVERALL ONEWAY)]] [TRANSFORM]
         [SIGNIF(MULTIV UNIV EIGEN DIMENR HYPOTH STEPDOWN BRIEF SINGLEDF AVERF AVONLY GG HF EFSIZE)]
         [ERROR[(SSCP COV COR STDDEV)]]]
   [/OMEANS=[VARIABLES(varlist)] TABLES(factor [BY factor] | CONSTANT, ...)]
   [/PMEANS[=[VARIABLES(dependents)] [TABLES(factor [BY factor] | CONSTANT, ...)]]]
   [/CINTERVAL=[INDIVIDUAL | JOINT][(level)] [UNIVARIATE(SCHEFFE | BONFER)]] [/RESIDUALS[=CASEWISE]]
   [/ANALYSIS=dependents [WITH covariates]] [/MISSING=[LISTWISE] [EXCLUDE | INCLUDE]]
   [/DESIGN=term [term ...]] [/DESIGN ...].
```

term: effect \[+ effect ...\] \[= n\] \[VS WITHIN \| RESIDUAL \| WR \| n\], or CONSTANT, or MUPLUS before a term; effect: item \[BY item ...\] \[WITHIN item \[BY item ...\] ...\], or MWITHIN factor(n); item: factor\[(n)\], a continuous variable or POOL(varlist).

Factors take every whole number of their range (values are cut to whole numbers); a case outside a range or missing on any listed variable is left out. Case weights are used as they are. Settings carry over to every later DESIGN; when the command does not end with DESIGN, a full factorial design (all main effects and interactions) is added. Each factor is coded by its contrast (DEVIATION by default, the last level left out): the design columns are the inverse of the matrix whose first row is the level mean and whose other rows are the contrasts, so each parameter estimates its contrast. In a term, a(n) keeps partition n of the factor's degrees of freedom (one each, or as PARTITION groups them); after WITHIN (or W, or MWITHIN) a factor stands for each of its levels, or level n alone for b(n), and a term that starts with MWITHIN b(n) is the constant at that level; '+' pools effects into one test; '= n' makes the term error term n (1 to 10); VS (or AGAINST) names the error a term is tested against. A continuous variable on DESIGN must be a dependent or covariate that ANALYSIS leaves out. The constant is fitted first and shown only when written as CONSTANT (or when there are no factors); NOCONSTANT drops it unless written.

UNIQUE sums of squares adjust each term for all others; SEQUENTIAL for the terms before it.

Error terms: WITHIN, the pooled within-cell matrix (df N less the non-empty cells); RESIDUAL, the part of the cell model the design leaves out; WR, both together; n, a numbered term. The default is WITHIN, RESIDUAL when no cell holds two cases, WR when DESIGN has a continuous variable.

Covariates are taken out of every test by regression within the error term, which loses a df for each, and are tested as a regression with their coefficients.

TRANSFORM replaces the continuous variables (all, or each list in turn with one matrix) by combinations named T1, T2 and so on: with CONTRAST the first is their mean and the others the contrasts of the type, SPECIAL takes the square matrix as written, BASIS uses the transposed inverse of that matrix, ORTHONORM makes its rows orthonormal and drops a row the others span.

RENAME gives every continuous variable of the list a name in order ('\*' keeps the old one); ANALYSIS and later subcommands use the new names, OMEANS the original variables.

Repeated measures: WSFACTORS (first after the variable list) reads the dependents as the cells of the within-subjects factors, the last factor changing fastest, once per measure (MEASURE names them). Each factor's contrast (POLYNOMIAL unless CONTRAST says otherwise) is made orthonormal together with the mean, and the dependents are replaced by their Kronecker products. The between-subjects tests analyze the mean column; each WSDESIGN term (by default every main effect and interaction) is the constant of an analysis of its own columns, the DESIGN terms crossed with it. Covariates outside parentheses come in sets, one per within-subjects cell, and are transformed the same way; those in parentheses stay as they are. An effect with more than one column gets Mauchly's test with the Greenhouse-Geisser, Huynh-Feldt and lower-bound epsilons, and averaged tests (AVERF, by default): the sums of the hypothesis and error sums of squares over its columns with the df times their number, per measure, and with several measures their multivariate tests.

GG and HF add the significance with both df times the epsilon; AVONLY drops the multivariate tests of such effects. SIGNIF(UNIV) is off by default with WSFACTORS.

Tables: Cases; with one dependent, Tests of Significance (each error term and the terms tested against it, the regression, (Model), (Total), R-squared); with several, Multivariate Tests of Significance (Pillais, Wilks, Hotellings with their F approximations, and Roys as the largest root over one plus it) and Univariate F-tests. SIGNIF adds Eigenvalues and Canonical Correlations (EIGEN), Dimension Reduction Analysis (DIMENR), the Hypothesis SSCP Matrix (HYPOTH), Roy-Bargmann Stepdown F-tests (STEPDOWN), a test for each degree of freedom of a term, each column entered after the ones before it (SINGLEDF), partial eta squared for every F and t test (EFSIZE), or only a Wilks table (BRIEF); NOPRINT SIGNIF(MULTIV) or (UNIV) drops a default table. PARAMETERS gives each design column's estimate with its standard error from the term's error term, t, Sig. and an interval (CINTERVAL: INDIVIDUAL at the level given, 0.95 by default; JOINT with Scheffe or Bonferroni multipliers over the term's parameters), NEGSUM minus their sum per term, ORTHO the orthogonal estimates of the design columns in order (their squares add up to the sequential sums of squares), COR the covariance factors and correlations of the estimates; ERROR the error matrices after the covariates; TRANSFORM the transformation matrix; DESIGN the design matrix of the cells (OVERALL) and each factor's one-way basis (ONEWAY). CELLINFO gives cell means with 95% intervals and the within-cell matrices; HOMOGENEITY Cochran's C, the Bartlett-Box F and Box's M; OMEANS weighted and unweighted means over the factors named; PMEANS observed, adjusted and estimated cell means with residuals, for the VARIABLES named, and with TABLES the adjusted means over the other factors, weighted by cell size and not; RESIDUALS each case's observed and predicted values and residuals.

Not supported yet: POWER, PLOT, PCOMPS, DISCRIM, MATRIX, CINTERVAL MULTIVARIATE, RESIDUALS PLOT, PMEANS PLOT, PRINT DESIGN other than OVERALL and ONEWAY, PARAMETERS(OPTIMAL), ANALYSIS with WSFACTORS, and groups of analyses on ANALYSIS.

## Example

```text
MANOVA score1 score2 BY method(1,3) school(1,2) WITH pretest /PRINT=SIGNIF(EIGEN) /DESIGN.
MANOVA t1 t2 t3 BY method(1,3) /WSFACTORS=trial(3) /PRINT=SIGNIF(GG HF) /DESIGN.
```

`statscraft help MANOVA` prints the same text in the terminal. [All commands](README.md)
