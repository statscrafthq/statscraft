<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# GENLOG

General loglinear and logit models of a table of counts, with cell weights, covariates and contrasts.

## Syntax

```text
GENLOG factors [BY factors] [WITH covariates]
   [/CSTRUCTURE=variable] [/GRESID=varlist] [/GLOR=varlist] [/MODEL=POISSON | MULTINOMIAL]
   [/CRITERIA=[CONVERGE(0.001)] [ITERATE(20)] [DELTA(0.5)] [CIN(95)] [EPS(1E-8)] [DEFAULT]]
   [/PRINT=[FREQ] [RESID] [ZRESID] [ADJRESID] [DEV] [ITERATION] [COV] [DESIGN] [ESTIM] [CORR]
          [ALL] [DEFAULT] [NONE]]
   [/PLOT=DEFAULT | RESID[(ADJRESID DEV)] | NORMPROB[(ADJRESID DEV)] | NONE]
   [/SAVE=[RESID [(name)]] [ZRESID [(name)]] [ADJRESID [(name)]] [DEV [(name)]] [PRED [(name)]]]
   [/MISSING=EXCLUDE | INCLUDE] [/DESIGN=term [term ...]].
```

terms: a factor, a(n) for category n of a factor alone, a covariate, or parts joined by \* or BY. Factors are numeric and their categories are the values that occur; the table has a cell for every combination, at most ten factors. The factors after BY make a logit model, which needs MULTINOMIAL. A cell's count sums the case weights; its covariates, cell weight (CSTRUCTURE) and GRESID and GLOR coefficients are weighted means over its cases. A cell weight of 0 or less makes a structural zero, which is left out of the fit; other cell weights act as multipliers of the expected count. Without DESIGN the model is saturated, which is not allowed with covariates. Dummy coding with a constant: a factor has a column for each category, and columns that repeat earlier ones (by EPS) are redundant and 0. Under MULTINOMIAL the constant, and in a logit model the terms of the BY factors alone, only fix the totals.

Newton-Raphson starts from the counts plus DELTA (a saturated model keeps DELTA in the counts) and stops when the fitted values move by less than CONVERGE and the score is below it, or after ITERATE steps. Tables: Data Information, Model Information, Convergence Information (ITERATION adds every step), likelihood-ratio and Pearson Goodness-of-Fit Tests, Cell Counts and Residuals with the columns FREQ, RESID, ZRESID, ADJRESID and DEV ask for (FREQ, RESID, ADJRESID and DEV by default; NONE leaves the table out), the Design Matrix, Parameter Estimates with Wald z and the CIN interval (ESTIM), and their covariances (COV) and correlations (CORR). GRESID gives, for each coefficient variable, the observed and expected sums and their simple, standardized and adjusted residuals; GLOR tests each contrast of log expected counts (coefficients summing to 0, per BY combination in a logit model) with a Wald chi-square and the CIN interval. A logit model adds the analysis of dispersion with the entropy and concentration measures of association. PLOT draws adjusted (or deviance) residuals against the observed and expected counts and in normal and detrended normal plots; by default the adjusted ones in both. SAVE gives each case the value of its cell: residual (RES\_n), standardized (ZRE\_n), adjusted (ADJ\_n), deviance (DEV\_n) residual or expected count (PRE\_n), or the name given.

## Example

```text
GENLOG outcome BY clinic shift /MODEL=MULTINOMIAL /PRINT=FREQ ADJRESID ESTIM /DESIGN=outcome outcome*clinic outcome*shift.
```

`statscraft help GENLOG` prints the same text in the terminal. [All commands](README.md)
