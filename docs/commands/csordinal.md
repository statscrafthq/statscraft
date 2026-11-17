<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# CSORDINAL

Ordinal regression (cumulative link model) on a complex sample, with design-based standard errors and tests.

## Syntax

```text
CSORDINAL dependent [(ASCENDING | DESCENDING)] [BY factors] [WITH covariates] /PLAN FILE='file'
   [/MODEL effects] [/LINK FUNCTION=LOGIT | PROBIT | CLOGLOG | NLOGLOG | CAUCHIT]
   [/CRITERIA [CHKSEP=20] [CILEVEL=95] [DF=n] [LCONVERGE=[0 RELATIVE | ABSOLUTE]]
              [METHOD=NEWTON | FISHER[(n)]] [MXITER=100] [MXSTEP=5]
              [PCONVERGE=[1E-6 RELATIVE | ABSOLUTE]] [SINGULAR=1E-12]]
   [/STATISTICS [PARAMETER] [EXP] [SE] [TTEST] [CINTERVAL] [DEFF] [DEFFSQRT]]
   [/TEST [TYPE=F | ADJF | CHISQUARE | ADJCHISQUARE]
          [PADJUST=LSD | BONFERRONI | SEQBONFERRONI | SIDAK | SEQSIDAK]]
   [/DOMAIN VARIABLE=var(value)] [/MISSING CLASSMISSING=EXCLUDE | INCLUDE]
   [/PRINT [SAMPLEINFO] [VARIABLEINFO] [SUMMARY] [HISTORY[(n)]] [LMATRIX] [COVB] [CORB]
           [CLASSTABLE] [NONE]]
   [/CUSTOM [LABEL='text'] LMATRIX=[thresholds] effect coefficients ... [; ...]
            | ALL coefficients [; ...] [KMATRIX=value [; value ...]]] ...
   [/ODDSRATIOS FACTOR=[factor(LOW | HIGH | value) ...] | COVARIATE=[covariate(changes) ...]
                [CONTROL=[factor(LOW | HIGH | value) covariate(MEAN | value) ...]]] ...
   [/NONPARALLEL [TEST] [PARAMETER] [COVB]]
   [/SAVE [CUMPROB[(root[:n])]] [PREDPROB[(root[:n])]] [PREDVAL[(name)]]
          [PREDVALPROB[(name)]] [OBSVALPROB[(name)]]].
```

The categories of the dependent variable, sorted up (or down with DESCENDING), are ordered 1 to K. For each k below K the model says link(P(Y \<= k)) = threshold\_k - x'b: one threshold per cut point and one set of coefficients b for all of them, so a positive coefficient moves cases toward the higher categories. LOGIT is the default link. The estimates maximize the weighted log likelihood by Newton-Raphson with step-halving; METHOD=FISHER(n) takes n Fisher scoring steps first (FISHER alone: until convergence), then Newton-Raphson. The start is b = 0 with each threshold at the link of the weighted cumulative share of its category. The covariance is a sandwich: the inverse observed information on both sides of the design covariance of the score total. Design, df, domains and TEST work as in CSGLM; the Tests of Model Effects test the coefficients of each effect (and all of them in the corrected model). The model summary gives the pseudo R squares of Cox and Snell, Nagelkerke and McFadden against the thresholds-only fit, with the summed weights as N. EXP needs the logit link. CLASSTABLE puts each case in its most probable category and sums the weights. Without STATISTICS no parameter table is shown. A CUSTOM row may open with one coefficient per threshold before its effects; ALL takes the thresholds first, then every design column but the intercept. Moving all thresholds together is the same as moving the intercept, so a row is tested only when its coefficients pass as estimable with minus their threshold sum on the intercept: a lone threshold is not testable once there are factors. ODDSRATIOS (logit link only) gives one cumulative odds ratio per comparison, exp(-b'(x1 - x2)), x1 the compared level or the covariate raised by the change; other variables sit at their CONTROL values (factors at the highest level, covariates at the mean by default). NONPARALLEL refits the model with a set of coefficients per threshold, starting from the estimates; TEST is the Wald test that each coefficient is the same for every threshold, PARAMETER and COVB show that model. SAVE writes cumulative and category probabilities, the most probable category and its probability, and the probability of the observed category; a domain case whose dependent is missing still gets predictions when every predictor has a parameter. Default names: CumulativeProbability\_k, PredictedProbability\_k, PredictedValue, PredictedValueProbability, ObservedValueProbability, with a suffix when taken. Not supported yet: JOINTPROB, OUTFILE, PRINT GEF and the UNEQUAL\_WOR estimator.

## Example

```text
CSORDINAL rating BY region WITH age /PLAN FILE='poll_plan.json' /LINK FUNCTION=PROBIT /STATISTICS PARAMETER SE CINTERVAL TTEST /PRINT SUMMARY HISTORY.
```

`statscraft help CSORDINAL` prints the same text in the terminal. [All commands](README.md)
