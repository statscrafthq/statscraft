<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# NAIVEBAYES

Naive Bayes classification of a categorical target, with forward selection of the predictors.

## Syntax

```text
NAIVEBAYES target [BY factors] [WITH covariates]
           [/EXCEPT VARIABLES=varlist] [/FORCE [FACTORS=varlist] [COVARIATES=varlist]]
           [/TRAININGSAMPLE PERCENT=p | VARIABLE=var]
           [/SUBSET MAXSIZE=AUTO | n [(BESTSUBSET=PSEUDOBIC | TESTDATA)] | EXACTSIZE=n | NOSELECTION]
           [/CRITERIA [BINS=10] [MEMALLOCATE=1024] [TIMER=5]] [/MISSING USERMISSING=EXCLUDE | INCLUDE]
           [/PRINT [CPS] [EXCLUDED] [SUMMARY] [SELECTED] [CLASSIFICATION] | NONE]
           [/SAVE [PREDVAL[(name)]] [PREDPROB[(root[:n])]]] [/OUTFILE MODEL='file'].
```

Without BY and WITH every other variable is a predictor (strings, nominal and ordinal ones as factors, scale ones as covariates), except the weight, the TRAININGSAMPLE variable and EXCEPT.

Keywords are written in full, each subcommand once. A covariate is cut into BINS intervals of equal width over the training cases, empty inner intervals merged with their neighbours. Each probability is a smoothed count: (count + 1/N) over (total + categories/N), N the training cases. Selection starts from the FORCE predictors and adds, step by step, the one that raises the mean log-likelihood of the training cases most, up to EXACTSIZE more or the MAXSIZE limit (AUTO: 20, or a fifth of the predictors when that is more, at most 100); of the subsets passed the best has the lowest pseudo-BIC (minus the mean log-likelihood plus J log(N) / 2N), or with TESTDATA the highest mean log-likelihood of the test cases (the default when TRAININGSAMPLE splits the data). NOSELECTION uses every predictor. A case is put in the target category of highest posterior probability; a missing predictor is left out of a case's product. Cases without a target or without any predictor are left out, as are predictors that are empty or constant. PERCENT draws the training cases from the SET SEED generator; VARIABLE takes cases equal to 1. Frequency weights are rounded (below 0.5 left out). SPLIT FILE is not used.

SAVE adds PredictedValue and PredictedProbability\_1 to \_n (25 by default) for the first n target categories. OUTFILE MODEL writes Statscraft's own JSON model. MEMALLOCATE and TIMER are checked; every case is kept in memory.

## Example

```text
NAIVEBAYES churn BY region plan WITH tenure spend /TRAININGSAMPLE PERCENT=70
  /SUBSET MAXSIZE=3 /CRITERIA BINS=5 /SAVE PREDVAL PREDPROB(prob).
```

`statscraft help NAIVEBAYES` prints the same text in the terminal. [All commands](README.md)
