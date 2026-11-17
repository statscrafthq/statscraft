<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# PLS

Partial least squares regression of one or more dependent variables on many, possibly collinear, predictors.

## Syntax

```text
PLS dependent [MLEVEL=N | O | S] [REFERENCE=FIRST | LAST | value] [dependent ...]
   [BY factors] [WITH covariates]
   [/ID VARIABLE=var] [/MODEL effects]
   [/OUTDATASET [CASES=dataset] [LATENTFACTORS=dataset] [PREDICTORS=dataset]]
   [/CRITERIA LATENTFACTORS=5].
```

Each subcommand and keyword at most once, written in full, with its equals sign. A dependent is scale or categorical: MLEVEL after it, else its measurement level, else scale for a number and categorical for a string (N and O both mean categorical). ALL MLEVEL=... or ALL REFERENCE=... sets every dependent that has no setting of its own. A categorical dependent becomes one 0/1 column per category except the reference one (REFERENCE: FIRST, LAST by default, or a value that occurs). Factors follow BY, covariates WITH; a factor gives a 0/1 column for every level, an interaction one column per combination of levels that occurs (times the covariates in it). MODEL lists main effects and interactions (a\*b or a BY b), the main effects of all predictors by default; nested terms are not taken.

Every column is centered and divided by its standard deviation. Cases missing on any variable (user-missing too) are left out; the weight is rounded and a case below 1 is left out. NIPALS extracts up to LATENTFACTORS factors, no more than the predictor columns or the cases less one. Tables: Proportion of Variance Explained (X and Y, cumulative, R-square and adjusted R-square), Parameters (constant and coefficient of every predictor column in the original units), Variable Importance in the Projection (cumulative by factor), Weights and Loadings. OUTDATASET writes named datasets: CASES (ID or case number, predicted values and residuals, the distance to the X model, the X and Y scores) for every case with valid predictors, LATENTFACTORS (loadings and weights of each variable) and PREDICTORS (coefficients and VIP of each predictor column). No plots.

## Example

```text
PLS yield MLEVEL=S quality MLEVEL=N REFERENCE=FIRST BY plot WITH rain sun soil_ph /CRITERIA LATENTFACTORS=3 /OUTDATASET CASES=plsCases.
```

`statscraft help PLS` prints the same text in the terminal. [All commands](README.md)
