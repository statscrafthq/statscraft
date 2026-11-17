<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# SIMRUN

Runs a simulation plan: draws the inputs, computes the targets and describes their distributions.

## Syntax

```text
SIMRUN /PLAN FILE='file' [/CRITERIA [REPRESULTS=TRUE | FALSE] [SEED=629111597]]
  [/DISTRIBUTION [DISPLAY=[PDF] [CDF] [NONE]] [CDFORDER=[ASCENDING] [DESCENDING]] [SCALE=...] [CATEGORICAL=...]]
  [/SCATTERPLOT [DISPLAY=NO | YES] [ROWSIZE=n]] [/BOXPLOT [DISPLAY=NO | YES] [OVERLAYTARGETS=...]]
  [/TORNADO [DISPLAY=YES | NO] [TYPE=[CORR] [DELTASTDDEV(k)] [VARIANCE]]]
  [/PRINT [DESCRIPTIVES=YES[(CILEVEL=95)] | NO] [PERCENTILES=NO | YES([NTILES=k] [PTILES=values])]
          [ASSOCIATIONS=NO | YES]]
  [/OPTIONS VIZSTYLESHEET='name'] [/OUTFILE FILE='file' | dataset | *].
```

Each case draws standard normal numbers from a Mersenne Twister started by SEED, correlates them (the correlation of each pair of normals is searched so that the inputs, after the inverse distribution function, reach the planned correlation) and turns them into the inputs; the model's expressions then give the targets. Cases are drawn up to MAXCASES, or until the stopping rule of the plan holds, for each value of a sensitivity analysis. Tables: Target Statistics (mean, standard deviation, median, minimum, maximum, the confidence interval of the mean, 5th and 95th percentiles), Target Categories for an ordinal or nominal target, Input Statistics, Target Percentiles, Correlations of Inputs (ASSOCIATIONS), and the tornado tables: the correlation with each input, the mean change of the target when an input moves DELTASTDDEV standard deviations down and up, and each input's total share of the target's variance (Sobol, from a second sample). Charts: the histogram (PDF) and the cumulative distribution (CDF) of each target, the boxplot, scatterplots of each target with each input (the first 5000 cases) and tornado bar charts. SCALE, CATEGORICAL, ROWSIZE, OVERLAYTARGETS and OPTIONS are accepted and change nothing. OUTFILE writes every case: the targets, the inputs and, with a sensitivity analysis, Iteration. Without a model only the input statistics, ASSOCIATIONS and OUTFILE apply.

## Example

```text
SIMRUN /PLAN FILE='margin.splan' /CRITERIA SEED=4711
  /TORNADO TYPE=CORR VARIANCE /PRINT PERCENTILES=YES(NTILES=4) /OUTFILE FILE='draws.sav'.
```

`statscraft help SIMRUN` prints the same text in the terminal. [All commands](README.md)
