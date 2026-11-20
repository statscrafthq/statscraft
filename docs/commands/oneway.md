<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# ONEWAY

One-way analysis of variance with contrasts and post hoc comparisons.

## Syntax

```text
ONEWAY deps BY factor [/STATISTICS=DESCRIPTIVES EFFECTS HOMOGENEITY WELCH BROWNFORSYTHE ALL NONE]
       [/CONTRAST=coefficients] [/POLYNOMIAL=degree] [/POSTHOC=test ... [ALPHA(0.05)]]
       [/RANGES=LSD | DUNCAN | SNK | TUKEYB | TUKEY | MODLSD | SCHEFFE [(alpha)]] ...
       [/MISSING=ANALYSIS | LISTWISE [INCLUDE | EXCLUDE]] [/CRITERIA=CILEVEL(0.95)] [/PLOT MEANS].
```

test: LSD BONFERRONI SIDAK TUKEY SCHEFFE GT2 GABRIEL GH T2 T3 C (pairwise comparisons), DUNNETT DUNNETTL DUNNETTR \[(refcat)\] (each group against the reference group, by default the last; L: smaller, R: larger), SNK BTUKEY DUNCAN FREGW QREGW (homogeneous subsets only); TUKEY SCHEFFE GT2 GABRIEL give both. DUNCAN and the Dunnett keywords are written in full. Every value of the factor is a group. CONTRAST gives one coefficient per group in ascending order (n\*c repeats c n times); it is tested with pooled and separate variances. HOMOGENEITY is Levene's test based on the mean. Multiple Comparisons show every ordered pair of groups (Dunnett C has no significance).

Homogeneous subsets use the harmonic mean of the group sizes (the R-E-G-W tests the sizes themselves); Tukey B shows no significance. Weighted when a weight variable is set. PLOT MEANS gives each dependent a line chart through the mean of every group, as chart data.

POLYNOMIAL (1 to 5, the last one given counts; lowered with a warning to the groups minus 1) splits the between-groups sum of squares into trend terms in the ANOVA table, with the group value as the metric: a Weighted row per degree (sequential, the groups weighted by size) and a Deviation row of what is left (none after the last possible degree); when the group sizes differ and the groups are equally spaced there is also an Unweighted row per degree.

RANGES names one of these post hoc tests (MODLSD, also LSDMOD, is Bonferroni), at most 10 times; its tests add to those of POSTHOC and the last alpha given anywhere counts for all of them.

EFFECTS adds the rows Fixed Effects (pooled standard deviation, and the error and interval of the overall mean from the within mean square) and Random Effects (error and interval from the between-groups component, which has its own column) to the Descriptives table; without DESCRIPTIVES it holds only those two rows. A negative component counts as 0 in the error, with a warning.

Not supported yet: WALLER.

## Example

```text
ONEWAY income BY region /STATISTICS=DESCRIPTIVES HOMOGENEITY WELCH /POSTHOC=TUKEY GH.
```

`statscraft help ONEWAY` prints the same text in the terminal. [All commands](README.md)
