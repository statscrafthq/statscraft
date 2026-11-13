<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# GENLIN

Generalized linear models: normal, binomial, Poisson, gamma, inverse Gaussian, negative binomial and ordinal responses.

## Syntax

```text
GENLIN dependent [(REFERENCE=LAST | FIRST | value)] [(ORDER=ASCENDING | DESCENDING | DATA)]
   | events OF trials-variable | n
   [BY factors [(ORDER=ASCENDING | DESCENDING | DATA)]] [WITH covariates]
   [/MODEL [effects] [INTERCEPT=YES | NO] [OFFSET=variable | value] [SCALEWEIGHT=variable]
           [DISTRIBUTION=NORMAL | BINOMIAL | GAMMA | IGAUSS | POISSON | NEGBIN[(k | MLE)] | MULTINOMIAL]
           [LINK=IDENTITY | LOG | LOGIT | PROBIT | CLOGLOG | NLOGLOG | LOGC | NEGBIN | POWER(a) |
                 ODDSPOWER(a) | CUMLOGIT | CUMPROBIT | CUMCLOGLOG | CUMNLOGLOG | CUMCAUCHIT]]
   [/CRITERIA [ANALYSISTYPE=3 | 1 | ALL [(WALD | LR)]] [CHECKSEP=20] [CILEVEL=95] [CITYPE=WALD]
              [COVB=MODEL | ROBUST] [HCONVERGE=0] [LCONVERGE=0] [PCONVERGE=1E-6] (each [(ABSOLUTE | RELATIVE)])
              [LIKELIHOOD=FULL | KERNEL] [MAXITERATIONS=100] [MAXSTEPHALVING=5]
              [METHOD=FISHER[(n)] | NEWTON] [SCALE=MLE | DEVIANCE | PEARSON | value] [SINGULAR=1E-12]]
   [/EMMEANS [TABLES=factor[*factor...]] [CONTROL=covariate(value | MEAN) ...]
             [SCALE=ORIGINAL | TRANSFORMED] [COMPARE=factor[*factor...]]
             [CONTRAST=PAIRWISE | DEVIATION[(value)] | DIFFERENCE | HELMERT | POLYNOMIAL[(metric)] |
                       REPEATED | SIMPLE[(value)]]
             [PADJUST=LSD | BONFERRONI | SEQBONFERRONI | SIDAK | SEQSIDAK]] ...
   [/REPEATED SUBJECT=variable[*variable...] [WITHINSUBJECT=variable[*variable...]] [SORT=YES | NO]
             [CORRTYPE=INDEPENDENT | AR(1) | EXCHANGEABLE | MDEPENDENT(m) | UNSTRUCTURED | FIXED(list)]
             [ADJUSTCORR=YES | NO] [COVB=ROBUST | MODEL] [HCONVERGE=0] [MAXITERATIONS=100]
             [PCONVERGE=1E-6[(ABSOLUTE | RELATIVE)]] [UPDATECORR=1]]
   [/MISSING CLASSMISSING=EXCLUDE | INCLUDE]
   [/PRINT [CPS] [DESCRIPTIVES] [MODELINFO] [FIT] [SUMMARY] [SOLUTION[(EXPONENTIATED)]] [COVB] [CORB]
           [WORKINGCORR] | NONE]
   [/SAVE [XBPRED] [XBSTDERROR] [MEANPRED] [CIMEANPREDL] [CIMEANPREDU] [PREDVAL] [LEVERAGE] [RESID]
          [PEARSONRESID] [DEVIANCERESID] [STDPEARSONRESID] [STDDEVIANCERESID] [LIKELIHOODRESID] [COOK]].
```

Every keyword is written in full and every subcommand but EMMEANS appears at most once. The response is one dependent variable or a count of events out of a number of trials (a variable or a constant); covariates, offset and scale weight are numeric, factors and a categorical dependent may be strings. Without DISTRIBUTION, events/trials is binomial and anything else normal; without LINK each distribution takes its own (identity, logit, inverse, inverse square, log, cumulative logit), and pairs the model cannot use are an error. A case is left out when its rounded weight is below 1, a variable is missing (CLASSMISSING=INCLUDE keeps user-missing factor values), the scale weight is not positive, or the response is out of range: not above 0 for GAMMA and IGAUSS, not a whole number of 0 or more for POISSON and NEGBIN, events above the trials. A binary dependent must have two values; the procedure models the lower one unless REFERENCE names another reference.

