<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# ONEWAY

One-way analysis of variance with contrasts and post hoc comparisons.

## Syntax

```text
ONEWAY deps BY factor [/STATISTICS=DESCRIPTIVES HOMOGENEITY WELCH BROWNFORSYTHE ALL NONE]
       [/CONTRAST=coefficients] [/POSTHOC=test ... [ALPHA(0.05)]]
       [/MISSING=ANALYSIS | LISTWISE [INCLUDE | EXCLUDE]] [/CRITERIA=CILEVEL(0.95)] [/PLOT MEANS].
```

test: LSD BONFERRONI SIDAK TUKEY SCHEFFE GT2 GABRIEL GH T2 T3 C (pairwise comparisons), DUNNETT DUNNETTL DUNNETTR \[(refcat)\] (each group against the reference group, by default the last; L: smaller, R: larger), SNK BTUKEY DUNCAN FREGW QREGW (homogeneous subsets only); TUKEY SCHEFFE GT2 GABRIEL give both. DUNCAN and the Dunnett keywords are written in full. Every value of the factor is a group. CONTRAST gives one coefficient per group in ascending order (n\*c repeats c n times); it is tested with pooled and separate variances. HOMOGENEITY is Levene's test based on the mean. Multiple Comparisons show every ordered pair of groups (Dunnett C has no significance).

Homogeneous subsets use the harmonic mean of the group sizes (the R-E-G-W tests the sizes themselves); Tukey B shows no significance. Weighted when a weight variable is set. PLOT MEANS gives each dependent a line chart through the mean of every group, as chart data.

Not supported yet: POLYNOMIAL, RANGES, EFFECTS, WALLER.

## Example

```text
ONEWAY income BY region /STATISTICS=DESCRIPTIVES HOMOGENEITY WELCH /POSTHOC=TUKEY GH.
```

`statscraft help ONEWAY` prints the same text in the terminal. [All commands](README.md)
