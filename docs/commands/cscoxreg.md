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

The command fits a Cox proportional hazards model to a complex sample, with design-based standard errors and tests. VARIABLES STATUS and PLAN FILE are required. STATUS lists the event values (values or THRU ranges); any other valid status is censored at the end time. A record is at risk after its start time up to and including its end time, or from the beginning with one time variable. A negative time, or a start time not below the end time, leaves the record out.

The hazard is h0(t) exp(x'b) without an intercept, so the last level of each factor is the reference, and a CUSTOM row must name an effect or ALL (CUSTOM needs LMATRIX). BASELINESTRATA gives each of its values its own baseline hazard with shared coefficients. The estimates maximize the weighted partial likelihood by Newton-Raphson with step-halving from b = 0; tied event times use Efron's method, or Breslow's with TIES=BRESLOW. The covariance is a sandwich around the design covariance of the score residuals. Design, df, domains, TEST, CUSTOM and design effects work as in CSGLM, and JOINTPROB as in CSDESCRIPTIVES.

The Model Summary (-2 log pseudo likelihood) and the Tests of Model Effects are always shown. Without PRINT the output also has SAMPLEINFO (the design, with event and censored counts) and EVENTINFO (events and censored records per baseline stratum); a PRINT list replaces these. The parameter table needs STATISTICS. RISKINFO lists the events and records at risk at each event time; GEF has one column per nonredundant parameter.

ID names the subject of each record. A subject's records are summed before the design variance, and where the plan's last stage has no clusters the subjects are its units. With ID and one time variable, a subject's first record starts at the beginning and each later one where the previous record in time ended.

Factors and covariates may come from a TIME PROGRAM: each record is split at the event times of its baseline stratum and the program runs on every piece with T\_ set to that event time. Then there are no baseline functions, plots, PATTERN or SURVIVAL file, and SAVE writes only SCHOENFELD, SCORE, DFBETA, their AGG forms and XBETA.

TESTASSUMPTIONS refits with every predictor also multiplied by g(t): one minus the weighted Kaplan-Meier survival (KM, the default), t (IDENTITY), ln(t) (LOG) or the event time's rank plus one (RANK). A Wald test of the TEST type checks that the extra coefficients are 0; PARAMETER and COVB show the larger model's estimates and covariances.

Baseline functions are computed per baseline stratum at the reference values (factors at their last level, covariates at their weighted means). The cumulative hazard is Efron's or Breslow's sum (as TIES, unless SURVIVALMETHOD BASELINE says otherwise) or minus the log of the product-limit survival, with design-based standard errors. The survival interval is normal on the CI scale: ORIGINAL, LOG (the default) or LML (log of minus log), cut to 0 and 1. PRINT BASELINE tabulates them. PLOT draws them for the reference values and for each PATTERN, whose BY gives one line per factor level; variables a PATTERN leaves out stay at the reference. A PATTERN giving the end time ascending times above 0 is a path: each other variable takes one value or one per time (held up to that time), and the curve stops at the last time.

SAVE writes, for the records analyzed, SCHOENFELD, SCORE and DFBETA (one variable per parameter, root\_1, root\_2 and so on, at most n, 25 by default), MARTINGALE, DEVIANCE, COXSNELL, XBETA, and SURVIVAL, CUMHAZARD and their LCL\_ and UCL\_ limits at each record's own end time and values. Default names are Resid\_Schoenfeld, Resid\_Score, Resid\_DFBETA, Resid\_Martingale, Resid\_Deviance, Resid\_CoxSnell, XBETA, Survival, CumHazard, LCL\_Survival and so on, with a suffix when taken. The AGG keywords (default names AggResid\_...) total a subject's records onto its last record; without ID they are skipped with a warning.

OUTFILE COVB or CORB (one of them) writes the matrix file as CSGLM does, MODEL a JSON file with the estimates, covariances, reference values and baseline functions, SURVIVAL a data file of the PLOT curves at every event time.

## Example

```text
CSCOXREG months BY clinic WITH dose /VARIABLES STATUS=relapse(1) /PLAN FILE='trial_plan.json' /STATISTICS PARAMETER EXP SE CINTERVAL /PRINT RISKINFO.
```

`statscraft help CSCOXREG` prints the same text in the terminal. [All commands](README.md)
