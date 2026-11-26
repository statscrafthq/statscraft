<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# FIT

Goodness-of-fit statistics of residual series from time series models.

## Syntax

```text
FIT [[ERRORS=] residual series] [/OBS=observed series] [/DFE=n ... | /DFH=n ...].
```

For each residual series e over n cases: the mean error, the mean absolute error, the sum of squared errors, the mean square error (the sum over the error degrees of freedom), its square root and the Durbin-Watson statistic. With OBS (one observed series per residual series, in order, repeated where several residual series come from one) also the mean percent error and the mean absolute percent error, 100 times the mean of e/x. Without any names it takes the last ERR\_n series a procedure saved, with the series it was made from. DFE gives the error degrees of freedom of each residual series and DFH the model's parameters, which are taken from the cases; with neither the cases count. A PREDICT period that starts inside the series gets its own columns next to the USE period's. A case with a missing residual is left out; the percent statistics also leave out a case whose observed value is missing or 0. With SPLIT FILE each group is a block of rows in the one table.

## Example

```text
FIT ERRORS=ERR_1 ERR_2 /OBS=sales sales /DFH=2 3.
```

`statscraft help FIT` prints the same text in the terminal. [All commands](README.md)
