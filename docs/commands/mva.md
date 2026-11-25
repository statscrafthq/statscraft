<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# MVA

Missing value analysis: where values are missing, how that relates to the other variables, and estimates that allow for it.

## Syntax

```text
MVA [VARIABLES=] varlist | ALL
   [/CATEGORICAL=varlist] [/MAXCAT=25] [/ID=var]
   [/NOUNIVARIATE] [/TTEST [PERCENT=5] [T | NOT] [DF | NODF] [PROB | NOPROB] [COUNTS | NOCOUNTS]
                          [MEANS | NOMEANS]]
   [/CROSSTAB [PERCENT=5]] [/MISMATCH [PERCENT=5] [NOSORT]]
   [/DPATTERN [SORT=var[(ASCENDING | DESCENDING)] ...] [DESCRIBE=varlist]]
   [/MPATTERN [NOSORT] [DESCRIBE=varlist]] [/TPATTERN [NOSORT] [DESCRIBE=varlist] [PERCENT=1]]
   [/LISTWISE] [/PAIRWISE]
   [/EM [varlist] [WITH varlist] [(TOLERANCE=0.001 CONVERGENCE=0.0001 ITERATIONS=25
        TDF=n | LAMBDA=a PROPORTION=b, OUTFILE='file' | dataset)]]
   [/REGRESSION [varlist] [WITH varlist] [(TOLERANCE=0.001 FLIMIT=4 NPREDICTORS=n
        ADDTYPE=RESIDUAL | NORMAL | T[(5)] | NONE, OUTFILE='file' | dataset)]].
```

String variables and those on CATEGORICAL are categorical (a long string by its first 8 bytes); MVA stops with a warning when one has more than MAXCAT values. All others are quantitative. System-missing and user-missing values count as missing; user-missing ones are coded A, B, C in the order of their definition (a range is A, its extra value B). CATEGORICAL, MAXCAT and ID come before the other subcommands; VARIABLES, SORT, NOSORT, DESCRIBE and WITH cannot name variables here. Weights are not used.

Univariate Statistics (unless NOUNIVARIATE): valid N, missing count and percent, and for quantitative variables the mean, standard deviation and the count of extremes: values beyond 1.5 interquartile ranges outside the quartiles (at (n+1)p), or, when variables times n times log10(n) passes 150000, beyond 2 standard deviations from the mean.

TTEST: for each variable with at least PERCENT% missing, every quantitative variable is compared between the cases where it is present and missing (Welch t, its df, two-sided p with PROB, the counts and the means). CROSSTAB: per categorical variable and category, the count and percent present and the percent of each missing kind of every other variable with at least PERCENT% missing. MISMATCH: the percent of cases where one variable of a pair is missing and the other not, the percent missing on the diagonal.

DPATTERN lists every case (in file order or by SORT), MPATTERN the cases with a missing value, each with S, A, B, C for missing and + or - for extremes, the number and percent missing and the DESCRIBE values. TPATTERN counts each pattern of missing values (X), leaving out those under PERCENT% of the cases; DESCRIBE adds category counts or means. ID labels the cases. In MPATTERN, TPATTERN and MISMATCH, unless NOSORT, variables go from fewest to most missing and alike patterns stand together; DPATTERN keeps the list order.

LISTWISE: means, covariances and correlations over the cases complete on every variable.

PAIRWISE: counts, means and standard deviations of the column variable, covariances and correlations over the cases valid on both variables.

EM estimates the means, covariances and correlations of the quantitative variables (or the ones listed, from those after WITH) under a normal, a t (TDF) or a mixed normal (LAMBDA, the ratio of the standard deviations, PROPORTION, the share of the second part) distribution, starting from the univariate means and pairwise covariances; covariances divide by n - 1. It stops when no variance changes by more than CONVERGENCE relative to its value, or after ITERATIONS, and gives Little's MCAR test. REGRESSION fills each missing value by forward stepwise regression on the predictors the case has (F to enter FLIMIT, at most NPREDICTORS), from the univariate means and pairwise covariances, plus the residual of a random complete case, a normal or t error scaled by the residual standard deviation, or nothing; NORMAL is the default when fewer than half the cases are complete. Random terms follow SET SEED. When two or more ways to estimate are shown (univariate, LISTWISE, PAIRWISE, EM, REGRESSION), summary tables compare their means and standard deviations (pairwise ones are those of all values). OUTFILE saves the active data with the missing values of the estimated variables filled in and their user-missing definitions dropped. Under SPLIT FILE each group is filled from its own estimates, all in one file; a case outside the groups that ran keeps its values, a user-missing one as system-missing.

## Example

```text
MVA VARIABLES=district mode age km minutes /CATEGORICAL=district /ID=resp /TTEST PROB /TPATTERN /EM (OUTFILE='filled.sav').
```

`statscraft help MVA` prints the same text in the terminal. [All commands](README.md)
