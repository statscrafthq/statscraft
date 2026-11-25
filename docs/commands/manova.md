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
         [PARAMETERS[(ESTIM NEGSUM ORTHO COR EFSIZE ALL)]] [DESIGN[(OVERALL ONEWAY DECOMP COLLINEARITY)]] [TRANSFORM]
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

The command fits a linear model to one or more dependents, with factors after BY and covariates after WITH. A range (min,max) applies to every factor before it, with one level per whole number; values are cut to whole numbers. Cases outside a range or missing on any listed variable are left out (MISSING=INCLUDE keeps user-missing values). Weights are used as they are. Each DESIGN runs one analysis with the settings given before it, which carry over. Without a final DESIGN, a full factorial design is added. With one dependent the output is Tests of Significance (each error term with its terms, the covariate regression, (Model), (Total), R-squared); with several, Multivariate Tests of Significance (Pillai, Wilks, Hotelling, Roy) and Univariate F-tests.

Design terms, separated by spaces or commas: an item is a factor, a continuous variable or POOL(varlist); items are crossed with BY and nested with WITHIN (or W). A factor written a(n) keeps partition n of its df, one df each or as PARTITION(a)=(n, m\*k) groups them. After WITHIN, b(n) means level n of b alone; a term that starts with MWITHIN b(n) is the constant at that level. A plus sign pools effects into one test, '= n' makes the term error term n (1 to 10), VS (or AGAINST, VERSUS) names its error, and MUPLUS joins the constant to the next term. A continuous variable on DESIGN must be a dependent or covariate that ANALYSIS leaves out. The constant is fitted first but shown only when written as CONSTANT or without factors; METHOD=NOCONSTANT drops it unless written.

ERROR picks the error term: WITHIN (W), the pooled within-cell matrix; RESIDUAL (R), what the design leaves out of the cell model; WR (RW), both; or a numbered term. The default is WITHIN, RESIDUAL when no cell holds two cases, and WR when DESIGN has a continuous variable. METHOD=UNIQUE (the default) adjusts each term for all others, SEQUENTIAL for the terms before it. CONTRAST(factor) sets the coding, DEVIATION by default (last level left out). Covariates are removed from every test within the error term (one df each) and tested as a regression.

TRANSFORM replaces the continuous variables (all, or each list in turn) by T1, T2 and so on, from a contrast type whose first row is the mean (SPECIAL takes a square matrix); BASIS uses its transposed inverse, ORTHONORM makes the rows orthonormal. RENAME names every continuous variable in order ('\*' keeps a name); later subcommands use the new names, OMEANS the originals. ANALYSIS picks the dependents and covariates. Groups in parentheses, separated by slashes, are separate analyses; CONDITIONAL makes earlier groups covariates of later ones, and covariates after the closing parenthesis go to every group.

Repeated measures: WSFACTORS, the first subcommand, reads the dependents as the cells of the within-subjects factors (2 or more levels each), the last changing fastest, once per measure named on MEASURE. Contrasts are POLYNOMIAL unless CONTRAST says otherwise; between-subjects tests use the cell mean. WSDESIGN lists within-subjects terms (all main effects and interactions by default), each crossed with the DESIGN terms. Covariates outside parentheses come one per cell; those in parentheses are constant. An effect with several df gets Mauchly's test with Greenhouse-Geisser, Huynh-Feldt and lower-bound epsilons, and averaged F tests (AVERF, the default). GG and HF add tests with epsilon-scaled df; AVONLY drops the multivariate tests. SIGNIF(UNIV) is off by default here.

PRINT adds tables, NOPRINT removes them; a keyword alone means its defaults. SIGNIF adds EIGEN (eigenvalues, canonical correlations), DIMENR (dimension reduction), HYPOTH (hypothesis SSCP), STEPDOWN (stepdown F), SINGLEDF (a test per df), EFSIZE (partial eta squared, also under PARAMETERS) or BRIEF (Wilks only); MULTIV and UNIV are on by default. PARAMETERS gives estimates with standard error, t, Sig. and interval (ESTIM, default), NEGSUM (minus their sum), ORTHO (orthogonal estimates) or COR (their correlations). ERROR prints error matrices (COV, COR, STDDEV by default, or SSCP), TRANSFORM the transformation matrix, DESIGN the design matrix (OVERALL, default), ONEWAY bases, DECOMP the upper triangular factor R of the weighted design (R'R = X'WX, positive diagonal, a zero row for a redundant column) or COLLINEARITY the singular values of the design with its columns scaled to unit length, their condition indexes and the share of each parameter's variance that goes with each singular value. CELLINFO gives cell MEANS (default), SSCP, COV or COR; HOMOGENEITY gives BARTLETT, COCHRAN and BOXM (all by default).

OMEANS gives weighted and unweighted means of the VARIABLES over the factors on TABLES; alone it gives cell means. PMEANS gives observed, adjusted and estimated cell means with residuals, and with TABLES adjusted means over the other factors. RESIDUALS lists each case's predictions and residuals. CINTERVAL sets estimate intervals: INDIVIDUAL at the level given (0.95 by default), or JOINT over a term's parameters by SCHEFFE or BONFER. MULTIVARIATE widens them to cover all dependents (ignored with one). POWER adds noncentrality and observed power to F tests (alpha F(.05)) and t tests (T(.05)); multivariate tests use APPROXIMATE power, or EXACT power only for one-df effects.

With several dependents, PCOMPS gives principal components of the error matrix as COR or COV, for eigenvalues of at least MINEIGEN (0). ROTATE turns the first NCOMP (all) by VARIMAX unless named, until NOROTATE. ALL means COR, COV and ROTATE. DISCRIM gives discriminant functions of each multivariate effect and the covariates, kept while their dimension reduction Sig. is at most ALPHA (0.25). It shows RAW and STAN coefficients, ESTIM (estimates times coefficients) and COR (dependent-function correlations), all four by default; ALL adds ROTATE.

MATRIX=OUT writes N (total and per cell), each cell's MEAN with its factor values between ROWTYPE\_ and VARNAME\_, and the within-cell STDDEV and CORR. MATRIX=IN analyzes such a file, or a CORRELATIONS file as one cell; CELLINFO, HOMOGENEITY, RESIDUALS and PLOT are refused then. PLOT draws, once per command, BOXPLOTS (the default), CELLPLOTS (cell means against spreads), NORMAL (normal and detrended plots) and ZCORR (half-normal plot of within-cell correlations). PMEANS PLOT draws adjusted predicted cell means; RESIDUALS PLOT draws standardized residuals against observed, predicted and case number, with normal plots. RESIDUALS=PLOT alone skips the listing.

## Example

```text
MANOVA score1 score2 BY method(1,3) school(1,2) WITH pretest /PRINT=SIGNIF(EIGEN) /DESIGN.
MANOVA t1 t2 t3 BY method(1,3) /WSFACTORS=trial(3) /PRINT=SIGNIF(GG HF) /DESIGN.
```

`statscraft help MANOVA` prints the same text in the terminal. [All commands](README.md)
