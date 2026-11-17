<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# CSSELECT

Draw a complex sample from the active dataset by a plan written by CSPLAN SAMPLE.

## Syntax

```text
CSSELECT /PLAN FILE='file' [/CRITERIA [STAGES=n [n [n]]] [SEED=RANDOM | n]]
   [/CLASSMISSING EXCLUDE | INCLUDE] [/DATA [RENAMEVARS] [PRESORTED]]
   [/SAMPLEFILE OUTFILE='file' | dataset [KEEP=vars] [DROP=vars]]
   [/JOINTPROB OUTFILE='file'] [/PRINT [SELECTION] [CPS]].
```

The active dataset is the frame. Stage by stage, the units (cases, or the combinations of the CLUSTER values) are drawn inside each stratum; a later stage draws only inside the units the earlier one selected. STAGES runs part of the plan: a later stage then needs the cumulative weight variable of the stage before it in the data. A case with a missing stratum or cluster (user-missing ones count under CLASSMISSING=INCLUDE), or a missing size, rate or previous weight, is not part of the frame. FILTER and USE limit the frame; WEIGHT and SPLIT FILE are ignored with a warning.

SEED=n makes the draw repeatable and leaves the session's generator alone. Without it the seed comes from the session's generator when SET SEED was given in the session, so the same syntax draws the same sample again; otherwise from the clock.

Without SAMPLEFILE the new variables join the active dataset, which keeps every case: a case outside the sample has missing weights. SAMPLEFILE writes the selected cases only, with the variables KEEP and DROP leave (applied in their order) and the new ones; a unit drawn several times with replacement appears once per draw, numbered by the index variable. A plan that draws with replacement needs SAMPLEFILE. A new variable whose name already exists is an error; with RENAMEVARS the existing variable is renamed to the first free name of the form name\_1, name\_2, ... with a warning.

Stage inclusion probabilities are n/N (with replacement 1 - (1 - 1/N)^n) and stage weights N/n per draw. PRINT SELECTION (the default) shows per stage and stratum the population size, the sample size and the rate; CPS counts the cases. JOINTPROB is for the methods proportional to size, not supported yet, and is ignored with a warning. Not supported yet: SELECTRULE.

## Example

```text
CSSELECT /PLAN FILE='orchards_draw.json' /CRITERIA SEED=4711 /SAMPLEFILE OUTFILE='orchards_sample.sav' DROP=notes /PRINT SELECTION CPS.
```

`statscraft help CSSELECT` prints the same text in the terminal. [All commands](README.md)
