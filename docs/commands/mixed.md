<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# MIXED

Linear mixed model: fixed effects with random effects of subjects and correlated repeated measures.

## Syntax

```text
MIXED dependent [BY factors] [WITH covariates]
   [/FIXED=[effects] [| [NOINT] [SSTYPE(3)]]] [/METHOD=REML | ML] [/MISSING=EXCLUDE | INCLUDE]
   [/RANDOM=effects [| [SUBJECT(var[*var...])] [COVTYPE(type)]]] ...
   [/REPEATED=var[*var...] | SUBJECT(var[*var...]) [COVTYPE(type)]] [/REGWGT=var]
   [/CRITERIA=[CIN(95)] [MXITER(100)] [MXSTEP(10)] [SCORING(1)] [SINGULAR(1E-12)]
             [PCONVERGE(1E-6[, ABSOLUTE | RELATIVE])] [LCONVERGE(0[, ...])] [HCONVERGE(0[, ...])]
             [DFMETHOD(SATTERTHWAITE | RESIDUAL)]]
   [/PRINT=[SOLUTION] [TESTCOV] [G] [R] [COVB] [CORB]] [/SAVE=tempvar[(name)] ...]
   [/EMMEANS=TABLES(OVERALL | factor[*factor...]) [WITH(covariate=value | MEAN ...)]
             [COMPARE[(factor)] [REFCAT(value | FIRST | LAST)] [ADJ(LSD | BONFERRONI | SIDAK)]]] ...
   [/TEST[(constants)]=['label'] effect values ... [DIVISOR=n]; ... | ALL values ...] ...
```

effects: INTERCEPT (first), a factor or covariate, a\*b or a BY b, a(b) or a WITHIN b.

type: VC (the RANDOM default: one variance per effect), ID (one variance for the block), DIAG (the REPEATED default: one variance per level), UN (every variance and covariance), CS (a diagonal offset plus one common covariance), AR1 (a variance and a correlation that is raised to the power of the distance between levels). VC on REPEATED is DIAG. tempvar: FIXPRED (X b), PRED (X b plus the predicted random effects), RESID (dependent minus PRED), SEFIXP and DFFIXP (the standard error and degrees of freedom of FIXPRED).

The fixed part is the intercept unless NOINT, plus the FIXED effects. Each RANDOM adds one covariance block per subject, the same for every subject; subjects are independent, and so are the RANDOM subcommands. Without SUBJECT the whole file is one subject. REPEATED orders the levels of its variables and gives each subject a residual block of that structure; without it the residuals have one variance. Each subject variable of RANDOM must be a subject variable of REPEATED. A case missing on any variable named is left out; the weight is rounded and counts the case that many times (with REPEATED only a weight of 1). REGWGT divides the residual covariance of a case by its value. The covariance parameters maximize the restricted (REML) or full (ML) likelihood by Newton steps, the first SCORING ones with the expected Hessian, halving a step up to MXSTEP times when it fails to improve; the fit stops when the change of every parameter is below PCONVERGE, or by LCONVERGE or HCONVERGE when set. Tables: Model Dimension, Information Criteria (-2 log likelihood, AIC, AICC, CAIC, BIC; for REML n is the cases less the fixed parameters and only covariance parameters count), Type III Tests of Fixed Effects, Estimates of Fixed Effects (SOLUTION) and Estimates of Covariance Parameters (with TESTCOV the standard errors from the observed Hessian, Wald Z and intervals on the log scale for variances and the Fisher z scale for correlations). Degrees of freedom are Satterthwaite's, or the cases less the rank of the fixed design with DFMETHOD(RESIDUAL). EMMEANS averages the fitted cells equally over the factors left out, covariates at their means or the WITH values; each mean and each difference gets its own degrees of freedom. COMPARE lists every pair of levels, or each level against REFCAT, with Bonferroni or Sidak adjustment over that number of comparisons, and an F test of the compared factor at each level of the others. TEST takes coefficients for the parameters of named fixed effects (redundant ones included) or ALL of them, one row per combination; the constants in parentheses are the values tested against (0 by default) and DIVISOR divides a row. It gives each row's estimate with a t test and the joint F test; a row that is not estimable is dropped with a warning. PRINT G and R show the covariance of one subject's random effects and repeated levels, COVB and CORB the covariance and correlation of the fixed estimates. Not supported yet: TEST coefficients for random effects, SSTYPE(1), DFMETHOD(KENWARDROGER), COVTYPE AD1, ARH1, ARMA11, CSH, CSR, FA1, FAH1, HF, TP, TPH, UNR and the spatial and Kronecker types, KRONECKER, SPCOORDS, SOLUTION on RANDOM, PRINT CPS, DESCRIPTIVES, HISTORY and LMATRIX, SAVE SEPRED and DFPRED.

## Example

```text
MIXED yield BY plot_type WITH rain /FIXED=plot_type rain /RANDOM=INTERCEPT rain | SUBJECT(farm) COVTYPE(UN) /PRINT=SOLUTION TESTCOV.
```

`statscraft help MIXED` prints the same text in the terminal. [All commands](README.md)
