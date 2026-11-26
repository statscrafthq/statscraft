<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# LOGLINEAR

General loglinear and logit models of a table of counts, with contrasts, cell weights and generalized residuals.

## Syntax

```text
LOGLINEAR varlist (min,max) [varlist (min,max) ...] [BY varlist (min,max) ...] [WITH covariates]
   [/CWEIGHT=variable | (w n*w ...)] [/GRESID=variable | (c n*c ...)] [/GRESID ...]
   [/CONTRAST(factor)=DEVIATION[(ref)] | DIFFERENCE | HELMERT | SIMPLE[(ref)] | REPEATED |
      POLYNOMIAL[(metric)] | [BASIS] SPECIAL(matrix)]
   [/CRITERIA=[CONVERGE(0.001)] [ITERATE(20)] [DELTA(0.5)] [DEFAULT]]
   [/PRINT=[FREQ] [RESID] [DESIGN] [ESTIM] [COR] | DEFAULT | ALL | NONE]
   [/PLOT=[RESID] [NORMPROB] | DEFAULT | ALL | NONE] [/MISSING=EXCLUDE | INCLUDE]
   [/DESIGN=effect [effect ...]] [/DESIGN ...].
```

effect: a factor, a covariate, factor(n) (the n-th contrast column of the factor) or effects joined by BY.

Every integer of a factor's range is a category and the table has every combination; values are cut to integers and a case outside a range is counted and left out. A case missing on a variable is left out (INCLUDE keeps user-missing values, which the ranges must then hold); case weights are the counts. Factors after BY are the independent ones of a logit model, each combination of them a row whose total the model keeps. Cell covariates (WITH) take the weighted mean of the cases in the cell. At most 10 factors and 200 covariates.

Every DESIGN fits its own model with the CWEIGHT, GRESID, CONTRAST, CRITERIA, PRINT and PLOT given before it, which carry over to the next DESIGN; the DESIGN does not. A DESIGN without effects, or the model added when the command does not end with DESIGN (with a warning), is the saturated model of every effect, which also shows ESTIM by default. A factor of K categories gives K - 1 columns: DEVIATION (the default, the last category is the reference) estimates each category against the mean, SIMPLE against the reference, DIFFERENCE against the mean of the earlier ones, HELMERT of the later ones, REPEATED against the next, POLYNOMIAL gives orthonormal polynomials over the metric (1, 2, ..., K by default); SPECIAL takes the K by K matrix transposed as the columns (its first row is the constant), BASIS SPECIAL takes the inverse (the rows are the contrasts). factor(n) is the n-th column alone and effects joined by BY are products of columns. CWEIGHT gives a weight per cell, from a variable (its mean in the cell, 1 for an empty one) or as a list in cell order with the last factor changing fastest; a weight of 0 or less makes a structural zero left out of the fit and the tables. The model is fitted by Newton-Raphson from the counts plus DELTA, which stay in the cells of a saturated model, until nothing moves by CONVERGE or ITERATE steps have run.

Tables: Data Information, Convergence Information, Cell Counts and Residuals (FREQ: observed and expected counts and percents; RESID: raw, standardized and adjusted residuals), Goodness-of-Fit Tests (likelihood ratio and Pearson, df = I (J - 1) - parameters - cells whose count times weight is 0 or less, for a table of I rows and J columns), Design Matrix (DESIGN), Parameter Estimates (ESTIM: coefficient, standard error, z and 95% interval) and the correlation of the estimates (COR, also COV), Generalized Residuals (each GRESID a linear combination of the cells: observed, expected, residual and adjusted residual), and for a logit model the Analysis of Dispersion and Measures of Association (entropy and concentration). PLOT draws adjusted residuals against the observed and expected counts (RESID) and a normal and a detrended normal plot (NORMPROB).

## Example

```text
LOGLINEAR shift(1,3) site(1,4) /CONTRAST(site)=SIMPLE(1) /PRINT=DEFAULT ESTIM /DESIGN=shift site.
```

`statscraft help LOGLINEAR` prints the same text in the terminal. [All commands](README.md)
