<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# KNN

Nearest neighbor analysis: classify or predict a target from the k most similar training cases.

## Syntax

```text
KNN [target [(MLEVEL=S | O | N)]] [BY factors] [WITH covariates]
    [/EXCEPT VARIABLES=varlist] [/CASELABELS VARIABLE=var] [/FOCALCASES VARIABLE=var]
    [/RESCALE COVARIATE=ADJNORMALIZED | NONE] [/PARTITION TRAINING=70 HOLDOUT=30 | VARIABLE=var]
    [/MODEL [METRIC=EUCLID | CITYBLOCK] [NEIGHBORS=FIXED(K=3) | AUTO(KMIN=3, KMAX=5)]
            [FEATURES=ALL | AUTO(FORCE=varlist)]]
    [/CRITERIA [NUMFEATURES=AUTO | FIXED(n) | ERRORRATIO(MINCHANGE=0.01)] [PREDICTED=MEAN | MEDIAN]
               [WEIGHTFEATURES=NO | YES]] [/CROSSVALIDATION FOLDS=10 | VARIABLE=var]
    [/MISSING USERMISSING=EXCLUDE | INCLUDE] [/VIEWMODEL DISPLAY=YES | NO] [/PRINT CPS | NONE]
    [/SAVE [PREDVAL[(name)]] [PREDPROB[(root)]] [PARTITION[(name)]] [FOLD[(name)]] [MAXCAT(25)]]
    [/OUTFILE [MODEL='file'] [FOCALCASES='file' | dataset]].
```

Names and keywords are written in full, each subcommand once and never empty. Factors are coded with one 0/1 column per category seen in training; covariates are mapped onto -1 to 1 by the training minimum and maximum unless COVARIATE=NONE. Cases missing on the target or a predictor are left out (INCLUDE keeps user-missing codes of categorical ones). The rest are split at random into training and holdout cases by the TRAINING:HOLDOUT ratio (SET SEED makes it repeatable), or by VARIABLE: above 0 trains. A case's neighbors are the k training cases closest to it by EUCLID or CITYBLOCK distance, the case itself never among them; ties keep file order. A categorical target gets the commonest neighbor category (ties: the category with more training cases, then the lower value) and probabilities (count + 1) / (k + J) over the J training categories; a scale target gets the neighbors' MEAN or MEDIAN. NEIGHBORS=AUTO tries each k from KMIN to KMAX: by FOLDS-fold cross-validation of the training cases, or with FEATURES=AUTO by the error left after feature selection. FEATURES=AUTO starts from the FORCE list and adds, one at a time, the predictor whose model errs least, until NUMFEATURES are added (AUTO: up to 20 in all) or, with ERRORRATIO, the relative change of the error is small.

Importance: the error without the predictor plus 1/m, scaled to sum 1; WEIGHTFEATURES=YES multiplies each distance term by it. Tables: Case Processing Summary (CPS), then unless DISPLAY=NO Model Summary, the k or feature selection error log, Feature Importance, Error Summary, Classification Table, and the Nearest Neighbors and Distances of focal cases (FOCALCASES variable above 0, named by CASELABELS). SAVE adds KNN\_PredictedValue, KNN\_Probability\_1 to \_MAXCAT, KNN\_Partition (1 training, 0 holdout) and KNN\_Fold. OUTFILE MODEL writes a JSON model; FOCALCASES writes each focal case's neighbors and distances. WEIGHT is ignored with a warning; SPLIT FILE gives one analysis per group.

## Example

```text
KNN plan (MLEVEL=N) BY region WITH age income /PARTITION TRAINING=8 HOLDOUT=2
  /MODEL NEIGHBORS=AUTO(KMIN=2, KMAX=6) /CROSSVALIDATION FOLDS=5 /SAVE PREDVAL PREDPROB.
```

`statscraft help KNN` prints the same text in the terminal. [All commands](README.md)
