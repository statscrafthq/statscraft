<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# PACF

Partial autocorrelations of time series, with standard errors and charts.

## Syntax

```text
PACF [VARIABLES=] series [/DIFF=n] [/SDIFF=n] [/PERIOD=n] [/LN | /NOLOG] [/SEASONAL]
     [/MXAUTO=n] [/APPLY].
```

The series are prepared and the lags chosen as for ACF. The partial autocorrelations come from the autocorrelations by the Durbin-Levinson recursion; each has the standard error 1 / sqrt(n), n the number of valid values. A table and a chart (limits at two standard errors) per series. APPLY without a name repeats the previous PACF.

## Example

```text
PACF river_level /SDIFF /PERIOD=7 /MXAUTO=28.
```

`statscraft help PACF` prints the same text in the terminal. [All commands](README.md)
