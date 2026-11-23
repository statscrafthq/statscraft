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
   [/PMEANS[=[VARIABLES(dependents)] [TABLES(factor [BY factor] | CONSTANT, ...)] [PLOT]]]
   [/CINTERVAL=[INDIVIDUAL | JOINT][(level)]
              [UNIVARIATE(SCHEFFE | BONFER) | MULTIVARIATE(ROY | PILLAI | WILKS | HOTELLING | BONFER)]]
   [/POWER[=[F(alpha)] [T(alpha)] [APPROXIMATE | EXACT]]] [/RESIDUALS[=CASEWISE] [PLOT]]
   [/PLOT=[BOXPLOTS] [CELLPLOTS] [NORMAL] [ZCORR]]
   [/PCOMPS=[COR] [COV] [ROTATE[(VARIMAX | EQUAMAX | QUARTIMAX | NOROTATE)]] [NCOMP(n)]
            [MINEIGEN(value)] [ALL]]
   [/DISCRIM[=[RAW] [STAN] [ESTIM] [COR] [ROTATE[(VARIMAX | EQUAMAX | QUARTIMAX)]] [ALL]
             [ALPHA(value)]]]
   [/ANALYSIS[(CONDITIONAL | UNCONDITIONAL)]=dependents [WITH covariates]
      | (dependents [WITH covariates] [/ ...]) [WITH covariates]]
   [/MISSING=[LISTWISE] [EXCLUDE | INCLUDE]] [/MATRIX=IN(file | dataset | *) OUT(file | dataset | *)]
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

POWER adds two columns to every F test (alpha .05, or F(alpha)) and every t test of an estimate (two-sided, alpha .05, or T(alpha)): the noncentrality, F times its hypothesis df or \|t\|, and the observed power from the noncentral F or t, always computed exactly. In the multivariate tests, APPROXIMATE (the default) uses each criterion's eta squared over one less it, times the error df of its F, and gives Roys none; EXACT gives all four the noncentrality of Hotelling's T-squared, the trace times the error df, when the effect has one hypothesis df, and gives nothing otherwise.

With several dependents, CINTERVAL MULTIVARIATE widens the intervals of the estimates so they hold for all dependents together: PILLAI, WILKS and HOTELLING find the critical largest root through that statistic's F approximation, BONFER splits alpha over the dependents and the family. With INDIVIDUAL the family is one parameter, and ROY, PILLAI, WILKS and HOTELLING then all give Hotelling's T-squared interval; with JOINT it is the term's parameters. With one dependent MULTIVARIATE is ignored.

PCOMPS, with several dependents, finds the principal components of the error matrix after the covariates, as correlations (COR) or as variances and covariances (COV): the eigenvalues with their share of the total, and loadings (each eigenvector times the root of its eigenvalue) for the components whose eigenvalue reaches MINEIGEN (0 by default). ROTATE turns the first NCOMP of them (all by default, at least 2) by VARIMAX unless another type is named, with Kaiser normalization; a rotation carries over to later PCOMPS until NOROTATE. ALL means COR, COV and ROTATE.

DISCRIM describes every multivariate effect, and the covariates, by its discriminant functions: the eigenvectors of the hypothesis matrix relative to the error matrix, scaled to a variance of 1 within the error. Functions are kept in order while the dimension reduction test of a function and the ones after it has Sig. at most ALPHA (0.25 by default; 1 keeps all, 0 none). RAW gives the coefficients, STAN the coefficients times the error standard deviations, ESTIM the effect's parameter estimates (or the covariates' coefficients) times the coefficients, COR each dependent's correlation with each function within the error, ROTATE those correlations rotated when two or more functions are kept. DISCRIM alone gives RAW, STAN, ESTIM and COR; ALL also ROTATE.

ANALYSIS picks which variables are dependents and which covariates (also with WSFACTORS, where its dependents are the cells and its covariates after WITH the constant ones). Several groups in parentheses separated by slashes are separate analyses, each with its own tables; CONDITIONAL makes the variables of the earlier groups covariates of the later ones, and covariates after the closing parenthesis go to every group. A variable is named once on the subcommand.

MATRIX=OUT(file \| dataset \| \*) writes the cells and the pooled within-cells moments: records N (the total), N and MEAN of every non-empty cell with its factor values between ROWTYPE\_ and VARNAME\_, STDDEV (the root of the within-cells mean square) and CORR (the within-cells correlations) over the dependents and covariates. MATRIX=IN(file \| dataset \| \*) runs the analyses from such a file, with the tables the data would give. CELLINFO, HOMOGENEITY, RESIDUALS and PLOT need the cases and are refused with IN. A file CORRELATIONS wrote (N, MEAN, STDDEV, CORR) is one cell without factors.

PLOT draws, once for the command and from the cases, a box for every cell of each variable (BOXPLOTS; alone it is the default), the cell means against the cell variances and standard deviations and a histogram of the cell means (CELLPLOTS), the normal and detrended normal plots of each variable's scores (NORMAL, the i-th of n at the normal quantile of i / (n + 1)) and a half-normal plot of Fisher's z of the within-cells correlations of the dependents (ZCORR). PMEANS PLOT draws the adjusted predicted mean of each cell. RESIDUALS PLOT draws, for each dependent, the standardized residuals against the observed values, the predicted values and the case numbers, and their normal and detrended normal plots; RESIDUALS=PLOT alone leaves out the casewise listing. Each design draws its own, its number in the title.

## Example

```text
MANOVA score1 score2 BY method(1,3) school(1,2) WITH pretest /PRINT=SIGNIF(EIGEN) /DESIGN.
MANOVA t1 t2 t3 BY method(1,3) /WSFACTORS=trial(3) /PRINT=SIGNIF(GG HF) /DESIGN.
```

`statscraft help MANOVA` prints the same text in the terminal. [All commands](README.md)
