<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# MIXED

Linear mixed model: fixed effects with random effects of subjects and correlated repeated measures.

## Syntax

```text
MIXED dependent [BY factors] [WITH covariates]
   [/FIXED=[effects] [| [NOINT] [SSTYPE(3 | 1)]]] [/METHOD=REML | ML] [/MISSING=EXCLUDE | INCLUDE]
   [/RANDOM=effects [| [SUBJECT(var[*var...])] [COVTYPE(type)] [SOLUTION]]] ...
   [/REPEATED=var[*var...] | SUBJECT(var[*var...]) [COVTYPE(type)]] [/REGWGT=var]
   [/CRITERIA=[CIN(95)] [MXITER(100)] [MXSTEP(10)] [SCORING(1)] [SINGULAR(1E-12)]
             [PCONVERGE(1E-6[, ABSOLUTE | RELATIVE])] [LCONVERGE(0[, ...])] [HCONVERGE(0[, ...])]
             [DFMETHOD(SATTERTHWAITE | RESIDUAL | KENWARDROGER)]]
   [/PRINT=[SOLUTION] [TESTCOV] [G] [R] [COVB] [CORB] [CPS] [DESCRIPTIVES] [HISTORY[(n)]] [LMATRIX]]
   [/SAVE=tempvar[(name)] ...]
   [/EMMEANS=TABLES(OVERALL | factor[*factor...]) [WITH(covariate=value | MEAN ...)]
             [COMPARE[(factor)] [REFCAT(value | FIRST | LAST)] [ADJ(LSD | BONFERRONI | SIDAK)]]] ...
   [/TEST[(constants)]=['label'] effect values ... [| effect values ...] [DIVISOR=n]; ...
          | ALL values ... [| values ...] ...] ...
```

The command fits a linear mixed model to one dependent. The fixed part is the intercept (unless NOINT) plus the FIXED effects, written as INTERCEPT (first), a factor or covariate, a\*b (a BY b) or a(b) (a WITHIN b). RANDOM, EMMEANS and TEST may repeat; for other subcommands the last one wins. A case missing on any variable named is left out (INCLUDE keeps user-missing values). Weights are rounded to frequency counts; REPEATED allows only a weight of 1. REGWGT divides the residual covariance of a case by its value.

Each RANDOM adds one covariance block per subject, the same for all; subjects and RANDOM subcommands are independent, and INTERCEPT is never implied there. Without SUBJECT the whole file is one subject. REPEATED orders the levels of its variables and gives each SUBJECT (required) a residual block; without it the residuals share one variance. A RANDOM subject variable must also be a REPEATED subject variable. COVTYPE is VC on RANDOM (one variance per effect) and DIAG on REPEATED (one variance per level; VC there means DIAG). Other types: ID (one variance), UN (unstructured), CS (a common covariance plus a diagonal offset), CSR (one variance, one correlation), AR1 (first-order autoregressive), ARMA11, TP (Toeplitz), AD1 (antedependence), UNR (a correlation per pair), HF (Huynh-Feldt), FA1 (one factor plus a diagonal offset). CSH, ARH1, TPH and FAH1 are CSR, AR1, TP and FA1 with a variance (FAH1: an offset) per level.

METHOD=REML (the default) maximizes the restricted likelihood, ML the full one. The fit takes up to MXITER Newton steps (the first SCORING with the expected Hessian), halving a step up to MXSTEP times. It stops when every parameter changes less than PCONVERGE, or by LCONVERGE or HCONVERGE when set (ABSOLUTE by default, or RELATIVE). CIN sets the interval level. DFMETHOD picks the df: Satterthwaite (the default), RESIDUAL (cases less the rank of the fixed design), or KENWARDROGER, which also adjusts the fixed standard errors and F tests.

Tables: Model Dimension, Information Criteria (-2 log likelihood, AIC, AICC, CAIC, BIC; for REML only covariance parameters count), Type III Tests of Fixed Effects (Type I with SSTYPE(1), each effect after those before it) and Estimates of Covariance Parameters. PRINT adds SOLUTION (fixed estimates), TESTCOV (Wald Z and intervals for covariance parameters), G and R (one subject's random and repeated covariance), COVB and CORB (fixed estimates, adjusted with KENWARDROGER), CPS (case counts), DESCRIPTIVES (cell statistics), HISTORY(n) (every n-th iteration, 1 by default, and the last) and LMATRIX (test coefficients). SOLUTION on RANDOM lists every subject's predicted random effects with tests and intervals.

EMMEANS averages the fitted cells equally over the factors left out, covariates at their means or the WITH values; each mean and difference gets its own df. COMPARE tests every pair of levels, or each level against REFCAT, with LSD, BONFERRONI or SIDAK adjustment, plus an F test of the compared factor at each level of the others.

TEST takes coefficients for the parameters of named fixed effects (redundant ones included), or ALL of them; rows are separated by semicolons. Constants in parentheses are the tested values (0 by default); DIVISOR divides a row. ALL and effect names never mix in one TEST: the first decides the form. After \| come coefficients for one subject's random effects, by effect or as bare values over the RANDOM subcommands in order; bare values fit either form, and an effect on two RANDOM subcommands needs them. With SUBJECT they are shared among subjects, so the row tests their average. Each row gets an estimate and t test, plus a joint F test; a row with a non-estimable fixed part is dropped with a warning. A TEST with random effects keeps Satterthwaite df even with KENWARDROGER.

SAVE adds casewise variables: FIXPRED (fixed-part prediction), PRED (with the predicted random effects), RESID (dependent minus PRED), SEFIXP and DFFIXP (standard error and df of FIXPRED), SEPRED and DFPRED (the same for PRED). Default names are FXPRED\_1, PRED\_1, RESID\_1, SEFIXP\_1 and so on (first free number); a name in parentheses replaces it. Each is saved once, and an existing name is an error.

## Example

```text
MIXED yield BY plot_type WITH rain /FIXED=plot_type rain /RANDOM=INTERCEPT rain | SUBJECT(farm) COVTYPE(UN) /PRINT=SOLUTION TESTCOV.
```

`statscraft help MIXED` prints the same text in the terminal. [All commands](README.md)
