<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# ONEWAY

One-way analysis of variance with contrasts and post hoc comparisons.

## Syntax

```text
ONEWAY deps BY factor [/STATISTICS=DESCRIPTIVES EFFECTS HOMOGENEITY WELCH BROWNFORSYTHE ALL NONE]
       [/CONTRAST=coefficients] [/POLYNOMIAL=degree] [/POSTHOC=test ... [ALPHA(0.05)]]
       [/RANGES=LSD | DUNCAN | SNK | TUKEYB | TUKEY | MODLSD | SCHEFFE [(alpha)]] ...
       [/MISSING=ANALYSIS | LISTWISE [INCLUDE | EXCLUDE]] [/CRITERIA=CILEVEL(0.95)] [/PLOT MEANS]
       [/TEMPLATE='file']
       [/MATRIX=IN(file | dataset | *) | OUT(file | dataset | *) | NONE].
```

test: LSD BONFERRONI SIDAK TUKEY SCHEFFE GT2 GABRIEL GH T2 T3 C (pairwise comparisons), DUNNETT DUNNETTL DUNNETTR \[(refcat)\] (each group against the reference group, by default the last; L: smaller, R: larger), SNK BTUKEY DUNCAN FREGW QREGW WALLER\[(kratio)\] (homogeneous subsets only); TUKEY SCHEFFE GT2 GABRIEL give both. DUNCAN and the Dunnett keywords are written in full. Every value of the factor is a group. CONTRAST gives one coefficient per group in ascending order (n\*c repeats c n times); it is tested with pooled and separate variances. HOMOGENEITY gives Levene's test four ways, one row each under Based on Mean, Median, Median and with adjusted df, and trimmed mean. Multiple Comparisons show every ordered pair of groups (Dunnett C has no significance).

Homogeneous subsets use the harmonic mean of the group sizes (the R-E-G-W tests the sizes themselves); Tukey B and WALLER show no significance. WALLER, the Waller-Duncan t test, takes the ratio of the seriousness of a Type 1 to a Type 2 error as a whole number above 1 (100 by default); it calls two means different when they are t\_B s sqrt(2/n) apart, t\_B the Bayesian t value from the k-ratio, the F ratio of the analysis of variance and its df, n the harmonic mean of the group sizes. Weighted when a weight variable is set. PLOT MEANS gives each dependent a line chart through the mean of every group, as chart data.

POLYNOMIAL (1 to 5, the last one given counts; lowered with a warning to the groups minus 1) splits the between-groups sum of squares into trend terms in the ANOVA table, with the group value as the metric: a Weighted row per degree (sequential, the groups weighted by size) and a Deviation row of what is left (none after the last possible degree); when the group sizes differ and the groups are equally spaced there is also an Unweighted row per degree.

RANGES names one of these post hoc tests (MODLSD, also LSDMOD, is Bonferroni), at most 10 times; its tests add to those of POSTHOC and the last alpha given anywhere counts for all of them.

EFFECTS adds the rows Fixed Effects (pooled standard deviation, and the error and interval of the overall mean from the within mean square) and Random Effects (error and interval from the between-groups component, which has its own column) to the Descriptives table; without DESCRIPTIVES it holds only those two rows. A negative component counts as 0 in the error, with a warning.

MATRIX=OUT writes, per split group and value of the factor, the records MEAN, STDDEV and N of every dependent; IN reads such records (and the pooled MSE and DFE records, which then give the within variance) and runs everything that needs no cases: HOMOGENEITY is an error, and the minimum and maximum of the descriptives are empty. TEMPLATE is ignored; charts come as data.

## Example

```text
GET FILE='survey.sav'.
ONEWAY satisfaction BY region.
ONEWAY spend BY age_group /STATISTICS=DESCRIPTIVES HOMOGENEITY.
ONEWAY satisfaction BY region /POSTHOC=TUKEY ALPHA(0.05).
```

The example reads the sample data file [survey.sav](../sample/survey.sav), an invented customer survey; save it in the folder you run the syntax from.

## Example output

### ANOVA

|  |  | Sum of Squares | df | Mean Square | F | Sig. |
| --- | --- | --- | --- | --- | --- | --- |
| Overall satisfaction with the service | Between Groups | 47.20 | 3 | 15.73 | 13.13 | 0.00 |
|  | Within Groups | 327.12 | 273 | 1.20 |  |  |
|  | Total | 374.32 | 276 |  |  |  |

`statscraft help ONEWAY` prints the same text in the terminal. [All commands](README.md)
