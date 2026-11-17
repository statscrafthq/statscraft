<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# VARCOMP

Variance components: how much each random effect of a linear model adds to the variance of the dependent variable.

## Syntax

```text
VARCOMP dependent BY factors [WITH covariates] /RANDOM=factor [factor ...] ...
   [/METHOD=MINQUE[(1 | 0)] | ML | REML | SSTYPE[(3 | 1)]] [/INTERCEPT=INCLUDE | EXCLUDE]
   [/MISSING=EXCLUDE | INCLUDE] [/REGWGT=var]
   [/CRITERIA=[CONVERGE(1E-8)] [EPS(1E-8)] [ITERATE(50)]]
   [/PRINT=[EMS] [SS] [HISTORY[(1)]]]
   [/OUTFILE=[VAREST] [COVB | CORB] ('file' | dataset)]
   [/DESIGN=[INTERCEPT] [effect ...]]
```

effects: a factor or covariate, a\*b or a BY b, a(b) or a WITHIN b.

Every effect that holds a factor named on RANDOM is random, with its own variance; the other effects are fixed. RANDOM may repeat and its factors add up. Without DESIGN the model has the intercept, the covariates and every main effect and interaction of the factors.

The design is coded as in UNIANOVA: one indicator column per level combination that occurs.

METHOD picks the estimator. MINQUE (the default) solves the minimum norm quadratic unbiased equations with prior weight 1 for every random effect and the residual, or with 0 for the random effects and 1 for the residual. ML and REML maximize the full or the restricted likelihood, starting from the variance of each effect's least squares estimates and the residual variance of the full model; Fisher scoring takes the first step and Newton steps follow while the Hessian allows, a step is halved up to 10 times until the likelihood rises, a variance that would turn negative is set to 0, and the fit stops when both the likelihood and the parameters change by less than CONVERGE (relative) or after ITERATE iterations. SSTYPE equates the mean square of each random effect and of the error, by Type I or III sums of squares, with its expected value and solves for the variances; these may come out negative. A case missing on any variable is left out; INCLUDE keeps user-missing values. The case weight is rounded and counts the case that many times; REGWGT weights the residual of each case (cases with a weight of 0 or less are left out). EPS is the tolerance for redundant design columns. Tables: Factor Level Information, Variance Estimates, with ML and REML the Asymptotic Covariance Matrix of the estimates (the inverse of the expected information); PRINT SS shows the sums of squares, df and mean squares and PRINT EMS the expected mean squares (SSTYPE only; the fixed effects that enter appear as a quadratic term); HISTORY(n) lists the log likelihood and the estimates at every n-th iteration and the last (ML and REML only). OUTFILE writes one row per component with its label (VarName\_), its estimate (Estimate, with VAREST) and its row of the covariance or correlation matrix (COVB or CORB, ML and REML only) in columns VC1, VC2, ...

## Example

```text
VARCOMP strength BY mixture batch operator /RANDOM=batch operator /METHOD=REML /PRINT=HISTORY.
```

`statscraft help VARCOMP` prints the same text in the terminal. [All commands](README.md)