Effects are written as in UNIANOVA; a factor has one column per level and the column of the last level (in the ORDER given) is redundant and shown as 0. The estimates maximize the log likelihood by Newton steps from weighted least squares start values: Fisher scoring for the first n iterations of METHOD=FISHER(n) (1 by default), the observed information after; a step is halved while the log likelihood falls. The scale is estimated with the coefficients (MLE, the default for NORMAL, GAMMA and IGAUSS), set from the deviance or Pearson chi-square over its degrees of freedom, or fixed (1 for the other distributions). NEGBIN(MLE) estimates k jointly; NEGBIN alone fixes k at 1. ROBUST gives the sandwich covariance.

Tables: Model Information, Case Processing Summary, Categorical and Continuous Variable Information, Goodness of Fit (deviance and Pearson chi-square, raw and scaled, over the subpopulations of a binary response; the log likelihood, full or KERNEL; AIC, AICC, BIC and CAIC), Omnibus Test (a likelihood ratio test against the intercept-only model), Tests of Model Effects (Type I and III, Wald or likelihood ratio; with SCALE=DEVIANCE or PEARSON the likelihood ratio is divided by the scale and an F test added) and Parameter Estimates with Wald intervals and Exp(B), then the covariance and correlation of the estimates. The ordinal multinomial model puts the link of the cumulative probability at each threshold minus the effects, without an intercept. EMMEANS gives the predicted means of the factor cells (covariates at their means or CONTROL values), on the response or the linear predictor scale, with pairwise or contrast comparisons and an overall Wald test. SAVE names the new variables after the keyword (XBPredicted, MeanPredicted, Residual and so on, a suffix \_1 when taken) or as given; the multinomial ones are numbered per category.

REPEATED fits generalized estimating equations for measurements that are correlated within a subject. A subject is a combination of the SUBJECT values; WITHINSUBJECT values put its cases in order, and a value a subject lacks counts as a missing measurement. Without WITHINSUBJECT the file order is kept. SORT=NO takes each run of equal SUBJECT values in the file as one subject and ignores WITHINSUBJECT. The fit starts from the plain model and then updates the estimates and the working correlation matrix in turn: AR(1), EXCHANGEABLE, MDEPENDENT(m) and UNSTRUCTURED are estimated from the Pearson residuals (with ADJUSTCORR=YES each count in a denominator is less the number of parameters), FIXED takes the correlations below the diagonal row by row. UPDATECORR=n re-estimates the matrix every n iterations, 0 never. SCALE=MLE becomes the Pearson moment of the residuals; another scale and an estimated NEGBIN k stay at the plain model's value. COVB=ROBUST (the default) gives sandwich standard errors, MODEL the model-based ones; the CRITERIA COVB is not used. FIT then shows QIC and QICC instead of the likelihood statistics, SUMMARY only the Wald tests of the effects, CPS adds the Correlated Data Summary, WORKINGCORR the Working Correlation Matrix. A frequency weight must be the same for all cases of a subject. SAVE has no leverage, deviance-based or standardized residuals or Cook's distance here.

Not supported yet: OUTFILE, TWEEDIE, INITIAL, CITYPE=PROFILE, PRINT GEF, HISTORY, LAGRANGE and LMATRIX, and with REPEATED the multinomial distribution and generalized score tests (ANALYSISTYPE with LR).

## Example

```text
GENLIN visits BY clinic WITH age /MODEL clinic age DISTRIBUTION=NEGBIN(MLE) LINK=LOG OFFSET=logdays /PRINT FIT SUMMARY SOLUTION(EXPONENTIATED) /EMMEANS TABLES=clinic COMPARE=clinic.
```

`statscraft help GENLIN` prints the same text in the terminal. [All commands](README.md)
