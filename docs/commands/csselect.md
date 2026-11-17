<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# CSSELECT

Draw a complex sample from the active dataset by a plan written by CSPLAN SAMPLE.

## Syntax

```text
CSSELECT /PLAN FILE='file' [/CRITERIA [STAGES=n [n [n]]] [SEED=RANDOM | n]]
   [/CLASSMISSING EXCLUDE | INCLUDE] [/DATA [RENAMEVARS] [PRESORTED]]
   [/SAMPLEFILE OUTFILE='file' | dataset [KEEP=vars] [DROP=vars]]
   [/JOINTPROB OUTFILE='file'] [/SELECTRULE OUTFILE='file'] [/PRINT [SELECTION] [CPS]].
```

The active dataset is the frame. Stage by stage, the units (cases, or the combinations of the CLUSTER values) are drawn inside each stratum; a later stage draws only inside the units the earlier one selected. STAGES runs part of the plan: a later stage then needs the cumulative weight variable of the stage before it in the data. A case with a missing stratum or cluster (user-missing ones count under CLASSMISSING=INCLUDE), or a missing size, rate or previous weight, is not part of the frame. FILTER and USE limit the frame; WEIGHT and SPLIT FILE are ignored with a warning.

SEED=n makes the draw repeatable and leaves the session's generator alone. Without it the seed comes from the session's generator when SET SEED was given in the session, so the same syntax draws the same sample again; otherwise from the clock.

Without SAMPLEFILE the new variables join the active dataset, which keeps every case: a case outside the sample has missing weights. SAMPLEFILE writes the selected cases only, with the variables KEEP and DROP leave (applied in their order) and the new ones; a unit drawn several times with replacement appears once per draw, numbered by the index variable. A plan that draws with replacement needs SAMPLEFILE. A new variable whose name already exists is an error; with RENAMEVARS the existing variable is renamed to the first free name of the form name\_1, name\_2, ... with a warning.

Stage inclusion probabilities are n/N (with replacement 1 - (1 - 1/N)^n) and stage weights N/n per draw; with a PPS method n times the unit's share of the total size (PPS\_WR: 1 - (1 - share)^n, weight one over n times the share; PPS\_MURTHY its own formula), and a measure of size must be the same in all cases of a unit. PRINT SELECTION (the default) shows per stage and stratum the population size, the sample size and the rate; CPS counts the cases. A first stage drawn by PPS\_WOR, PPS\_BREWER, PPS\_MURTHY or PPS\_SAMPFORD also writes the joint inclusion probabilities of the selected units to a data file: JOINTPROB OUTFILE, or else the plan's name with the extension .joint.sav. It holds the first-stage strata and clusters, Unit\_No\_ (the unit's number in its stratum) and Joint\_Prob\_k\_ for k = 1, 2 and so on (the joint probability with unit k; the inclusion probability on the diagonal); with other plans JOINTPROB is ignored with a warning.

SELECTRULE OUTFILE writes a text file (replacing one of that name) with a condition that is true for the units drawn by the last stage run that has clusters, ready to turn into a SELECT IF or a database query for the next stage. Each unit is a line of comparisons (name EQ value) on the strata and clusters of its stage and of the stages above it, joined by AND; the lines are joined by OR; string values are quoted. When no stage run has clusters, the cases themselves are the units and nothing names them, so no file is written and a warning says so.

## Example

```text
CSSELECT /PLAN FILE='orchards_draw.json' /CRITERIA SEED=4711 /SAMPLEFILE OUTFILE='orchards_sample.sav' DROP=notes /PRINT SELECTION CPS.
```

`statscraft help CSSELECT` prints the same text in the terminal. [All commands](README.md)
