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

effects: INTERCEPT (first), a factor or covariate, a\*b or a BY b, a(b) or a WITHIN b.

type: VC (the RANDOM default: one variance per effect), ID (one variance for the block), DIAG (the REPEATED default: one variance per level), UN (every variance and covariance), CS (a diagonal offset plus one common covariance), CSR (one variance and one correlation), CSH (a variance per level and one correlation), AR1 (a variance and a correlation that is raised to the power of the distance between levels), ARH1 (the same with a variance per level), ARMA11 (a variance, a correlation gamma between neighbours and a factor rho for each step further), TP and TPH (a correlation for each distance, with one variance or a variance per level), AD1 (a variance per level; two levels correlate by the product of the neighbour correlations between them), UNR (a variance per level and a correlation per pair), HF (a variance per level; two levels have their mean variance less lambda as covariance), FA1 and FAH1 (the loadings of one factor plus a diagonal offset, common or per level). Variances and offsets stay at least 0 and correlations between -1 and 1. VC on REPEATED is DIAG. tempvar: FIXPRED (X b), PRED (X b plus the predicted random effects), RESID (dependent minus PRED), SEFIXP and DFFIXP (the standard error and degrees of freedom of FIXPRED), SEPRED and DFPRED (the same for PRED, counting the prediction error of the random effects).

The fixed part is the intercept unless NOINT, plus the FIXED effects. Each RANDOM adds one covariance block per subject, the same for every subject; subjects are independent, and so are the RANDOM subcommands. Without SUBJECT the whole file is one subject. REPEATED orders the levels of its variables and gives each subject a residual block of that structure; without it the residuals have one variance. Each subject variable of RANDOM must be a subject variable of REPEATED. A case missing on any variable named is left out; the weight is rounded and counts the case that many times (with REPEATED only a weight of 1). REGWGT divides the residual covariance of a case by its value. The covariance parameters maximize the restricted (REML) or full (ML) likelihood by Newton steps, the first SCORING ones with the expected Hessian, halving a step up to MXSTEP times when it fails to improve; the fit stops when the change of every parameter is below PCONVERGE, or by LCONVERGE or HCONVERGE when set. Tables: Model Dimension, Information Criteria (-2 log likelihood, AIC, AICC, CAIC, BIC; for REML n is the cases less the fixed parameters and only covariance parameters count), Type III Tests of Fixed Effects (Type I with SSTYPE(1): each effect after the effects listed before it), Estimates of Fixed Effects (SOLUTION) and Estimates of Covariance Parameters (with TESTCOV the standard errors from the observed Hessian, Wald Z and intervals on the log scale for variances and the Fisher z scale for correlations). Degrees of freedom are Satterthwaite's, or the cases less the rank of the fixed design with DFMETHOD(RESIDUAL). DFMETHOD(KENWARDROGER) gives every standard error and test of fixed effects the Kenward-Roger adjusted covariance, and each F its scale and denominator df from that method, built on the expected information of the covariance parameters. EMMEANS averages the fitted cells equally over the factors left out, covariates at their means or the WITH values; each mean and each difference gets its own degrees of freedom. COMPARE lists every pair of levels, or each level against REFCAT, with Bonferroni or Sidak adjustment over that number of comparisons, and an F test of the compared factor at each level of the others. TEST takes coefficients for the parameters of named fixed effects (redundant ones included) or ALL of them, one row per combination; the constants in parentheses are the values tested against (0 by default) and DIVISOR divides a row. After \| come coefficients for one subject's random effects, by effect or as bare values (or ALL) for all of them over the RANDOM subcommands in order; an effect on two RANDOM subcommands needs the bare values. With SUBJECT they are shared equally among the subjects, so the row speaks of their average. TEST gives each row's estimate with a t test and the joint F test; a row whose fixed part is not estimable is dropped with a warning. SOLUTION on RANDOM lists the predicted random effects of every subject with the standard error of their prediction error, df, a t test and an interval. Rows with random effects keep the unadjusted covariance and Satterthwaite's df (from numerical derivatives) even with KENWARDROGER. PRINT G and R show the covariance of one subject's random effects and repeated levels, COVB and CORB the covariance and correlation of the fixed estimates (adjusted with KENWARDROGER). CPS counts the cases at each value of the factors and of the repeated and subject variables; DESCRIPTIVES gives the count, mean and standard deviation of the dependent and covariates in each cell of the factors; HISTORY(n) lists the start and every n-th iteration (update type, step halvings, -2 log likelihood and every parameter) and always the last; LMATRIX shows the coefficients of each test of a fixed effect and of each TEST. Not supported yet: the spatial and Kronecker types of REPEATED, KRONECKER and SPCOORDS.

## Example

```text
MIXED yield BY plot_type WITH rain /FIXED=plot_type rain /RANDOM=INTERCEPT rain | SUBJECT(farm) COVTYPE(UN) /PRINT=SOLUTION TESTCOV.
```

`statscraft help MIXED` prints the same text in the terminal. [All commands](README.md)
