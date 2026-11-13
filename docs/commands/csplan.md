<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# CSPLAN

Write or show an analysis plan of a complex sample: strata, clusters, weight and estimator per stage.

## Syntax

```text
CSPLAN ANALYSIS /PLAN FILE='file' /PLANVARS ANALYSISWEIGHT=var [/SRSESTIMATOR TYPE=WOR | WR]
   [/PRINT [PLAN] [MATRIX]]
   /DESIGN [STAGELABEL='label'] [STRATA=vars] [CLUSTER=vars]
   /ESTIMATOR TYPE=EQUAL_WOR | UNEQUAL_WOR | WR
   [/POPSIZE VALUE=n | VARIABLE=var | MATRIX=vars; categories n; ...]
   [/INCLPROB VALUE=p | VARIABLE=var | MATRIX=vars; categories p; ...]
   [/DESIGN ... /ESTIMATOR ... (a second and a third stage)].
CSPLAN VIEW /PLAN FILE='file' [/PRINT [PLAN] [MATRIX]].
```

The plan is a small JSON file that CSDESCRIPTIVES and CSTABULATE read; CSPLAN does not read the data. Each DESIGN starts a stage; the ESTIMATOR, POPSIZE and INCLPROB after it belong to that stage. WR treats the units of the stage as drawn with replacement and ends the design: later stages add nothing to the variance. EQUAL\_WOR treats them as drawn without replacement with equal chances and needs either the population size of each stratum (POPSIZE, whole numbers) or the share of units drawn (INCLPROB, above 0 and at most 1), as one value, a variable that holds it, or a matrix with one row per stratum; it brings the finite population correction. A stage followed by another needs CLUSTER. SRSESTIMATOR picks the simple random sampling variance that design effects compare with: WOR (the default) includes the finite population correction, WR does not. PRINT PLAN (the default) shows the plan, MATRIX the matrix rows. Not supported yet: CSPLAN SAMPLE and the UNEQUAL\_WOR estimator in analysis.

## Example

```text
CSPLAN ANALYSIS /PLAN FILE='farms_plan.json' /PLANVARS ANALYSISWEIGHT=wt /DESIGN STRATA=county CLUSTER=parish /ESTIMATOR TYPE=WR.
```

`statscraft help CSPLAN` prints the same text in the terminal. [All commands](README.md)
