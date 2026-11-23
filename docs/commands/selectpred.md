<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# SELECTPRED

Screens a long list of predictors and ranks them one by one by how well each relates to a target.

## Syntax

```text
SELECTPRED target [(MLEVEL={S | O | N})] [BY factors] [WITH covariates]
    [/EXCEPT VARIABLES=varlist]
    [/SCREENING [STATUS={ON | OFF}] [PCTMISSING=70] [PCTEQUAL=95] [PCTUNEQUAL=90] [STDDEV=0] [CV=0.001]]
    [/CRITERIA [SIZE={AUTO | n}] [PCUTOFF=0.05] [RANKING={PEARSONCHISQ | LRCHISQ | CRAMERSV | LAMBDA}]
               [TIMER=5] [SHOWUNSELECTED=0]]
    [/MISSING USERMISSING={EXCLUDE | INCLUDE}]
    [/PRINT [CPS] [EXCLUDED] [SELECTED] [SUMMARY[(PEARSONCHISQ LRCHISQ CRAMERSV LAMBDA)]] [NONE]]
    [/PLOT SUMMARY].
```

The target is categorical when nominal or ordinal (by MLEVEL or the dictionary), else scale.

Factors (BY) are categorical predictors, covariates (WITH, numeric) scale ones; without BY and WITH every other variable but the weight is a predictor, its kind from the dictionary, and EXCEPT leaves some out. Cases missing on the target or on every predictor are left out.

Predictors missing or constant in every case are left out, and with screening on also those with more than PCTMISSING percent missing, categorical ones with one category over PCTEQUAL percent of their cases or over PCTUNEQUAL percent of categories holding a single case, and scale ones with a standard deviation under STDDEV or an absolute coefficient of variation under CV. Each remaining predictor is tested alone on the cases valid on it and the target: with a categorical target a categorical predictor by RANKING (the Pearson or likelihood-ratio chi-square significance, or Cramer's V or lambda, largest first), a scale one by the one-way F of the predictor across the target classes; with a scale target a categorical predictor by the F of the target across its categories and a scale one by the significance of Pearson's r.

When both kinds are present a chi-square significance (Pearson's unless LRCHISQ) ranks the categorical ones. Ties go to the larger statistic, then the smaller df, fewer categories or more cases, then file order. SIZE predictors are selected from the top (AUTO: the nearest whole number to the smaller of the predictor count and the larger of 30 and twice its square root), only those with a significance below PCUTOFF when significance ranks. Tables: Case Processing Summary, Excluded Predictors, Predictor Ranking (the selected ones and SHOWUNSELECTED more) and Selected Predictors; PLOT SUMMARY charts the ranking. Frequency weights are rounded, cases with a weight under 0.5 left out.

USERMISSING=INCLUDE keeps user-missing values of categorical variables. TIMER stops a run longer than that many minutes (0 for no limit). SPLIT FILE is not used. Names are written in full and each subcommand once.

## Example

```text
SELECTPRED churned BY region plan WITH spend visits tenure
  /EXCEPT VARIABLES=tenure /CRITERIA SIZE=3 RANKING=LRCHISQ /PRINT SUMMARY(CRAMERSV) /PLOT SUMMARY.
```

`statscraft help SELECTPRED` prints the same text in the terminal. [All commands](README.md)
