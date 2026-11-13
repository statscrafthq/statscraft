<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# PLUM

Ordinal regression: a cumulative link model of an ordered dependent variable.

## Syntax

```text
PLUM dependent [BY factors] [WITH covariates]
   [/LINK=LOGIT | PROBIT | CLOGLOG | NLOGLOG | CAUCHIT] [/LOCATION=effects] [/SCALE=effects]
   [/CRITERIA=[CIN(95)] [DELTA(0)] [BIAS(0)] [MXITER(100)] [MXSTEP(5)] [LCONVERGE(0)]
              [PCONVERGE(1E-6)] [SINGULAR(1E-8)]] [/MISSING=EXCLUDE | INCLUDE]
   [/PRINT=[FIT] [SUMMARY] [PARAMETER] [TPARALLEL] [CELLINFO] [COVB] [CORB] [HISTORY[(n)]] [KERNEL]]
   [/SAVE=[ESTPROB[(root[:n])]] [PREDCAT[(name)]] [PCPROB[(name)]] [ACPROB[(name)]]]
   [/TEST[(constants)]=['label'] effect values ...; ... | ALL values ...] ...
```

The values of the dependent, sorted ascending (numbers or strings), are the ordered categories. Factors may be strings; covariates are numeric. A case missing on any variable is left out (INCLUDE keeps user-missing values); the weight is rounded to a whole number and a case below 1 after rounding is left out. The model puts the link of the probability of each category or a lower one equal to its threshold minus the location effects, divided by exp of the scale effects. LOCATION defaults to the covariates and then the main effect of each factor; effects are written as in UNIANOVA (a\*b, a BY b, a(b), a WITHIN b, x\*x), and INTERCEPT alone leaves only the thresholds. A factor has one parameter per level that occurs; the last is redundant and 0. The cases are pooled by their factor levels and covariate values: DELTA goes into each empty cell of such a group, BIAS into every cell. The fit is Newton-Raphson from thresholds at the cumulative shares of the categories and the other parameters 0 (the location part first when SCALE is given), each step halved up to MXSTEP times while the log likelihood would fall; it stops when no parameter moves PCONVERGE or more, the log likelihood changes less than LCONVERGE (absolute or relative), or after MXITER iterations.

Tables: Case Processing Summary and Model Fitting Information (-2 log likelihood of the thresholds alone and of the final model, their difference as a chi-square) always; Goodness-of-Fit (FIT: Pearson and deviance over the pooled cells), Pseudo R-Square (SUMMARY: Cox and Snell, Nagelkerke, McFadden) and Parameter Estimates (PARAMETER: estimate, standard error, Wald, df, significance and CIN interval) without PRINT or when named; Test of Parallel Lines (TPARALLEL: a likelihood ratio test against separate location parameters for each threshold), Cell Information (CELLINFO), Parameter Covariance (COVB), Parameter Correlation (CORB) and Iteration History (HISTORY, every n-th, the first and the last) when named. The -2 log likelihood includes the multinomial constant of the pooled cells; KERNEL leaves it out. A warning counts the empty pooled cells.

SAVE adds EST1\_m, EST2\_m, ... (the first n categories, 25 by default), PRE\_m, PCP\_m and ACP\_m with the smallest free m, or root1, root2, ... and the names given: the probability of each category, the most likely category, its probability and the probability of the observed category, for every case with valid factors and covariates. TEST gives Wald tests of combinations of the parameters, in the order of the Parameter Estimates (thresholds, location, scale, redundant ones included): INTERCEPT takes one value per threshold, ALL one per parameter; one TEST uses either ALL in every row or effect names, never both. Each row is tested against its constant (0 by default) on 1 df and the rows together on the rank of their covariance; a coefficient on a redundant parameter multiplies its 0.

## Example

```text
PLUM comfort BY seat_row WITH trip_hours /LINK=PROBIT /PRINT=FIT PARAMETER SUMMARY TPARALLEL /SAVE=ESTPROB PREDCAT.
```

`statscraft help PLUM` prints the same text in the terminal. [All commands](README.md)
