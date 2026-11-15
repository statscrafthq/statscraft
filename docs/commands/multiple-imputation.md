<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# MULTIPLE IMPUTATION

Multiple imputation: several completed copies of the data with plausible values in place of the missing ones.

## Syntax

```text
MULTIPLE IMPUTATION varlist
   [/IMPUTE [METHOD=AUTO | FCS | MONOTONE | NONE] [NIMPUTATIONS=5] [SCALEMODEL=LINEAR | PMM[(5)]]
           [INTERACTIONS=NONE | TWOWAY] [MAXPCTMISSING=NONE | pct] [MAXCASEDRAWS=50]
           [MAXPARAMDRAWS=2] [MAXITER=10] [SINGULAR=1E-12] [MAXMODELPARAM=100]]
   [/CONSTRAINTS varlist ([MIN=NONE | value] [MAX=NONE | value] [RND=NO | step]
                          [ROLE=BOTH | DEP | IND])] ...
   [/MISSINGSUMMARIES [OVERALL] [VARIABLES[(MAXVARS=25 MINPCTMISSING=10)]] [PATTERNS] | NONE]
   [/IMPUTATIONSUMMARIES [MODELS] [DESCRIPTIVES[(varlist)]] | NONE]
   [/ANALYSISWEIGHT var]
   [/OUTFILE [IMPUTATIONS='file' | dataset] [FCSITERATIONS='file' | dataset]].
```

Fills in the missing values of the listed variables several times over and writes every completed copy, stacked under the original cases, to IMPUTATIONS (needed unless METHOD=NONE).

The new first variable Imputation\_ is 0 for the original cases and 1 to n for the copies, and the new data are split by it. Keywords are written in full and their equals signs are needed.

Each variable is imputed from the others by its own model: linear regression for a scale variable (PMM instead takes the observed value of one of the k cases whose predictions lie nearest), multinomial logistic regression for a nominal, ordinal or string one; model parameters are drawn from their posterior first, so the copies differ. Categorical predictors enter as indicators, with TWOWAY also their pairwise products. MONOTONE imputes once in list order, each variable from the ones before it, and needs a monotone pattern of missing values; FCS starts from random values and cycles through the variables MAXITER times, each one from all the others; AUTO orders the variables from fewest to most missing, takes MONOTONE when the pattern allows it and FCS (10 rounds, linear, main effects) otherwise. A variable with more than MAXPCTMISSING percent missing is left out. CONSTRAINTS: MIN and MAX bound linear imputations (a value outside is drawn again, up to MAXCASEDRAWS times, then the parameters, up to MAXPARAMDRAWS times), RND rounds them to a multiple, ROLE=DEP imputes without predicting and ROLE=IND predicts and is imputed only for that. User-missing and system-missing values are both filled; cases with nothing valid, a weight under 0.5 or an ANALYSISWEIGHT of 0 or less keep theirs. WEIGHT counts cases (rounded); ANALYSISWEIGHT weights the models. Random draws follow SET SEED and SET RNG. FCSITERATIONS saves the mean and standard deviation of the imputed values of each scale variable after every round.

Tables: the overall missing summary, VARIABLES (the most missing first, at least MINPCTMISSING percent), PATTERNS, the specifications, constraints and results, MODELS and DESCRIPTIVES of the original, imputed and completed values. A summary subcommand shows only the keywords on it.

Pooling: with the data split by Imputation\_, DESCRIPTIVES adds a table of pooled means by Rubin's rules (standard error, df, 95% interval, fraction of missing information, relative increase in variance, relative efficiency) and FREQUENCIES a table of the frequencies and percents averaged over the imputations. MEANS pools the mean of every row of its reports, T-TEST the mean differences, CORRELATIONS each correlation (on Fisher's z scale), REGRESSION, LOGISTIC REGRESSION, PLUM and NOMREG the coefficients; REGRESSION and LOGISTIC REGRESSION also get an F test that all their slopes are 0, pooled from the covariance matrices of the coefficients (Li, Raghunathan and Rubin 1991). SET MIOUTPUT chooses which of the original, imputed and pooled results appear and whether the pooling diagnostics do.

## Example

```text
MULTIPLE IMPUTATION rent hhsize tenure commute /IMPUTE METHOD=FCS NIMPUTATIONS=10 /CONSTRAINTS hhsize (MIN=1 RND=1) /OUTFILE IMPUTATIONS='filled.sav'.
```

`statscraft help MULTIPLE IMPUTATION` prints the same text in the terminal. [All commands](README.md)
