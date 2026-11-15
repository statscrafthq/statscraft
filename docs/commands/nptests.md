<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# NPTESTS

Nonparametric tests picked by measurement level: one sample, independent samples, related samples.

## Syntax

```text
NPTESTS [/ONESAMPLE TEST(fields) [CHISQUARE[(EXPECTED=EQUAL | CUSTOM(FREQUENCIES=list CATEGORIES=list))]]
          [BINOMIAL[(TESTVALUE=p SUCCESSCATEGORICAL=FIRST | LIST(values)
                    SUCCESSCONTINUOUS=CUTPOINT(MIDPOINT | value) CLOPPERPEARSON JEFFREYS LIKELIHOOD)]]
          [KOLMOGOROV_SMIRNOV[(NSAMPLES=n MC_CILEVEL=level NORMAL=SAMPLE[(SIMULATION=TRUE | FALSE)] |
                    CUSTOM(MEAN=m SD=s) UNIFORM=SAMPLE | CUSTOM(MIN=a MAX=b)
                    EXPONENTIAL=SAMPLE | CUSTOM(MEAN=m) POISSON=CUSTOM(MEAN=m))]]
          [RUNS[(GROUPCATEGORICAL=SAMPLE | LIST(values)
                    GROUPCONTINUOUS=CUTPOINT(SAMPLEMEDIAN | SAMPLEMEAN | value))]]
          [WILCOXON(TESTVALUE=value)]]
        [/INDEPENDENT TEST(fields) GROUP(field) [MANN_WHITNEY] [WALD_WOLFOWITZ] [KOLMOGOROV_SMIRNOV]
          [HODGES_LEHMAN] [MOSES[(TRIMOUTLIERS=SAMPLE | n)]] [KRUSKAL_WALLIS[(COMPARE=c)]]
          [MEDIAN[(TESTVALUE=SAMPLE | value COMPARE=c)]] [JONCKHEERE_TERPSTRA[(ORDER=ASCENDING | DESCENDING
          COMPARE=c)]]]
        [/RELATED TEST(fields) [MCNEMAR[(SUCCESS=FIRST | LIST(values))]] [WILCOXON] [SIGN]
          [MARGINAL_HOMOGENEITY] [HODGES_LEHMAN] [COCHRAN[(SUCCESS=FIRST | LIST(values) COMPARE=c)]]
          [FRIEDMAN[(COMPARE=c)]] [KENDALL[(COMPARE=c)]]]
        [/MISSING [SCOPE=ANALYSIS | LISTWISE] [USERMISSING=EXCLUDE | INCLUDE]]
        [/CRITERIA [ALPHA=0.05] [CILEVEL=95] [SEED=2000000 | RANDOM]].
```

c is PAIRWISE (the default), STEPWISE or NONE. Keywords are written out in full and each appears once.

Nominal and ordinal fields count as categorical, scale fields as continuous; a field of unknown level gets one from its values. A test subcommand with only TEST (and GROUP) picks the tests itself: one sample, a binomial test for a categorical field with two values, chi-square for one with more, and a normal Kolmogorov-Smirnov test for a continuous field; independent samples, Mann-Whitney for two groups and Kruskal-Wallis for more; related samples, the fields split by level, then Wilcoxon (two) or Friedman (more) on continuous fields and McNemar (two) or Cochran (more) on categorical fields with two values between them. Named tests run only on fields that suit them, with a warning for the others: chi-square on categorical fields, K-S and Wilcoxon on continuous and ordinal ones; independent tests on continuous and ordinal fields, the two-group tests only when GROUP has two values (the lower value is the first group, the Moses control group); McNemar and Cochran on the nominal fields and on the ordinal fields separately; Wilcoxon, SIGN, HODGES\_LEHMAN, FRIEDMAN and KENDALL on the continuous and ordinal fields together; MARGINAL\_HOMOGENEITY on two ordinal fields.

FIRST makes the first value met in the data the success and needs exactly two values. A continuous field is cut at a point: values up to it are the success (binomial) or the first group (runs).

Binomial, sign and McNemar are exact up to 25 cases, Mann-Whitney for small groups, Wald-Wolfowitz up to 30 cases, the median test of two groups up to 30 cases (Fisher), Moses always. K-S with parameters from the data draws NSAMPLES samples (default 1000) from SEED for a Monte Carlo significance with a MC\_CILEVEL interval; NORMAL=SAMPLE(SIMULATION=FALSE) uses the Lilliefors table.

Jonckheere-Terpstra decides one-sided in the ORDER direction. After a rejected test of three or more samples, COMPARE gives pairwise comparisons (Bonferroni-adjusted) or homogeneous subsets found by stepwise step-down tests. Output: the hypothesis test summary (ALPHA), a confidence interval summary (binomial intervals and Hodges-Lehmann estimates at CILEVEL), a detail table per test and the comparisons. USERMISSING applies to categorical fields; SCOPE=LISTWISE drops a case missing on any field named. Weights are rounded to whole numbers; the weight variable cannot be tested.

## Example

```text
NPTESTS /ONESAMPLE TEST(score rating) /INDEPENDENT TEST(score) GROUP(site) KRUSKAL_WALLIS(COMPARE=STEPWISE) /CRITERIA ALPHA=0.01.
```

`statscraft help NPTESTS` prints the same text in the terminal. [All commands](README.md)
