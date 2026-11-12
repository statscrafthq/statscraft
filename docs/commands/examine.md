<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# EXAMINE

Descriptives, percentiles, M-estimators, extreme values and normality tests, by factor cells.

## Syntax

```text
EXAMINE [VARIABLES=]deps [BY factor [BY factor] [factor ...]] [/TOTAL | NOTOTAL] [/ID=var]
        [/PERCENTILES[(list)][=HAVERAGE | WAVERAGE | ROUND | EMPIRICAL | AEMPIRICAL | NONE]]
        [/STATISTICS=DESCRIPTIVES EXTREME[(n)] ALL NONE] [/CINTERVAL=95]
        [/MESTIMATORS[=HUBER[(c)] TUKEY[(c)] HAMPEL[(a,b,c)] ANDREW[(c)] ALL NONE]]
        [/PLOT=[BOXPLOT] [STEMLEAF] [HISTOGRAM] [NPPLOT] [SPREADLEVEL[(power)]] | ALL | NONE]
        [/COMPARE=GROUPS | VARIABLES]
        [/MISSING=LISTWISE | PAIRWISE  EXCLUDE | INCLUDE  NOREPORT | REPORT].
```

Each dependent is analyzed over all cases (unless NOTOTAL) and in every cell of each factor term, two factors crossed with BY. Descriptives (the default): mean with its confidence interval (CINTERVAL, 50 to 99.99), 5% trimmed mean, median, variance, standard deviation, minimum, maximum, range, interquartile range, skewness and kurtosis with standard errors. PERCENTILES alone gives 5 10 25 50 75 90 95 by HAVERAGE, with Tukey's hinges when 25, 50 or 75 is listed; the method also gives the median and the interquartile range. MESTIMATORS alone gives all four (Huber 1.339, Tukey 4.685, Hampel 1.7 3.4 8.5, Andrews 1.34 times pi), each started at the median and scaled by the median absolute deviation. EXTREME lists the n highest and lowest values by case number or by the ID variable.

NPPLOT gives the Tests of Normality table (Kolmogorov-Smirnov with the Lilliefors significance and Shapiro-Wilk by Royston's algorithm, 3 to 5000 cases).

Plots come as chart data (a data table, and Vega-Lite with --vega-lite). Without PLOT every cell gets a stem-and-leaf plot and a boxplot; a PLOT subcommand names the ones wanted. BOXPLOT: the box spans Tukey's hinges around the median; a case 1.5 to 3 box lengths past a hinge is an outlier, 3 or more an extreme, each labeled by case number or by the ID variable, and the whiskers stop at the last unmarked values. COMPARE=GROUPS (default) gives each dependent one plot with a box per cell; VARIABLES puts all dependents in one plot, grouped by cell. STEMLEAF lists the cases past the outlier bounds as extremes. NPPLOT adds the normal Q-Q plot (each distinct value against the normal score of its mean rank over W + 1) and the detrended one. SPREADLEVEL needs a factor: per cell the log of the interquartile range against the log of the median, with the slope and the power it suggests (1 minus the slope); with a power p the values are raised to p first (0 takes logs), and the Test of Homogeneity of Variance table gives Levene's tests on the same values. ALL is every plot.

LISTWISE (default) leaves out a case missing on any dependent or factor, PAIRWISE only from the analyses of that variable. INCLUDE makes user-missing values of the dependents valid, REPORT makes missing values of the factors categories. Weighted when a weight variable is set (Shapiro-Wilk rounds the weights and takes 3 to 50 cases with non-integer weights).

## Example

```text
EXAMINE VARIABLES=income BY region /PERCENTILES(25,50,75)=HAVERAGE /STATISTICS=ALL /MESTIMATORS /PLOT=NPPLOT.
```

`statscraft help EXAMINE` prints the same text in the terminal. [All commands](README.md)
