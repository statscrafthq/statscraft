<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# BAYES INDEPENDENT

Bayesian comparison of the means of two independent groups.

## Syntax

```text
BAYES INDEPENDENT /INFERENCE VARIABLES=varlist GROUP=var
                             SELECT=LEVEL(a b) | MIDPOINT | CUTPOINT(v)
                             [ANALYSIS=POSTERIOR | BAYESFACTOR | BOTH] [DATAVAR=v1 v2]
   [/ESTBF COMPUTATION=ROUDER[(1)] | GONEN([0] variance) | HYPER[(-0.75)]]
   [/PRIOR [EQUALDATAVAR=FALSE | TRUE] [VARDIST=DIFFUSE | JEFFREYS | INVCHISQ(df scale)]
           [MEANDIST=DIFFUSE | NORMAL(mean1 mean2 variance1 variance2)]]
   [/CRITERIA [CILEVEL=95] [TOL=0.000001] [MAXITER=2000]] [/MISSING SCOPE=ANALYSIS | LISTWISE].
```

LEVEL takes the two values of GROUP that form group 1 and group 2; MIDPOINT (the middle of the range of GROUP) and CUTPOINT put values below the point in group 1 and the rest in group 2. The difference is group 2 minus group 1. Bayes factors use the pooled t: ROUDER gives BF01 for a Cauchy prior of that scale on the effect size, GONEN BF01 for a normal prior with that mean and variance, HYPER BF10 for a hyper-prior with parameter a (above -1, at most -0.5). The posterior of the difference: with DATAVAR (known variances) normal, from flat or NORMAL mean priors; else with EQUALDATAVAR=TRUE a t from VARDIST=JEFFREYS or INVCHISQ, and in all other cases the unequal-variances posterior of flat priors (each group needs more than five cases). Weights as on BAYES ONESAMPLE.

## Example

```text
BAYES INDEPENDENT /INFERENCE VARIABLES=yield GROUP=fertilizer SELECT=LEVEL(1 2)
  ANALYSIS=BOTH /ESTBF COMPUTATION=GONEN(0.5)
  /PRIOR EQUALDATAVAR=TRUE VARDIST=JEFFREYS.
```

`statscraft help BAYES INDEPENDENT` prints the same text in the terminal. [All commands](README.md)
