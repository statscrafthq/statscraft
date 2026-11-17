<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# CSCOXREG

Cox proportional hazards regression on a complex sample, with design-based standard errors and tests.

## Syntax

```text
CSCOXREG [start] end [BY factors] [WITH covariates]
   /VARIABLES STATUS=var(values) [BASELINESTRATA=var] /PLAN FILE='file'
   [/MODEL effects]
   [/CUSTOM [LABEL='text'] LMATRIX=effect coefficients ... [; ...] [KMATRIX=k [; k ...]]]
   [/CRITERIA [CILEVEL=95] [DF=n] [LCONVERGE=[0 RELATIVE | ABSOLUTE]] [MXITER=100]
              [MXSTEP=5] [PCONVERGE=[1E-6 RELATIVE | ABSOLUTE]] [SINGULAR=1E-12]
              [TIES=EFRON | BRESLOW]]
   [/STATISTICS [PARAMETER] [EXP] [SE] [TTEST] [CINTERVAL] [DEFF] [DEFFSQRT]]
   [/TEST [TYPE=F | ADJF | CHISQUARE | ADJCHISQUARE]
          [PADJUST=LSD | BONFERRONI | SEQBONFERRONI | SIDAK | SEQSIDAK]]
   [/DOMAIN VARIABLE=var(value)] [/MISSING CLASSMISSING=EXCLUDE | INCLUDE]
   [/PRINT [SAMPLEINFO] [EVENTINFO] [RISKINFO] [HISTORY[(n)]] [LMATRIX] [COVB] [CORB]
           [NONE]].
```

A record is at risk after its start time up to and including its end time; with one time variable it is at risk from the beginning. A status among the event values means the event happened at the end time; any other valid status is censored. A negative time, or a start time not below the end time, leaves the record out. The hazard is h0(t) exp(x'b) without an intercept, which the baseline hazard h0 absorbs, so the last level of each factor is the reference. BASELINESTRATA gives each of its values its own baseline hazard while the coefficients are shared. The estimates maximize the weighted partial likelihood, tied event times treated by Efron's method (TIES=BRESLOW: Breslow's), by Newton-Raphson with step-halving from b = 0. The covariance is a sandwich: the inverse information on both sides of the design covariance of the total of the records' score residuals. Design, df, domains, TEST, CUSTOM and design effects work as in CSGLM. The default output is the sample design information with the event and censored counts, the events and censored records of each baseline stratum, the -2 log pseudo likelihood and the Tests of Model Effects; the parameter table needs STATISTICS. RISKINFO lists, per event time, the events and the records at risk. Not supported yet: VARIABLES ID, covariates from a TIME PROGRAM, JOINTPROB, TESTASSUMPTIONS, SURVIVALMETHOD, SAVE, PLOT, PATTERN, OUTFILE, PRINT GEF and BASELINE, and the UNEQUAL\_WOR estimator.

## Example

```text
CSCOXREG months BY clinic WITH dose /VARIABLES STATUS=relapse(1) /PLAN FILE='trial_plan.json' /STATISTICS PARAMETER EXP SE CINTERVAL /PRINT RISKINFO.
```

`statscraft help CSCOXREG` prints the same text in the terminal. [All commands](README.md)
