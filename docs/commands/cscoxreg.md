<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# CSCOXREG

Cox proportional hazards regression on a complex sample, with design-based standard errors and tests.

## Syntax

```text
CSCOXREG [start] end [BY factors] [WITH covariates]
   /VARIABLES STATUS=var(values) [ID=var] [BASELINESTRATA=var] /PLAN FILE='file'
   [/MODEL effects]
   [/CUSTOM [LABEL='text'] LMATRIX=effect coefficients ... [; ...] [KMATRIX=k [; k ...]]]
   [/CRITERIA [CILEVEL=95] [DF=n] [LCONVERGE=[0 RELATIVE | ABSOLUTE]] [MXITER=100]
              [MXSTEP=5] [PCONVERGE=[1E-6 RELATIVE | ABSOLUTE]] [SINGULAR=1E-12]
              [TIES=EFRON | BRESLOW]]
   [/STATISTICS [PARAMETER] [EXP] [SE] [TTEST] [CINTERVAL] [DEFF] [DEFFSQRT]]
   [/TEST [TYPE=F | ADJF | CHISQUARE | ADJCHISQUARE]
          [PADJUST=LSD | BONFERRONI | SEQBONFERRONI | SIDAK | SEQSIDAK]]
   [/TESTASSUMPTIONS [PROPHAZARD=KM | IDENTITY | LOG | RANK] [PARAMETER] [COVB]]
   [/DOMAIN VARIABLE=var(value)] [/MISSING CLASSMISSING=EXCLUDE | INCLUDE]
   [/PRINT [SAMPLEINFO] [EVENTINFO] [RISKINFO] [HISTORY[(n)]] [GEF] [LMATRIX] [COVB]
           [CORB] [BASELINE] [NONE]]
   [/SURVIVALMETHOD [BASELINE=EFRON | BRESLOW | PRODUCTLIMIT] [CI=ORIGINAL | LOG | LML]]
   [/SAVE keyword[(name)] ... | keyword[(root[:n])] ...]
   [/PLOT [SURVIVAL] [CUMHAZARD] [LML] [OMS] [CI=NO | YES]]
   [/PATTERN [end(times)] var(value or values) ... [BY factor]] [/PATTERN ...]
   [/OUTFILE [COVB='file' | CORB='file'] [MODEL='file'] [SURVIVAL='file']]
   [/JOINTPROB FILE='file' | dataset].
```

A record is at risk after its start time up to and including its end time; with one time variable it is at risk from the beginning. A status among the event values means the event happened at the end time; any other valid status is censored. A negative time, or a start time not below the end time, leaves the record out. The hazard is h0(t) exp(x'b) without an intercept, which the baseline hazard h0 absorbs, so the last level of each factor is the reference. BASELINESTRATA gives each of its values its own baseline hazard while the coefficients are shared. The estimates maximize the weighted partial likelihood, tied event times treated by Efron's method (TIES=BRESLOW: Breslow's), by Newton-Raphson with step-halving from b = 0. The covariance is a sandwich: the inverse information on both sides of the design covariance of the total of the records' score residuals. Design, df, domains, TEST, CUSTOM and design effects work as in CSGLM. The default output is the sample design information with the event and censored counts, the events and censored records of each baseline stratum, the -2 log pseudo likelihood and the Tests of Model Effects; the parameter table needs STATISTICS. RISKINFO lists, per event time, the events and the records at risk. PRINT GEF shows the general estimable function: one column per nonredundant parameter.

ID names the subject of each record. The records of a subject are summed before the design variance, and where the plan's last stage has no clusters the subjects are its units. With ID and one time variable, a subject's first record starts at the beginning and each later one where the previous record (in time) ended.

Factors and covariates may come from a TIME PROGRAM: each record is then split at the event times of its baseline stratum and the program runs on every piece with T\_ set to the piece's event time. With such covariates there are no baseline functions, plots, PATTERN or SURVIVAL file, and SAVE writes only SCHOENFELD, SCORE, DFBETA, their aggregated forms and XBETA. TESTASSUMPTIONS fits a larger model in which every predictor also enters multiplied by g(t), one minus the weighted Kaplan-Meier survival of all records (KM, the default), t, ln(t) or the rank of the event time plus one, and gives the Wald test (as TEST says) that those extra coefficients are 0; PARAMETER and COVB show the larger model's estimates, standard errors and covariances.

Baseline functions are computed per baseline stratum at the reference values (each factor at its last level, each covariate at its weighted mean): the cumulative hazard by Efron's or Breslow's sum (as TIES unless SURVIVALMETHOD says otherwise) or minus the log of the product-limit survival. Their standard errors are design-based, from the design variance of the total of each record's influence on the cumulative hazard, the same for all three estimates. The survival interval uses the normal quantile on the scale CI names: the survival itself, its log (default) or the log of minus its log, cut to 0 and 1. PRINT BASELINE tabulates them; PLOT draws the step functions for the reference values and for each PATTERN line (BY gives one line per factor level; variables a PATTERN leaves out stay at the reference). SAVE writes, for the records analyzed: SCHOENFELD, SCORE and DFBETA (one variable per parameter, root\_1, root\_2 ..., 25 at most unless :n says), MARTINGALE, DEVIANCE, COXSNELL, XBETA, and SURVIVAL, CUMHAZARD and their LCL\_ and UCL\_ limits at each record's own end time and values.

Default names: Resid\_Schoenfeld, Resid\_Score, Resid\_DFBETA, Resid\_Martingale, Resid\_Deviance, Resid\_CoxSnell, XBETA, Survival, CumHazard, LCL\_Survival and so on; a taken default name gets a suffix. AGGMARTINGALE, AGGDEVIANCE, AGGCOXSNELL, AGGSCORE and AGGDFBETA (default names AggResid\_...) total a subject's records and are written on its last record; the deviance comes from the summed martingale residual and events.

Without ID they are skipped with a warning. A PATTERN that gives the end time a list of ascending times is a path: each other variable takes one value, or one per time, held up to that time, and the curve stops at the last time. OUTFILE COVB or CORB writes the matrix file of CSGLM, MODEL a JSON file with the estimates, covariances, reference values and baseline functions, SURVIVAL a data file of the curves of PLOT at every event time. JOINTPROB works as in CSDESCRIPTIVES.

## Example

```text
CSCOXREG months BY clinic WITH dose /VARIABLES STATUS=relapse(1) /PLAN FILE='trial_plan.json' /STATISTICS PARAMETER EXP SE CINTERVAL /PRINT RISKINFO.
```

`statscraft help CSCOXREG` prints the same text in the terminal. [All commands](README.md)
