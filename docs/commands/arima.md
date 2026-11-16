<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# ARIMA

Seasonal ARIMA models with regressors by exact maximum likelihood, with forecasts.

## Syntax

```text
ARIMA [VARIABLES=] series [WITH regressors]
      [/MODEL=[(p,d,q)[(sp,sd,sq)[period]]] [CONSTANT | NOCONSTANT] [NOLOG | LG10 | LN]]
      [/P=n | (lags)] [/D=n] [/Q=n | (lags)] [/SP=...] [/SD=n] [/SQ=...]
      [/AR=values] [/MA=values] [/SAR=values] [/SMA=values] [/REG=values] [/CON=value]
      [/MXITER=10] [/MXLAMB=1E9] [/SSQPCT=0.001] [/PAREPS=0.001] [/CINPCT=95]
      [/FORECAST=EXACT | CLS | AUTOINIT] [/APPLY [SPECIFICATIONS | INITIAL | FIT]].
```

The model is phi(B) PHI(B^s) (w\_t - mu) = theta(B) THETA(B^s) a\_t, w\_t the series less the regressors times their coefficients, differenced d times and over the period sd times; the polynomials are 1 - c\_1 B - ..., so a positive MA coefficient means a negative weight on the past error. P, Q, SP and SQ name the lags to fit (n for 1 to n, a list in parentheses for those lags only) and win over MODEL. LOG is LG10; logs need values above 0. CONSTANT and the confidence level default to TSET. The estimates maximize the exact Gaussian likelihood (a Kalman filter that also takes missing values inside the series) by Levenberg-Marquardt steps on numerical derivatives. It stops after MXITER steps, when no estimate changes by PAREPS (default TSET CNVERGE), when the adjusted sum of squares changes by less than SSQPCT percent, or when the Marquardt constant passes MXLAMB.

MXITER=0 takes the initial values as final. AR, MA, SAR, SMA, REG and CON give initial values in the order of the lags. Output: the iteration history (TSET PRINT=BRIEF leaves it out, DETAILED adds the estimates of each step), the estimates with standard errors, t and approximate significance, the residual variance and standard error, the log-likelihood, AIC and SBC, and the covariance and correlation of the estimates. TSET NEWVAR saves FIT\_n, ERR\_n, LCL\_n, UCL\_n and SEP\_n: one-step fitted values (EXACT: from the Kalman filter; CLS: past errors 0 and missing past values at the mean of the differenced series; AUTOINIT: the first values of the series as their own fits), the residuals on the log scale, the prediction limits and standard errors. Fits, limits and standard errors are taken back from logs. With PREDICT the fit continues as forecasts; with regressors only where they are valid, and the file is not extended.

APPLY without a name repeats the previous ARIMA with the subcommands given now replacing its own; INITIAL starts from its estimates, FIT uses them without estimation.

## Example

```text
ARIMA daily_calls WITH promo /MODEL=(1,0,1)(0,1,1) 7 NOCONSTANT /FORECAST=EXACT.
```

`statscraft help ARIMA` prints the same text in the terminal. [All commands](README.md)
