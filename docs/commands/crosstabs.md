<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# CROSSTABS

Contingency tables with percentages, residuals and chi-square tests.

## Syntax

```text
CROSSTABS [/]TABLES=rowvars BY colvars [BY layervars ...] [/TABLES=...]
          [/VARIABLES=varlist (min, max) [varlist (min, max) ...]  (before TABLES)]
          [/CELLS=COUNT ROW COLUMN TOTAL EXPECTED RESID SRESID ASRESID ALL NONE]
          [/STATISTICS=CHISQ PHI CC LAMBDA UC BTAU CTAU GAMMA D ETA CORR KAPPA RISK MCNEMAR CMH[(1)] ALL NONE] [/MISSING=TABLE | INCLUDE | REPORT]
          [/FORMAT=AVALUE | DVALUE TABLES | NOTABLES]
          [/COUNT=[ASIS | CASE | CELL] [ROUND | TRUNCATE]] [/BARCHART]
          [/METHOD=MC [CIN(99)] [SAMPLES(10000)] | EXACT [TIMER(5)]].
```

First a Case Processing Summary: per table the valid, missing and total cases (weighted).

One table per pair of row and column variable; layer variables split it into blocks.

NOTABLES leaves out the tables, as CELLS=NONE does, keeping the statistics.

COUNT makes weighted counts whole: CASE every case's weight, CELL every cell's count, before anything is computed; ROUND or TRUNCATE. A keyword left out is CELL or ROUND; without COUNT, ASIS, weights stay as they are.

CELLS defaults to COUNT; CELLS alone is COUNT ROW COLUMN TOTAL. Percentages are 0 to 100.

STATISTICS alone is CHISQ. DVALUE orders the rows from the highest value; columns stay ascending.

CHISQ: Pearson, likelihood ratio, linear-by-linear (numeric variables), and for a 2 x 2 table Fisher's exact test and the continuity correction.

PHI (phi and Cramer's V), CC (contingency coefficient), BTAU, CTAU (Kendall's tau-b, tau-c), GAMMA and CORR (Spearman and Pearson, numeric variables only) go in a Symmetric Measures table; LAMBDA (lambda and Goodman and Kruskal's tau), UC (uncertainty coefficient), D (Somers' d) and ETA (numeric variables) in a Directional Measures table. MCNEMAR adds to the chi-square table the exact McNemar test (2 x 2) or the McNemar-Bowker test (larger), for rows and columns with the same values. KAPPA works on the square table over the values of both variables (same type), unseen pairs counted 0. RISK gives a Risk Estimate table for 2 x 2 tables: the odds ratio and the risk for each column category, with 95% intervals. CMH treats the layers as strata of 2 x 2 tables: Breslow-Day and Tarone (homogeneity), Cochran and Mantel-Haenszel (conditional independence), and the common odds ratio with its 95% interval, tested against the value in brackets. Each measure gives its value, asymptotic standard error, approximate T and significance; ALL asks for every statistic (CMH with 1); a table with one row or column gets no measures.

MISSING=TABLE leaves out a case missing on any variable of the table; INCLUDE keeps user-missing values as categories. Fisher's test takes whole counts: rounded, or truncated with TRUNCATE.

BARCHART adds a clustered bar chart per table: a cluster per row category, a bar per column category, the counts of the table summed over its layers.

VARIABLES switches to integer mode: a range follows each list (it covers the whole list) and values are cut to whole numbers. Rows and columns are every value of the range, with or without cases; a value outside it makes the case missing. The statistics use the values that occur. TABLES then names only variables of VARIABLES, and TO and ALL follow their order there.

MISSING=REPORT (integer mode only) shows the user-missing values of the rows and columns marked (missing): their cells hold counts, nothing else, and stay out of totals, percentages and statistics; the cases count as missing.

METHOD adds significances computed from the tables that share the observed row and column totals, next to the asymptotic ones, on whole counts (cells rounded, or truncated with TRUNCATE). EXACT walks all such tables with a network of partial tables: the chi-square table gets the exact two-sided significance and the point probability of Pearson, the likelihood ratio, Fisher's test (any size) and the linear-by-linear association (also one-sided); the measures tables an Exact Significance column for phi, Cramer's V, the contingency coefficient, lambda (not the symmetric value), Goodman and Kruskal's tau, the uncertainty coefficient, Kendall's tau-b and tau-c, Somers' d, Spearman, Pearson's R and kappa. TIMER(minutes) bounds each statistic (0: no bound); one that runs over, or would need more memory than a quarter of what is free on the computer (at least 256 MB, at most 8 GB), keeps only its asymptotic value, with a warning that gives the amount. MC instead draws SAMPLES random tables with those totals from the SET SEED generator and shows the share at least as extreme with its interval at CIN percent. Gamma, eta, McNemar-Bowker and CMH stay asymptotic.

## Example

```text
CROSSTABS /TABLES=region BY gender /CELLS=COUNT ROW /STATISTICS=CHISQ.
```

`statscraft help CROSSTABS` prints the same text in the terminal. [All commands](README.md)
