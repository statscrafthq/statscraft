<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# CSPLAN

Write or show the plan of a complex sample: how to draw it, or how to analyze it.

## Syntax

```text
CSPLAN SAMPLE /PLAN FILE='file' [/PLANVARS [SAMPLEWEIGHT=var] [PREVIOUSWEIGHT=var]]
   [/PRINT [PLAN] [MATRIX]]
   /DESIGN [STAGELABEL='label'] [STRATA=vars] [CLUSTER=vars]
   /METHOD TYPE=SIMPLE_WOR | SIMPLE_WR | SIMPLE_SYSTEMATIC | SIMPLE_CHROMY | PPS_WOR | PPS_WR
               | PPS_SYSTEMATIC | PPS_BREWER | PPS_MURTHY | PPS_SAMPFORD | PPS_CHROMY
               [ESTIMATION=DEFAULT | WR]
   /SIZE VALUE=n | VARIABLE=var | MATRIX=vars; categories n; ...
   | /RATE VALUE=f | VARIABLE=var | MATRIX=vars; categories f; ... [MINSIZE=n] [MAXSIZE=n]
   [/MOS VARIABLE=var | SOURCE=FROMDATA [MIN=m] [MAX=m]]
   [/STAGEVARS [INCLPROB[(var)]] [CUMWEIGHT[(var)]] [INDEX[(var)]] [POPSIZE[(var)]]
              [SAMPSIZE[(var)]] [RATE[(var)]] [WEIGHT[(var)]]]
   [/DESIGN ... /METHOD ... (a second and a third stage)].
CSPLAN ANALYSIS /PLAN FILE='file' /PLANVARS ANALYSISWEIGHT=var [/SRSESTIMATOR TYPE=WOR | WR]
   [/PRINT [PLAN] [MATRIX]]
   /DESIGN [STAGELABEL='label'] [STRATA=vars] [CLUSTER=vars]
   /ESTIMATOR TYPE=EQUAL_WOR | UNEQUAL_WOR | WR
   [/POPSIZE VALUE=n | VARIABLE=var | MATRIX=vars; categories n; ...]
   [/INCLPROB VALUE=p | VARIABLE=var | MATRIX=vars; categories p; ...]
   [/DESIGN ... /ESTIMATOR ... (a second and a third stage)].
CSPLAN VIEW /PLAN FILE='file' [/PRINT [PLAN] [MATRIX]].
```

The plan is a small JSON file that the other CS commands read; CSPLAN does not read the data. Each DESIGN starts a stage, and the subcommands after it belong to that stage.

A stage followed by another needs CLUSTER.

SAMPLE plans are carried out by CSSELECT. Per stage, METHOD says how units are drawn inside each stratum: SIMPLE\_WOR at random without replacement, SIMPLE\_WR with replacement (last stage only), SIMPLE\_SYSTEMATIC every N/n-th unit from a random start, SIMPLE\_CHROMY sequentially with Chromy's method. The PPS methods (first stage only) give each unit a chance in proportion to its measure of size, MOS: a variable, or with SOURCE=FROMDATA the number of cases in the cluster, held between MIN and MAX. PPS\_WOR (Hanurav and Vijayan), PPS\_SAMPFORD, PPS\_BREWER and PPS\_MURTHY (these two draw two units) draw without replacement, need CLUSTER and are analyzed as UNEQUAL\_WOR; PPS\_WR, PPS\_SYSTEMATIC and PPS\_CHROMY are analyzed as WR. A unit too large for its share of the sample is taken for sure. SIZE gives the number of units per stratum, RATE the share (n is the share times the population rounded, then raised to MINSIZE or lowered to MAXSIZE); either one as a value, a variable that holds it, or a matrix with a row per stratum. STAGEVARS renames the variables CSSELECT writes and asks for more: the inclusion probability and the cumulative weight are always written (InclusionProbability\_1\_ and SampleWeightCumulative\_1\_ for stage 1), the duplicate index for a stage drawn with replacement, and on request the population size, sample size, rate and weight of the stage.

The final weight (SAMPLEWEIGHT, default SampleWeight\_Final\_) multiplies the stage weights and PREVIOUSWEIGHT. A sample plan is an analysis plan too: SIMPLE\_WOR stages are analyzed as EQUAL\_WOR with the inclusion probability variable, the PPS methods as said above, the others as WR (and every stage as WR with ESTIMATION=WR, first stage only), so CSDESCRIPTIVES can read the plan of a drawn sample.

ANALYSIS plans describe a sample drawn elsewhere. WR treats the units of the stage as drawn with replacement and ends the design: later stages add nothing to the variance. EQUAL\_WOR treats them as drawn without replacement with equal chances and needs either the population size of each stratum (POPSIZE, whole numbers) or the share of units drawn (INCLPROB, above 0 and at most 1); it brings the finite population correction. UNEQUAL\_WOR (first stage, with CLUSTER) treats the units as drawn without replacement with unequal chances: the analysis reads their inclusion and joint inclusion probabilities from the file CSSELECT writes (see JOINTPROB of CSDESCRIPTIVES). SRSESTIMATOR picks the simple random sampling variance that design effects compare with: WOR (the default) includes the finite population correction, WR does not.

PRINT PLAN (the default) shows the plan, MATRIX the matrix rows.

## Example

```text
CSPLAN SAMPLE /PLAN FILE='orchards_draw.json' /DESIGN STRATA=valley CLUSTER=orchard /METHOD TYPE=SIMPLE_WOR /SIZE VALUE=3 /DESIGN /METHOD TYPE=SIMPLE_SYSTEMATIC /RATE VALUE=0.25 MINSIZE=2.
```

`statscraft help CSPLAN` prints the same text in the terminal. [All commands](README.md)
