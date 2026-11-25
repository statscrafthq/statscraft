<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# 2SLS

Two-stage least squares for equations with endogenous predictors.

## Syntax

```text
2SLS [EQUATION=] dependent WITH predictors [/[EQUATION=] dependent WITH predictors]
     /INSTRUMENTS=variables [/ENDOGENOUS=variables] [/CONSTANT | /NOCONSTANT]
     [/PRINT=COV] [/SAVE=PRED RESID] [/APPLY].
```

The first stage regresses every predictor on the instruments (and the constant); the second regresses the dependent on those predicted values. A predictor that is also an instrument predicts itself, so naming every predictor as an instrument gives ordinary least squares. There must be at least as many instruments as predictors in the longest equation. Repeated INSTRUMENTS lists add up; ENDOGENOUS only documents the model. Case weights count, and a case missing on any variable of the command leaves every equation. Per equation: R, R square, the standard error and the mean of the residuals, an ANOVA table, the coefficients with standardized ones, t tests and tolerances, and the correlations of the estimates (PRINT=COV or TSET PRINT=DETAILED adds their covariances). Residuals use the observed predictors. TSET NEWVAR saves FIT\_n and ERR\_n per equation; SAVE picks them itself, always under free numbers. CONSTANT defaults to TSET. APPLY repeats the previous 2SLS, or the model named in quotes (MOD\_1, MOD\_2, ... in the order the session made them), with the subcommands given now replacing its own.

## Example

```text
2SLS crop_yield WITH fertilizer_kg /INSTRUMENTS=subsidy_rate soil_ph /SAVE=RESID.
```

`statscraft help 2SLS` prints the same text in the terminal. [All commands](README.md)
