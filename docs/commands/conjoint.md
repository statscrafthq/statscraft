<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# CONJOINT

Part-worth utilities and importances from rankings or scores of the profiles of a plan.

## Syntax

```text
CONJOINT [PLAN=* | 'file' | dataset] [/DATA=* | 'file' | dataset] /SEQUENCE | RANK | SCORE=varlist
   [/SUBJECT=variable]
   [/FACTORS=varlist ['label'] ([DISCRETE [MORE | LESS] | LINEAR [MORE | LESS] | IDEAL | ANTIIDEAL]
             [value ['label'] ...]) ...]
   [/PRINT=ALL | ANALYSIS | SIMULATION | NONE [SUMMARYONLY]] [/UTILITY='file' | dataset]
   [/PLOT=[SUMMARY] [SUBJECT] [ALL] | NONE].
```

The plan (STATUS\_ 0 experimental, 1 holdout, 2 simulation, the simulations last; every variable but STATUS\_ and CARD\_ a numeric factor) and the responses come from PLAN and DATA, one of them a file or dataset and the other by default the active dataset. The response variables follow the experimental and holdout profiles in plan order: SCORE holds scores (higher is better), RANK the rank of each profile (1 is best), SEQUENCE the profile numbers from the best down. Consecutive cases with one SUBJECT value are one subject; without SUBJECT all cases are one. A subject with a missing response is skipped. Least squares on the experimental profiles estimates per subject and for the group the utilities: DISCRETE (the default) one per level summing to 0, LINEAR the value times a coefficient, IDEAL and ANTIIDEAL a quadratic of the value; the levels of the FACTORS list replace the factor's values from the smallest up, and labels its labels. MORE and LESS (and the quadratic models) only count subjects whose estimates go the other way. Tables: Model Description, recoded values, Cramer's V when the plan is not orthogonal, Utilities with standard errors and the constant, Importance Values (range of a factor's utilities as a percent of all ranges, averaged over subjects), Coefficients, Correlations (Pearson's R and Kendall's tau of the predicted and observed preferences, Kendall's tau for holdouts, one-tailed), reversals, and for simulation profiles the predicted scores and the mean probabilities of maximum utility, Bradley-Terry-Luce and logit choice. PRINT picks analysis or simulation results, SUMMARYONLY drops the subjects' tables. UTILITY writes a case per subject: split and subject variables, CONSTANT, the utilities (name1, name2, ... or name\_L and name\_Q) and the predicted SCOREn and SIMULn. PLOT draws bar charts of the importances and utilities, SUMMARY for the group, SUBJECT per subject.

## Example

```text
CONJOINT PLAN='plan.sav' /FACTORS=price (LINEAR LESS) brand /SUBJECT=id /RANK=r1 TO r11 /PLOT=SUMMARY.
```

`statscraft help CONJOINT` prints the same text in the terminal. [All commands](README.md)
