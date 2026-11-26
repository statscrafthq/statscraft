<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# GENLIN

Generalized linear models: normal, binomial, Poisson, gamma, inverse Gaussian, negative binomial and ordinal responses.

## Syntax

```text
GENLIN dependent [(REFERENCE=LAST | FIRST | value)] [(ORDER=ASCENDING | DESCENDING | DATA)]
   | events OF trials-variable | n
   [BY factors [(ORDER=ASCENDING | DESCENDING | DATA)]] [WITH covariates]
   [/MODEL [effects] [INTERCEPT=YES | NO] [OFFSET=variable | value] [SCALEWEIGHT=variable]
           [DISTRIBUTION=NORMAL | BINOMIAL | GAMMA | IGAUSS | POISSON | NEGBIN[(k | MLE)] | TWEEDIE(q) | MULTINOMIAL]
           [LINK=IDENTITY | LOG | LOGIT | PROBIT | CLOGLOG | NLOGLOG | LOGC | NEGBIN | POWER(a) |
                 ODDSPOWER(a) | CUMLOGIT | CUMPROBIT | CUMCLOGLOG | CUMNLOGLOG | CUMCAUCHIT]]
   [/CRITERIA [ANALYSISTYPE=3 | 1 | ALL [(WALD | LR)]] [CHECKSEP=20] [CILEVEL=95] [CITYPE=WALD | PROFILE[(1E-4)]] [INITIAL=numbers | 'file' | dataset]
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
           [GEF] [HISTORY[(n)]] [LAGRANGE] [LMATRIX] [WORKINGCORR] | NONE]
   [/SAVE [XBPRED] [XBSTDERROR] [MEANPRED] [CIMEANPREDL] [CIMEANPREDU] [PREDVAL] [LEVERAGE] [RESID]
          [PEARSONRESID] [DEVIANCERESID] [STDPEARSONRESID] [STDDEVIANCERESID] [LIKELIHOODRESID] [COOK]
          (each [(name)]; the first five [(root[:n])])]
   [/OUTFILE COVB='file' | dataset | CORB='file' | dataset].
```

The command fits a generalized linear model with factors after BY and covariates after WITH. The response is one dependent variable or events OF trials, where trials is a variable or a whole number of 1 or more. Covariates, the offset and the scale weight are numeric; factors and a categorical dependent may be strings. TO is not accepted: name every variable. Keywords are written in full, and every subcommand but EMMEANS appears at most once. Effects are written as in UNIANOVA.

Without DISTRIBUTION, events/trials is BINOMIAL (the only one it allows) and any other response NORMAL. The default LINK is IDENTITY for NORMAL, LOGIT for BINOMIAL, POWER(-1) for GAMMA, POWER(-2) for IGAUSS, LOG for POISSON and NEGBIN, POWER(1-q) for TWEEDIE, CUMLOGIT for MULTINOMIAL. MULTINOMIAL (an ordinal response) allows only the CUM links, BINOMIAL every other link but NEGBIN, NEGBIN also its own link unless k is 0, and the rest IDENTITY, LOG or POWER; another pair is an error. NEGBIN alone fixes k at 1, NEGBIN(MLE) estimates it. TWEEDIE(q), q above 1 and below 2, is the compound Poisson-gamma distribution with the variance function mu^q (zero or positive responses, the scale as for GAMMA).

A binary dependent must have two values; the model gives the probability of the one that is not the REFERENCE (LAST by default, in the ORDER given). A factor has one column per level, and the column of its last level is redundant and shown as 0. The ordinal model has thresholds instead of an intercept.

A case is left out when its rounded weight is below 1, a variable is missing, the scale weight is not positive, or the response is out of range: not above 0 for GAMMA and IGAUSS, below 0 for TWEEDIE, not a whole number of 0 or more for POISSON and NEGBIN, or events above trials. CLASSMISSING=INCLUDE keeps user-missing factor values.

Without PRINT the output is MODELINFO, CPS, DESCRIPTIVES, FIT, SUMMARY and SOLUTION; a PRINT list replaces this set, and NONE stands alone. FIT gives the deviance, Pearson chi-square, log likelihood (full or KERNEL) and information criteria. SUMMARY gives the Omnibus Test against the intercept-only model and the Tests of Model Effects: Type III by default (ANALYSISTYPE=1, 3 or ALL), Wald unless (LR) is given. SOLUTION gives the Parameter Estimates with Wald intervals; (EXPONENTIATED) adds Exp(B). COVB and CORB add the matrices of the estimates.

