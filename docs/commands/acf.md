<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# ACF

Autocorrelations of time series, with standard errors, Box-Ljung tests and charts.

## Syntax

```text
ACF [VARIABLES=] series [/DIFF=n] [/SDIFF=n] [/PERIOD=n] [/LN | /NOLOG] [/SEASONAL]
    [/MXAUTO=n] [/SERROR=IND | MA] [/PACF] [/APPLY].
```

Before the correlations each series may be logged (LN; a series with a value not above 0 is skipped), differenced DIFF times (DIFF alone: once) and then differenced over the period SDIFF times. The period comes from PERIOD (at least 2), else TSET PERIOD, else the DATE variables; without one SDIFF and SEASONAL are left out with a warning. MXAUTO (else TSET MXAUTO, else 16) is the highest lag, at most the number of values less one; SEASONAL shows only the lags that are multiples of the period. A missing value (user-missing too, unless TSET MISSING=INCLUDE) drops the products it is part of; the mean and the counts use the valid values. Each series gets a table: autocorrelation r\_k, its standard error, and the Box-Ljung statistic of lags 1 to k with k df and its significance. IND (the default, or TSET ACFSE) takes the standard error of white noise, sqrt((n - k) / (n (n + 2))); MA takes Bartlett's sqrt((1 + 2 (r\_1^2 + ... + r\_(k-1)^2)) / n). PACF adds the partial autocorrelations, with standard error 1 / sqrt(n). A chart per series shows the coefficients with limits at two standard errors. APPLY without a name repeats the previous ACF, with the subcommands given now replacing its own; named models are not supported.

DIFFERENCE is another name for DIFF. Case weights are not used.

## Example

```text
ACF VARIABLES=daily_orders /DIFF /MXAUTO=21 /PACF.
```

`statscraft help ACF` prints the same text in the terminal. [All commands](README.md)
