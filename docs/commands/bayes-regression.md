<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# BAYES REGRESSION

Bayesian linear regression: a Bayes factor for the model, posteriors, predictions and F tests.

## Syntax

```text
BAYES REGRESSION dependent [BY factors] [WITH covariates]
   [/DESIGN [FACTORS=sublist] [COVARIATES=sublist] [REGWGT=var]]
   [/INFERENCE ANALYSIS=POSTERIOR | BAYESFACTOR | BOTH]
   [/ESTBF [COMPUTATION=JZS | ZELLNER(g) | HYPER[(3)] | ROUDER[(1)]] [COMPARE=NULL | FULL]]
   [/PRIOR ...as on BAYES ANOVA] [/PREDICT REGRESSORS=values]
   [/SAVE [MEANPRED[(name)]] [VARIANCE[(name)]] [MODE[(name)]] [CIMEANPREDL[(name)]]
          [CIMEANPREDU[(name)]]] [/OUTFILE PARAMETER='file']
   [/PLOT [COVARIATES=sublist] [FACTORS=sublist] [MAX_CATEGORY=2] [INTERCEPT=FALSE | TRUE]
          [ERRORVAR=FALSE | TRUE] [BAYESPRED=FALSE | TRUE]]
   [/FTEST [LABEL='text'] [VARIABLES=sublist] [INTERCEPT=FALSE | TRUE] [TESTVALUES=values]] ...
   [/CRITERIA [CILEVEL=95] [TOL=0.000001] [MAXITER=2000]].
```

The model holds an intercept, one indicator per category of each factor (ascending; the last is redundant and set to 0) and the covariates; DESIGN lists narrow it. BF10 weighs the model against the intercept alone with the g-priors of BAYES ANOVA. COMPARE=FULL adds a table that weighs each model without one factor or covariate, and the intercept alone, against the full model. The posterior table gives every parameter and the error variance; PRIOR works as on BAYES ANOVA, with MEANVECTOR holding one value per parameter, intercept first, redundant ones included. PREDICT takes one value per factor (a category) and per covariate, factors first, and gives the predictive distribution; SAVE writes its mean, variance, mode and credible bounds for every case with valid regressors (default names such as MEANPRED\_1). Each FTEST tests whether the parameters of its variables (and the intercept) equal TESTVALUES, 0 by default. PLOT draws prior, likelihood and posterior as chart data: of covariates, of the first MAX\_CATEGORY categories of factors, of the intercept, of the error variance, and the predictive density. OUTFILE is accepted and writes no file. Cases missing on any variable of the model are left out.

## Example

```text
BAYES REGRESSION bill_total BY table_zone WITH party_size
  /INFERENCE ANALYSIS=BOTH /ESTBF COMPARE=FULL
  /PREDICT REGRESSORS=2 4 /FTEST LABEL='zone' VARIABLES=table_zone.
```

`statscraft help BAYES REGRESSION` prints the same text in the terminal. [All commands](README.md)
