<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# NLR

Nonlinear regression by least squares (Levenberg-Marquardt) for the model of the last MODEL PROGRAM.

## Syntax

```text
NLR dependent
   [/FILE=file] [/OUTFILE=file] [/PRED=variable]
   [/SAVE=[PRED] [RESID[(variable)]] [DERIVATIVES]]
   [/CRITERIA=[ITER(100 per parameter)] [CKDER(0.5)] [SSCON(1E-8)] [PCON(1E-8)] [RCON(1E-8)]]
```

Fits the model defined by MODEL PROGRAM (and DERIVATIVES) to the dependent variable by minimizing the sum of squared residuals, weighted by the case weights (which are not case replicates here: the degrees of freedom count the cases). The tables are the iteration history (the residual sum of squares and the parameters at each iteration), the analysis of variance (regression, residual, uncorrected total, corrected total) with R squared, the parameter estimates with asymptotic standard errors and 95% confidence intervals, and the asymptotic correlations of the estimates.

The iterations stop when a step lowers the sum of squares by a proportion of at most SSCON, when no parameter changes by more than the proportion PCON, when the correlation of every derivative with the residuals is at most RCON, or after ITER iterations (a warning); 0 switches a criterion off. FILE reads starting values from a data file that an earlier OUTFILE wrote, for the parameters the MODEL PROGRAM names without a value (a value given there wins); with SPLIT FILE its cases go to the groups in order, the last one to every group after it. OUTFILE writes the split variables, the parameters, SSE (the residual sum of squares) and NCASES, one case per group. SAVE appends the predicted values, the residuals (named in parentheses, else RESID or what the keyword was written as) and the derivatives D.parameter (the first six letters of the parameter name) to the data; a variable whose name is taken is not saved. A case whose dependent variable, weight or predicted value is missing, or whose weight is zero or negative, is left out.

## Example

```text
MODEL PROGRAM b0=2 b1=0.3.
COMPUTE pred = b0 * EXP(b1 * dose).
NLR response.
```

`statscraft help NLR` prints the same text in the terminal. [All commands](README.md)
