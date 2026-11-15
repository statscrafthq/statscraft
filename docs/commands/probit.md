<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# PROBIT

Probit and logit models of response counts out of subject counts, with effective levels.

## Syntax

```text
PROBIT responses OF subjects WITH covariates [BY factor(min,max)]
   [/MODEL=PROBIT | LOGIT | BOTH] [/LOG=[base | NONE]]
   [/CRITERIA=[OPTOL(n) | CONVERGE(n)] [ITERATE(n)] [P(0.15)] [STEPLIMIT(0.1)]]
   [/NATRES[=rate]] [/PRINT=[FREQ] [CI] [RMP] [PARALL] [DEFAULT] [ALL] [NONE]]
   [/MISSING=EXCLUDE | INCLUDE].
```

Each case is a group of subjects: how many there were and how many responded. The chance to respond is F(a + b'x), where F is the standard normal distribution (PROBIT, the default) or the logistic one (LOGIT); BOTH fits the logit model first, then the probit one. With a factor (BY, whole-number range; cases outside it are left out) each level has its own intercept and all share the slopes. Covariates are replaced by their base 10 logarithm unless LOG says otherwise: a bare LOG means base e, NONE no transformation. A case with a covariate of 0 is a control group; under a logarithm it stays out of the fit.

NATRES adds a natural response rate c, so the chance becomes c + (1 - c) F(...): alone it is estimated (starting from the control groups' rate, or the lowest observed rate), with a value it is fixed. Estimates are maximum likelihood by Newton steps from zero slopes, with standard errors from the observed information. More responses than subjects is an error; a case weight multiplies both counts.

Tables: Data Information, Convergence Information, Parameter Estimates (with z tests and 95% intervals), Natural Response Rate, Covariances and Correlations of Parameter Estimates and the Pearson goodness-of-fit test. PRINT left out or bare shows FREQ (observed and expected responses per case), CI (the covariate level at which each share from 0.01 to 0.99 respond, with 95% fiducial limits, on both scales under a logarithm) and RMP (the relative median potency of every pair of factor levels); PARALL tests whether the levels share the slope. CI, RMP and PARALL need a single covariate. When the goodness-of-fit test's significance is below P the limits widen by the heterogeneity factor chi-square/df and use t instead of the normal; P(0) never does that, P(1) always. With one covariate a scatter plot shows the transformed observed rates. MISSING=INCLUDE keeps user-missing values.

## Example

```text
PROBIT hatched OF eggs BY strain(1,3) WITH dose /MODEL=BOTH /PRINT=ALL.
```

`statscraft help PROBIT` prints the same text in the terminal. [All commands](README.md)
