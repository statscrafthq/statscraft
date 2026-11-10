<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# EXAMINE

Descriptives, percentiles, M-estimators, extreme values and normality tests, by factor cells.

## Syntax

```text
EXAMINE [VARIABLES=]deps [BY factor [BY factor] [factor ...]] [/TOTAL | NOTOTAL] [/ID=var]
        [/PERCENTILES[(list)][=HAVERAGE | WAVERAGE | ROUND | EMPIRICAL | AEMPIRICAL | NONE]]
        [/STATISTICS=DESCRIPTIVES EXTREME[(n)] ALL NONE] [/CINTERVAL=95]
        [/MESTIMATORS[=HUBER[(c)] TUKEY[(c)] HAMPEL[(a,b,c)] ANDREW[(c)] ALL NONE]]
        [/PLOT=NPPLOT | NONE] [/COMPARE=GROUPS | VARIABLES]
        [/MISSING=LISTWISE | PAIRWISE  EXCLUDE | INCLUDE  NOREPORT | REPORT].
```

Each dependent is analysed over all cases (unless NOTOTAL) and in every cell of each factor term, two factors crossed with BY. Descriptives (the default): mean with its confidence interval (CINTERVAL, 50 to 99.99), 5% trimmed mean, median, variance, standard deviation, minimum, maximum, range, interquartile range, skewness and kurtosis with standard errors. PERCENTILES alone gives 5 10 25 50 75 90 95 by HAVERAGE, with Tukey's hinges when 25, 50 or 75 is listed; the method also gives the median and the interquartile range. MESTIMATORS alone gives all four (Huber 1.339, Tukey 4.685, Hampel 1.7 3.4 8.5, Andrews 1.34 times pi), each started at the median and scaled by the median absolute deviation. EXTREME lists the n highest and lowest values by case number or by the ID variable.

NPPLOT gives the Tests of Normality table (Kolmogorov-Smirnov with the Lilliefors significance and Shapiro-Wilk by Royston's algorithm, 3 to 5000 cases); the plots themselves are not drawn.

LISTWISE (default) leaves out a case missing on any dependent or factor, PAIRWISE only from the analyses of that variable. INCLUDE makes user-missing values of the dependents valid, REPORT makes missing values of the factors categories. Weighted when a weight variable is set (Shapiro-Wilk rounds the weights and takes 3 to 50 cases with non-integer weights). COMPARE changes nothing.

Not supported yet: STEMLEAF, BOXPLOT, HISTOGRAM, SPREADLEVEL.

## Example

```text
EXAMINE VARIABLES=income BY region /PERCENTILES(25,50,75)=HAVERAGE /STATISTICS=ALL /MESTIMATORS /PLOT=NPPLOT.
```

`statscraft help EXAMINE` prints the same text in the terminal. [All commands](README.md)
