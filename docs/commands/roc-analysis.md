<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# ROC ANALYSIS

ROC curves with paired or two-group comparisons of their areas, precision-recall curves and classifier metrics.

## Syntax

```text
ROC ANALYSIS testvars BY state(value)
   [/MISSING USERMISSING=EXCLUDE | INCLUDE]
   [/CRITERIA [CUTOFF=INCLUDE | EXCLUDE] [TESTPOS=LARGE | SMALL] [CI=95]
              [DISTRIBUTION=FREE | NEGEXPO]]
   [/DESIGN [PAIR=FALSE | TRUE] [GROUP=var]
            [SELECT=LEVEL(value value) | MIDPOINT | CUTPOINT(value)]]
   [/PLOT [CURVE=ROC[(REFERENCE)] [PR[(REFERENCE)]] | NONE] [PR_INTERPOLATE=TP | FP]
          [MODELQUALITY=FALSE | TRUE]]
   [/PRINT [SE=FALSE | TRUE] [COORDINATES=[ROC] [PR]] [CLASSIFIER=FALSE | TRUE]].
```

Every keyword may also be written KEY(value). Test variables are numeric; the state variable may be a string, its value in parentheses marks the positive cases and every other valid state is negative. A case takes part when its state, all test variables and the grouping variable are valid (USERMISSING=INCLUDE keeps user-missing values).

Weights are rounded to whole numbers and count cases; a weight of 0.5 or less, or a missing one, makes the case missing. Cutoffs, directions and areas as in ROC; when every test value lies strictly between 0 and 1 the outer cutoffs are 0 and 1.

Without DESIGN each test variable gets its own area. PAIR=TRUE compares every pair of two or more test variables measured on the same cases: the errors of the areas and of their differences follow DeLong's method, and NEGEXPO does not apply. GROUP with SELECT compares two independent groups of cases for each test variable (PAIR is then ignored): LEVEL names the value of each group; MIDPOINT (halfway between the smallest and largest group value) and CUTPOINT split a numeric grouping variable into the cases at or above the point and those below it. NEGEXPO needs equally many positive and negative cases in both groups, else both groups use the nonparametric error.

Tables: Case Processing Summary; Area Under the ROC Curve (SE=TRUE adds the standard error, the test of area 0.5 and the CI interval); CLASSIFIER adds the Gini index 2 x area - 1 and the largest gap between sensitivity and 1 - specificity (Max K-S) with its cutoff; the paired or independent design adds a table of area differences with z tests and intervals; COORDINATES lists sensitivity and 1 - specificity (ROC) or precision and recall (PR) at every cutoff. CURVE draws ROC curves (the default), PR curves or none; with groups each test variable gets its own chart. PR curves are filled in between cutoffs one true positive (TP) or one false positive (FP) at a time; PR(REFERENCE) draws the share of positive cases. MODELQUALITY=TRUE adds a bar chart of the lower bounds of the area intervals.

## Example

```text
ROC ANALYSIS score_a score_b BY diseased (1) /DESIGN PAIR=TRUE /PRINT SE=TRUE CLASSIFIER=TRUE /PLOT CURVE=ROC(REFERENCE) PR.
```

`statscraft help ROC ANALYSIS` prints the same text in the terminal. [All commands](README.md)
