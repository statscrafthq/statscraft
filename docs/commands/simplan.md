<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# SIMPLAN

A simulation plan: distributions of the inputs, fitted to the data or given, their correlations and the targets of a model.

## Syntax

```text
SIMPLAN CREATE [SOURCE=AUTOFIT]
  [/MODEL FILE='file' [CUSTOM=NO | YES]]
  [/TARGETOPTS TARGET=field[(FORMAT=type[,d])] ... [COMMENT='text'] [OUTPUT=YES | NO]
              [TARGETFILTER=NO | YES([MIN=LO | value[(INCLUSIVE | EXCLUSIVE)]] [MAX=HI | value[(...)]])]] ...
  /SIMINPUT INPUT=field[(MAPTO=name FORMAT=type[,d])] ... [OUTPUT=YES | NO]
            [TYPE=AUTOFIT[(DEFAULT | distributions)] | MANUAL] [DISTRIBUTION=name(PARAM=values ...)]
            [MINVAL=n] [MAXVAL=n] ...
  [/FIXEDINPUT INPUT=field ... VALUE=n | ITERATIONS=VALUES(n ...) | RANGE(MIN=n MAX=n N=k)] ...
  [/CORRELATIONS VARORDER=fields CORRMATRIX=1; r, 1; r, r, 1 ...]
  [/AUTOFIT [NCASES=ALL | n] [FIT=AD | KS] [MAXITERATIONS=100] [BINS=100]]
  [/STOPCRITERIA [MAXCASES=100000] [TARGET=field
                TYPE=MEAN(THRESH=n UNIT=PERCENT | VALUE [CILEVEL=95]) |
                     TAIL(UNIT=PERCENTILE | VALUE THRESH=n [SIDE=RIGHT | LEFT] FREQUENCY=n)]]
  [/MISSING CLASSMISSING=EXCLUDE | INCLUDE] [/VALUELABELS VARIABLES=fields VALUES=value 'label' ...] ...
  /PLAN FILE='file' [DISPLAY=YES | NO].
SIMPLAN VIEW /PLAN FILE='file'.
```

Distributions and their parameters: BERNOULLI(PROB) BETA(SHAPE1 SHAPE2) BINOM(N PROB) CATEGORICAL(CATS=values PROBS=values) EMPIRICAL EXP(SCALE) GAMMA(SHAPE SCALE) LNORMAL(A B) NEGBIN(\[TYPE=FAILURES \| TRIALS\] THRESHOLD PROB) NORMAL(MEAN STDDEV) POISSON(MEAN) TRIANGULAR(MIN MAX MODE) UNIFORM(MIN MAX) USER\_RANGES(MIN=values MAX=values PROBS=values) WEIBULL(A B \[C\]). The SCALE of EXP and GAMMA is the rate, as in the RV and CDF functions; LNORMAL A is the median and B the standard deviation of the logarithm; WEIBULL A is the scale, B the shape and C the origin. The model is a file that BEGIN EXPR wrote, or a plan whose model is taken. Every input of a model is simulated (SIMINPUT) or fixed (FIXEDINPUT); without MODEL the plan only generates the SIMINPUT fields. AUTOFIT (default) fits each input to its field in the active dataset (MAPTO names another field) by maximum likelihood: a nominal field gets the categorical distribution, an ordinal one the binomial, negative binomial (failures) and Poisson by the chi-square statistic, a scale one the beta, exponential, gamma, lognormal, normal, triangular, uniform and two-parameter Weibull by the Anderson-Darling statistic (or Kolmogorov-Smirnov with FIT=KS); the smallest statistic wins and a Goodness of Fit table shows them all with their p-values where known. Weights count; cases missing on any fitted input are left out (user-missing values of a categorical fit count with CLASSMISSING=INCLUDE). Without CORRELATIONS the Pearson correlations of the fields in the data are used. MINVAL and MAXVAL cut the distribution. Several values of one parameter, several PROBS sets or FIXEDINPUT ITERATIONS run one simulation per value (sensitivity analysis). The plan is Statscraft's own JSON. SPLIT FILE is not used.

## Example

```text
SIMPLAN CREATE /MODEL FILE='margin.json' CUSTOM=YES
  /SIMINPUT INPUT=units TYPE=MANUAL DISTRIBUTION=TRIANGULAR(MIN=800 MAX=1400 MODE=1000)
  /SIMINPUT INPUT=unit_cost /FIXEDINPUT INPUT=unit_price VALUE=12
  /PLAN FILE='margin.splan'.
```

`statscraft help SIMPLAN` prints the same text in the terminal. [All commands](README.md)
