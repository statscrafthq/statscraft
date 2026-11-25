<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# COXREG

Cox proportional hazards regression of survival times, with strata, categorical covariates, stepwise selection and time-dependent covariates.

## Syntax

```text
COXREG [VARIABLES=] time [WITH var | var BY var | var TO var ...]
   /STATUS=var [EVENT] (values) [LOST (values)] [/STRATA=var]
   [/CATEGORICAL=varlist] [/CONTRAST(var)=DEVIATION[(ref)] | SIMPLE[(ref)] | DIFFERENCE |
   HELMERT | REPEATED | POLYNOMIAL[(metric)] | SPECIAL(matrix) | INDICATOR[(ref)]]
   [/METHOD=ENTER | FSTEP[(COND | LR | WALD)] | BSTEP[(COND | LR | WALD)] [ALL | terms]] ...
   [/MISSING=EXCLUDE | INCLUDE] [/PRINT=[DEFAULT] [SUMMARY] [BASELINE] [CORR] [CI[(95)]] [ALL]]
   [/CRITERIA=[BCON(1E-4)] [LCON(1E-5)] [ITERATE(20)] [PIN(0.05)] [POUT(0.1)]]
   [/PLOT=[NONE] [SURVIVAL] [HAZARD] [LML] [OMS]] [/PATTERN=var(value) ... [BY var]] ...
   [/SAVE=tempvar[(name)] ...] [/OUTFILE=[COEFF('file' | dataset)] [TABLE('file' | dataset)]] [/EXTERNAL].
```

tempvar: SURVIVAL SE HAZARD (or RESID) LML XBETA DFBETA PRESID. A case is used when time, status, strata and covariates are valid and its weight is positive; a negative time drops it, and so does a censored time earlier than every event of its stratum. Status values on the EVENT list mark an event; all others are censored (LOST changes nothing). String covariates are categorical; a categorical variable with k values becomes k - 1 coded columns that enter and leave together (DEVIATION, last category as reference, by default). Tied times follow Breslow. The fit is Newton-Raphson from all coefficients 0 on covariates centered at their means; it stops when no coefficient changes by BCON or more of its previous size, the log likelihood changes by less than LCON of its previous value, or after ITERATE iterations. Each METHOD starts from the model the one before it left: ENTER enters its terms, FSTEP enters by the score test (PIN) and removes by the chosen statistic (POUT), BSTEP enters its terms first and then removes and enters the same way. With no covariate only the beginning block and the baseline are shown. Tables: Case Processing Summary, Categorical Variable Codings, Omnibus Tests of Model Coefficients (block 0: -2 log likelihood; then per step the overall score test at all coefficients 0 and the changes from the previous step and block), Variables in the Equation (with PRINT CI the interval of Exp(B)), Model if Term Removed (COND and LR), Variables not in the Equation with the residual chi-square, and with PRINT the Correlation Matrix of Regression Coefficients, Covariate Means and a Survival Table per stratum: at every event time the baseline cumulative hazard with all covariates 0, and survival, its standard error and the cumulative hazard at the means. The baseline survival is the maximum likelihood estimate given the coefficients, so with no covariate it is the Kaplan-Meier curve. SUMMARY keeps a line per step in the Omnibus Tests and shows the other tables for the last step of each block only. String covariates tell their categories apart by their first eight bytes.

PLOT gives chart data of the step curves of each stratum at the means and at each PATTERN: named covariates at the values given, the others at their means; BY draws one line per category of a categorical covariate. SAVE adds, at each case's own time and covariates, SUR\_1, SE\_1, HAZ\_1, LML\_1 and XBE\_1 (the linear predictor with centered covariates), DFB1\_1, DFB2\_1, ... (the change in each coefficient when the case is left out, from its score residual) and PRE1\_1, PRE2\_1, ... for the Schoenfeld residuals of events, one per coded covariate and missing for those out of the final model, or the names given (DFBETA(name) gives name1, name2, ...). Weighted when a weight variable is set. After a TIME PROGRAM, covariates it creates change with time; SAVE then gives DFBETA only and the survival table, PATTERN and plots are left out. EXTERNAL changes nothing.

OUTFILE writes each keyword to a .sav file or a dataset made with DATASET DECLARE. COEFF holds the final model: VARNAME\_ (the coded column), B, SE, WALD, DF, SIG and EXPB, a record for each column, as in Variables in the Equation. TABLE holds the survival table of every stratum with a record for each event time: the strata variable (when there is one), TIME\_, then SURVIVAL\_, SE\_ and HAZARD\_ with the suffix 0 for the baseline (all covariates 0), M for the means, and the number of the line for each line of PATTERN (a BY line for each category); with a time program TABLE is not written.

## Example

```text
COXREG months WITH plan spend /STATUS=ended(1) /STRATA=site /CATEGORICAL=plan /PRINT=CI(95) BASELINE /PLOT=SURVIVAL.
```

`statscraft help COXREG` prints the same text in the terminal. [All commands](README.md)
