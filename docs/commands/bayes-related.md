<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# BAYES RELATED

Bayesian inference on the mean difference of paired measurements.

## Syntax

```text
BAYES RELATED /PAIR VARIABLES=first second [VARIANCE=v] [NULLVALUE=0] [ALTERGVALUE=g]
              [/PAIR ...] [/INFERENCE [ANALYSIS=POSTERIOR | BAYESFACTOR | BOTH]]
              [/PRIOR ...] [/CRITERIA ...] [/MISSING SCOPE=ANALYSIS | LISTWISE].
```

Each pair is analysed as one sample of differences, first minus second, with the rules of BAYES ONESAMPLE DISTRIBUTION=NORMAL: NULLVALUE is the difference tested, VARIANCE a known variance of the differences. PRIOR and CRITERIA take the keywords of BAYES ONESAMPLE except TYPE. LISTWISE drops a case missing on any variable of any pair.

## Example

```text
BAYES RELATED /PAIR VARIABLES=pulse_before pulse_after NULLVALUE=0
  /PAIR VARIABLES=grip_left grip_right /INFERENCE ANALYSIS=BAYESFACTOR.
```

`statscraft help BAYES RELATED` prints the same text in the terminal. [All commands](README.md)