METHOD=FISHER(n) uses Fisher scoring for the first n iterations and Newton-Raphson after: FISHER(1) is the default, FISHER alone scores throughout. A step is halved, at most MAXSTEPHALVING times, while the log likelihood falls. At least one of HCONVERGE, LCONVERGE and PCONVERGE must be above 0. SCALE=MLE estimates the scale; it is the default for NORMAL, GAMMA, IGAUSS and TWEEDIE and an error for the others, which fix it at 1. DEVIANCE or PEARSON set it to that chi-square over its df, then divide the likelihood ratio statistics by it and add an F test. A number fixes the scale. COVB=ROBUST gives the sandwich covariance.

EMMEANS gives the estimated marginal means of the TABLES cells, covariates at their means or the CONTROL values, on the response scale (ORIGINAL) or the linear predictor (TRANSFORMED). COMPARE takes factors from TABLES and adds pairwise comparisons, or the CONTRAST given, adjusted by PADJUST (LSD by default), with an overall Wald test. CONTRAST needs COMPARE, and crossed COMPARE factors allow only PAIRWISE.

SAVE names each variable as given or after its keyword (XBPredicted, MeanPredicted, Residual and so on, with the suffix \_1 when taken). For a multinomial response the first five keywords give one variable per category, at most n (25 by default).

REPEATED fits generalized estimating equations for cases correlated within a subject: one combination of the SUBJECT values (required). WITHINSUBJECT values order a subject's cases, and a value a subject lacks is a missing measurement; without it the file order is kept. SORT=NO takes each run of equal SUBJECT values as a subject and ignores WITHINSUBJECT. A frequency weight must be equal within a subject. CORRTYPE is INDEPENDENT by default; the other estimated structures use the Pearson residuals, and ADJUSTCORR=YES (the default) subtracts the number of parameters in each denominator. FIXED lists the correlations below the diagonal row by row, each from 0 to 1. UPDATECORR=n re-estimates the matrix every n iterations (0 never), at most MAXITERATIONS.

With REPEATED, its own COVB applies (ROBUST by default) and the CRITERIA COVB is ignored. SCALE=MLE becomes the Pearson estimate; another scale and an estimated k keep the value of the model without REPEATED. FIT shows QIC and QICC, SUMMARY only the Wald tests of the effects, CPS adds the Correlated Data Summary, and WORKINGCORR (used only here) the working correlation matrix. (LR) gives generalized score chi-squares. SAVE gives no leverage, deviance-based or standardized residuals, or Cook's distance.

LAGRANGE tests a fixed scale (NORMAL, GAMMA or IGAUSS) or a fixed NEGBIN k with 1 df; for k = 0 it gives the z of the dispersion regression. HISTORY(n) lists every n-th iteration and the last, then the final gradient and Hessian. GEF shows the general estimable function and LMATRIX the L' matrix of each test and marginal mean, neither for the ordinal model.

INITIAL gives numbers for all parameters in table order, redundant ones included; another count is an error. A file or dataset gives them in its first record with RowType\_ EST (the variables after VarName\_, in order, the scale parameter and the negative binomial parameter after the others and not used; a file of several SPLIT FILE groups is read group by group). INITIAL is ignored with a warning for NORMAL with IDENTITY and for the ordinal model. OUTFILE writes such a file, a .sav file or a dataset made with DATASET DECLARE: RowType\_, VarName\_ and a column P1, P2, ... for every parameter in table order (a redundant one blank), the scale parameter and, for NEGBIN, k. COVB gives a COV record per estimated parameter with its covariances, CORB a CORR record with its correlations; then come the records EST (the estimates, the redundant ones 0), SE, SIG (the Wald significances) and DF (1 for each estimated parameter). MODEL and PARAMETER (XML files) are not written. CITYPE=PROFILE replaces the Wald intervals of the parameters, Exp(B) and an estimated scale or k with profile likelihood intervals, refitting the other parameters (a DEVIANCE or PEARSON scale is held); its tolerance sets how closely the limits are solved, and a limit never reached is left empty. The ordinal model and REPEATED keep Wald intervals with a warning.

## Example

```text
GENLIN visits BY clinic WITH age /MODEL clinic age DISTRIBUTION=NEGBIN(MLE) LINK=LOG OFFSET=logdays /PRINT FIT SUMMARY SOLUTION(EXPONENTIATED) /EMMEANS TABLES=clinic COMPARE=clinic.
```

`statscraft help GENLIN` prints the same text in the terminal. [All commands](README.md)
