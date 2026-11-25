<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# CCF

Cross-correlations of pairs of time series, with standard errors and charts.

## Syntax

```text
CCF [VARIABLES=] series [WITH series] [/DIFF=n] [/SDIFF=n] [/PERIOD=n] [/LN | /NOLOG]
    [/SEASONAL] [/MXCROSS=n] [/APPLY].
```

Without WITH every series is paired with each one after it; with WITH each series before it is paired with each one after it. Both series of a pair are prepared as in ACF. At lag k the correlation is of x at time t with y at time t + k, so a positive lag means the first series leads; the sums use the cases where both are valid, divided by the root of the two sums of squares about their means. Lags run from -MXCROSS to MXCROSS (else TSET MXCROSS, else 7), at most the series length less one. The standard error is 1 / sqrt(m), m the number of valid pairs at that lag. A table and a chart (limits at two standard errors) per pair. APPLY repeats the previous CCF, or the model named in quotes (MOD\_1, MOD\_2, ... in the order the session made them).

## Example

```text
CCF ad_spend WITH weekly_sales /DIFF /MXCROSS=10.
```

`statscraft help CCF` prints the same text in the terminal. [All commands](README.md)
