<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# BAYES REPEATED

Bayesian one-way repeated measures analysis: a Bayes factor and posteriors of the condition means.

## Syntax

```text
BAYES REPEATED varlist
   [/INFERENCE ANALYSIS=POSTERIOR | BAYESFACTOR | BOTH] [/ESTBF COMPUTATION=BIC | ROUDER]
   [/DESIGN REGWGT=var] [/PLOT MEANS=sublist]
   [/CRITERIA [CILEVEL=95] [SEED=RANDOM | n] [MCSAMPLES=30000]].
```

Each case is a subject measured once under each of the two or more variables; a case missing on any of them is left out. BF10 weighs different condition means against equal ones with the subjects as a random effect. BIC uses a closed-form approximation from the sums of squares of conditions, subjects and the total. ROUDER averages the factor of a mixed model over MCSAMPLES draws of the two scale parameters, from SEED; it uses the regression weights only. The posterior of each condition mean is the normal large-sample approximation, which needs no prior. PLOT draws the posterior densities of the listed means as chart data.

## Example

```text
BAYES REPEATED trial1 trial2 trial3 trial4
  /INFERENCE ANALYSIS=BOTH /ESTBF COMPUTATION=ROUDER /CRITERIA SEED=731.
```

`statscraft help BAYES REPEATED` prints the same text in the terminal. [All commands](README.md)
