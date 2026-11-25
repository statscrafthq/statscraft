<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# AREG

Regression whose errors follow a first-order autoregressive process.

## Syntax

```text
AREG [VARIABLES=] series WITH regressors
     [/METHOD=PW | CO | ML] [/CONSTANT | /NOCONSTANT] [/RHO=0] [/MXITER=10]
     [/APPLY [SPECIFICATIONS | INITIAL | FIT]].
```

The errors of y = a + b x are taken to follow u\_t = rho u\_(t-1) + e\_t. PW (the default, Prais-Winsten) and CO (Cochrane-Orcutt) alternate two steps: least squares on the quasi-differences y\_t - rho y\_(t-1), then a new rho from the lag-one products of the residuals. CO drops the first case; PW keeps it, scaled by sqrt(1 - rho^2). RHO is the start (above -1, below 1). The loop ends when no estimate moves by TSET CNVERGE or after MXITER rounds. Both take missing values at the ends of the USE range only. ML fits the same model by exact maximum likelihood (the ARIMA engine with one autoregressive term), missing values allowed anywhere. CONSTANT defaults to TSET.

Output: per iteration rho, its standard error, Durbin-Watson and the mean square error (TSET PRINT=DETAILED adds the coefficients, BRIEF keeps only the final estimates), the R square of the transformed regression at the start and the end, an ANOVA table, and the estimates with t tests; ML shows what ARIMA shows. TSET NEWVAR saves FIT\_n, ERR\_n, LCL\_n, UCL\_n and SEP\_n; after the series, PREDICT gives forecasts where the regressors have values. APPLY repeats the previous AREG, or the model named in quotes (MOD\_1, MOD\_2, ... in the order the session made them) with the subcommands given now replacing its own; INITIAL starts from its estimates, FIT uses them as they are.

## Example

```text
AREG heating_cost WITH degree_days /METHOD=CO /RHO=0.3.
```

`statscraft help AREG` prints the same text in the terminal. [All commands](README.md)
