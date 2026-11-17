<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# AIM

Attribute importance: how much each variable sets every group (cluster) apart from all cases.

## Syntax

```text
AIM groupvar [/CATEGORICAL varlist] [/CONTINUOUS varlist]
    [/CRITERIA [ADJUST=BONFERRONI | NONE] [CI=95] [HIDENOTSIG=NO | YES] [SHOWREFLINE=YES | NO]]
    [/MISSING=EXCLUDE | INCLUDE]
    [/PLOT [CATEGORY] [CLUSTER[(TYPE=BAR | PIE)]] [ERRORBAR] [IMPORTANCE[([X=GROUP | VARIABLE] [Y=TEST | PVALUE])]]].
```

The group variable (numeric or string) comes first; CATEGORICAL takes variables of any type, CONTINUOUS numeric ones, and at least one of them is needed. For every group and variable a test compares the group with all cases: a categorical variable by the chi-square of the group's category counts against the shares of all cases (categories less one degrees of freedom), a continuous one by Student's t of the group mean against the overall mean, over the group's own standard deviation (group size less one degrees of freedom, two-sided). A group whose values are all equal gets an infinite t and significance 0. The significance level is 100 less CI percent; BONFERRONI (default) divides it by the number of groups, for the tests and for the error bar intervals. Tables: one row per variable and group with the statistic, degrees of freedom, significance and whether it passes the level. PLOT CATEGORY shows each categorical variable's category shares within the groups, CLUSTER the size of the groups as bars or a pie, ERRORBAR the group means with their intervals, IMPORTANCE the statistics (Y=TEST) or -log10 of the significance (Y=PVALUE), one chart per variable with the groups along the axis (X=GROUP) or one per group with the variables (X=VARIABLE).

HIDENOTSIG=YES leaves out of the charts what does not pass the level; SHOWREFLINE adds the critical values, or the overall mean to the error bars. A case missing on any of the named variables is left out; INCLUDE keeps user-missing values. The weight variable is not used.

Subcommands may be given in any order; a repeated one replaces the earlier.

## Example

```text
AIM segment /CATEGORICAL region plan /CONTINUOUS spend visits
  /CRITERIA CI=90 /PLOT CLUSTER(TYPE=PIE) ERRORBAR IMPORTANCE(X=VARIABLE Y=PVALUE).
```

`statscraft help AIM` prints the same text in the terminal. [All commands](README.md)
