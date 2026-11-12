<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# ROC

ROC curve of test variables against a state, with the area under the curve.

## Syntax

```text
ROC testvars BY state(value)
   [/MISSING=EXCLUDE | INCLUDE]
   [/CRITERIA=[CUTOFF(INCLUDE | EXCLUDE)] [TESTPOS(LARGE | SMALL)] [CI(95)]
              [DISTRIBUTION(FREE | NEGEXPO)]]
   [/PLOT=CURVE[(REFERENCE)] | NONE] [/PRINT=[SE[(MODELQUALITY)]] [COORDINATES]].
```

The test variables are numeric; the state variable may be a string, and the value in parentheses is its positive state, every other valid state negative. Cases with a missing state or test value are left out of every curve (MISSING=INCLUDE keeps user-missing values); weights count cases. TESTPOS(SMALL) makes low test values point to the positive state. Each cutoff lies halfway between neighboring test values, plus one below the smallest and one above the largest; a value equal to a cutoff tests positive unless CUTOFF(EXCLUDE).

Tables: Case Processing Summary; Area Under the Curve, which counts every positive and negative pair (a tie as half) and with SE adds the nonparametric standard error (or the bi-negative exponential one under NEGEXPO when both groups are equally large), the two-sided test of area 0.5 and the CI interval; COORDINATES lists sensitivity and 1 - specificity at every cutoff. CURVE draws the curves in one chart, REFERENCE adds the diagonal; MODELQUALITY adds a bar chart of the lower bounds of the intervals.

## Example

```text
ROC risk_score lab_index BY relapsed (1) /PLOT=CURVE(REFERENCE) /PRINT=SE COORDINATES.
```

`statscraft help ROC` prints the same text in the terminal. [All commands](README.md)
