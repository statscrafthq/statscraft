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
           [CLASSTABLE] [NONE]].
```

The categories of the dependent variable, sorted up (or down with DESCENDING), are ordered 1 to K. For each k below K the model says link(P(Y \<= k)) = threshold\_k - x'b: one threshold per cut point and one set of coefficients b for all of them, so a positive coefficient moves cases toward the higher categories. LOGIT is the default link. The estimates maximize the weighted log likelihood by Newton-Raphson with step-halving; METHOD=FISHER(n) takes n Fisher scoring steps first (FISHER alone: until convergence), then Newton-Raphson. The start is b = 0 with each threshold at the link of the weighted cumulative share of its category. The covariance is a sandwich: the inverse observed information on both sides of the design covariance of the score total. Design, df, domains and TEST work as in CSGLM; the Tests of Model Effects test the coefficients of each effect (and all of them in the corrected model). The model summary gives the pseudo R squares of Cox and Snell, Nagelkerke and McFadden against the thresholds-only fit, with the summed weights as N. EXP needs the logit link. CLASSTABLE puts each case in its most probable category and sums the weights. Without STATISTICS no parameter table is shown. Not supported yet: CUSTOM, ODDSRATIOS, NONPARALLEL, SAVE, JOINTPROB, OUTFILE, PRINT GEF and the UNEQUAL\_WOR estimator.

## Example

```text
CSORDINAL rating BY region WITH age /PLAN FILE='poll_plan.json' /LINK FUNCTION=PROBIT /STATISTICS PARAMETER SE CINTERVAL TTEST /PRINT SUMMARY HISTORY.
```

`statscraft help CSORDINAL` prints the same text in the terminal. [All commands](README.md)
