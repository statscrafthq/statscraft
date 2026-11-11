<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# NPAR TESTS

Nonparametric tests: one-sample, two and k related samples, two and k independent samples.

## Syntax

```text
NPAR TESTS [/CHISQUARE=varlist [(lo,hi)] [/EXPECTED=EQUAL | f1, n*f2, ...]]
           [/BINOMIAL[(p)]=varlist[(cut) | (value1,value2)]] [/RUNS(MEAN | MEDIAN | MODE | value)=varlist]
           [/K-S(NORMAL [mean,sd] | UNIFORM [min,max] | POISSON [mean] | EXPONENTIAL [mean])=varlist]
           [/WILCOXON | /SIGN | /MCNEMAR | /MH=varlist [WITH varlist [(PAIRED)]]]
           [/FRIEDMAN | /KENDALL | /COCHRAN=varlist]
           [/M-W | /K-S | /W-W | /MOSES[(n)] | /K-W | /J-T | /MEDIAN[(value)]=varlist BY var(value1,value2)]
           [/STATISTICS=[DESCRIPTIVES] [QUARTILES] [ALL]] [/MISSING=ANALYSIS | LISTWISE [INCLUDE]]
           [/METHOD=EXACT [TIMER(n)] | MC [CIN(n)] [SAMPLES(n)]] [/SAMPLE] [/KS_SIM ...].
```

Long names work too: CHI-SQUARE, KOLMOGOROV-SMIRNOV, MANN-WHITNEY, WALD-WOLFOWITZ, KRUSKAL-WALLIS, JONCKHEERE-TERPSTRA. Without WITH every variable is paired with every later one; (PAIRED) pairs the n-th with the n-th. M-W, two-sample K-S, W-W and MOSES compare the two groups (MOSES: the first is the control group); K-W and J-T take every value from value1 to value2 as a group; MEDIAN does so when value1 \< value2, and compares the two values otherwise. Exact significance where the method gives it for small samples: binomial; sign and McNemar up to 25 changes; Mann-Whitney when n1 n2 \<= 400 and n1 n2 / 2 + min(n1, n2) \<= 220; median test up to 30 cases (Fisher); W-W up to 30 cases; Moses.

K-S with normal parameters taken from the data uses the Lilliefors significance. METHOD adds exact significances for Wilcoxon, sign, McNemar, Mann-Whitney and the runs tests (MC gives them too, as exact results replace simulated ones); other tests stay asymptotic, with a warning. ANALYSIS (default) leaves out cases missing on a test's variables, LISTWISE cases missing on any variable of the command.

A weight repeats each case as often as its weight rounded to a whole number. SAMPLE and KS\_SIM change nothing.

## Example

```text
NPAR TESTS /M-W=income BY gender(1,2) /WILCOXON=before WITH after /STATISTICS=QUARTILES.
```

`statscraft help NPAR TESTS` prints the same text in the terminal. [All commands](README.md)
