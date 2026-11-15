<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# POWER ONEWAY ANOVA

Power or group sizes of a one-way analysis of variance, its contrasts and pairwise comparisons.

## Syntax

```text
POWER ONEWAY ANOVA /PARAMETERS [SIGNIFICANCE=0.05]  GROUP_SIZES=n1 n2 ... | POWER=powers
                   POOLED_SD=sd  GROUP_MEANS=m1 m2 ...  [GROUP_WEIGHTS=EQUAL | w1 w2 ...]
                   [/CONTRAST [COEFFICIENTS=c1 c2 ...] [TEST=NONDIRECTIONAL | DIRECTIONAL]
                              [PAIRWISE=FALSE | TRUE] [ADJUST=BONFERRONI | SIDAK | LSD]]
                   [/PLOT [TOTAL_N[(low high)]] [POOLED_SD[(low high)]]
                          [TOTAL_N_XAXIS[(low high)] POOLED_SD_YAXIS[(low high)]]
                          [TOTAL_N_YAXIS[(low high)] POOLED_SD_XAXIS[(low high)]] [VROTATE(10)] [HROTATE(325)]].
```

Needs no data. Each group has a size (a whole number of at least 2) and a mean; all share POOLED\_SD. The overall F test of equal means gets its power from the noncentral F with J - 1 and N - J df, its effect size being the root mean square standardized effect. COEFFICIENTS add a test of one contrast (noncentral t, one- or two-sided by TEST); coefficients that do not add up to 0 are corrected in the last group (the one before it when only the last is not 0), with a warning. PAIRWISE=TRUE adds a two-sided test of every pair of groups at the significance level split by Bonferroni, Sidak or not at all (LSD). With POWER each test gets its own sizes: the total is searched with the groups in the proportions of GROUP\_WEIGHTS (each 1 to 100, equal by default), then every group is rounded up (at least 2); a Group Sizes table lists them.

PLOT charts the overall test: TOTAL\_N over the total size with the groups kept in proportion (default from the smallest total that keeps every group at 2 or more to twice the table's total), POOLED\_SD over the standard deviation (default half to twice the value given), and the pair of \_XAXIS and \_YAXIS keywords a surface over both.

## Example

```text
POWER ONEWAY ANOVA /PARAMETERS GROUP_MEANS=48 52 57 POOLED_SD=9 POWER=0.85
  GROUP_WEIGHTS=2 1 1 /CONTRAST COEFFICIENTS=-2 1 1 PAIRWISE=TRUE ADJUST=SIDAK.
```

`statscraft help POWER ONEWAY ANOVA` prints the same text in the terminal. [All commands](README.md)
