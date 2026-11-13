<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# CSDESCRIPTIVES

Means, sums and ratios of a complex sample with design-based standard errors and design effects.

## Syntax

```text
CSDESCRIPTIVES /PLAN FILE='file' [/SUMMARY VARIABLES=vars]
   [/MEAN [TTEST=value | values]] [/SUM [TTEST=value | values]]
   [/RATIO NUMERATOR=vars DENOMINATOR=vars [TTEST=value | values]] ...
   [/STATISTICS [COUNT] [POPSIZE] [SE] [CV] [DEFF] [DEFFSQRT] [CIN[(95)]]]
   [/SUBPOP TABLE=var [BY var ...] [DISPLAY=LAYERED | SEPARATE]]
   [/MISSING [SCOPE=ANALYSIS | LISTWISE] [CLASSMISSING=EXCLUDE | INCLUDE]].
```

The design comes from a plan written by CSPLAN; WEIGHT and SPLIT FILE do not apply. A case with a missing weight, stratum, cluster, population size or inclusion probability leaves the design. Totals are weighted sums; a mean is the total over the summed weights and a ratio is one total over another, and their variances come from Taylor linearization. The variance adds, stratum by stratum, the spread of the cluster totals, times the finite population correction under EQUAL\_WOR, plus the variance of later stages. A stratum with one cluster adds nothing. Intervals and t tests use t with the number of first-stage clusters minus the number of first-stage strata as df. The design effect divides the variance by the one simple random sampling would give (see CSPLAN SRSESTIMATOR). Each RATIO crosses its numerators with its denominators. Subpopulations are domains: their estimates use the whole design, with cases outside counting as zero. Under SCOPE=ANALYSIS each statistic uses the cases valid on its own variables, under LISTWISE those valid on all of them; the others count as outside, so the design stays whole. Without STATISTICS: the estimate and its standard error. Not supported yet: JOINTPROB.

## Example

```text
CSDESCRIPTIVES /PLAN FILE='farms_plan.json' /SUMMARY VARIABLES=yield acres /MEAN /RATIO NUMERATOR=yield DENOMINATOR=acres /STATISTICS SE CIN DEFF /SUBPOP TABLE=organic.
```

`statscraft help CSDESCRIPTIVES` prints the same text in the terminal. [All commands](README.md)
