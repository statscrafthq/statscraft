<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# BOOTSTRAP

Give the next procedure bootstrap bias, standard errors and confidence intervals.

## Syntax

```text
BOOTSTRAP [/SAMPLING METHOD=SIMPLE | STRATIFIED(STRATA=varlist)]
  [/VARIABLES [TARGET=varlist] [INPUT=varlist]]
  [/CRITERIA [CILEVEL=95 | level] [CITYPE=PERCENTILE | BCA] [NSAMPLES=1000 | n]]
  [/MISSING [USERMISSING=EXCLUDE | INCLUDE]].
```

Keywords are written in full. The next procedure runs on the data and then again on n resamples drawn with replacement (within each stratum for STRATIFIED), with the generator SET SEED or MTINDEX starts. Cases with a missing value in a TARGET, INPUT or STRATA variable leave the analysis; user-missing values of nominal and ordinal variables count as valid with USERMISSING=INCLUDE. DESCRIPTIVES, FREQUENCIES, MEANS, CORRELATIONS, T-TEST, REGRESSION, CROSSTABS, EXAMINE, ONEWAY, NONPAR CORR, PARTIAL CORR, LOGISTIC REGRESSION, NOMREG, PLUM, UNIANOVA and GLM add Bias, Std. Error and the interval (percentile or BCa) to their main statistics; any other procedure warns and runs without resampling. RESIDUAL and WILD resampling are not supported yet.

## Example

```text
BOOTSTRAP /VARIABLES INPUT=height weight /CRITERIA CITYPE=BCA NSAMPLES=2000.
```

`statscraft help BOOTSTRAP` prints the same text in the terminal. [All commands](README.md)
