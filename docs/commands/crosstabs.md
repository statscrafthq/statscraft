<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# CROSSTABS

Contingency tables with percentages, residuals and chi-square tests.

## Syntax

```text
CROSSTABS [/][TABLES=]rowvars BY colvars [BY layervars ...] [/TABLES=...]
          [/VARIABLES=varlist (min, max) [varlist (min, max) ...]  (before TABLES)]
          [/CELLS=COUNT ROW COLUMN TOTAL EXPECTED RESID SRESID ASRESID ALL NONE]
          [/STATISTICS=CHISQ PHI CC LAMBDA UC BTAU CTAU GAMMA D ETA CORR KAPPA RISK MCNEMAR CMH[(1)] ALL NONE] [/MISSING=TABLE | INCLUDE | REPORT]
          [/FORMAT=AVALUE | DVALUE TABLES | NOTABLES]
          [/COUNT=[ASIS | CASE | CELL] [ROUND | TRUNCATE]] [/BARCHART]
          [/WRITE[=NONE | CELLS | ALL]]
          [/METHOD=MC [CIN(99)] [SAMPLES(10000)] | EXACT [TIMER(5)]].
```

The command makes one contingency table per pair of row and column variable; layer variables split it into blocks. The first TABLES= keyword may be left out. A Case Processing Summary comes first: the valid, missing and total cases of each table (weighted). CELLS defaults to COUNT; CELLS alone is COUNT ROW COLUMN TOTAL. Percentages run from 0 to 100. FORMAT=DVALUE orders the rows from the highest value; columns stay ascending. NOTABLES, like CELLS=NONE, drops the tables and keeps the statistics. MISSING=TABLE (default) leaves out a case missing on any variable of the table; INCLUDE keeps user-missing values as categories.

STATISTICS alone is CHISQ: Pearson, likelihood ratio, linear-by-linear (numeric variables), and for a 2 x 2 table Fisher's exact test (on whole counts, rounded or truncated by COUNT) and the continuity correction. ALL is every statistic, CMH with 1. Each measure gives its value, asymptotic standard error, approximate T and significance; a table with one row or column gets none. Symmetric Measures holds PHI (phi, Cramer's V), CC (contingency coefficient), BTAU, CTAU (Kendall's tau-b, tau-c), GAMMA and CORR (Spearman, Pearson; numeric only). Directional Measures holds LAMBDA (lambda, Goodman and Kruskal's tau), UC (uncertainty coefficient), D (Somers' d) and ETA (numeric).

KAPPA uses the square table over the values of both variables (same type), unseen pairs counted 0. MCNEMAR adds the exact McNemar test (2 x 2) or McNemar-Bowker (larger) to the chi-square table, for rows and columns with the same values. RISK gives a Risk Estimate table for 2 x 2 tables: the odds ratio and each column's risk, with 95% intervals. CMH treats the layers as strata of 2 x 2 tables: Breslow-Day and Tarone (homogeneity), Cochran and Mantel-Haenszel (conditional independence), and the common odds ratio with its 95% interval, tested against the value in parentheses.

COUNT makes weighted counts whole before anything is computed: CASE each case's weight, CELL each cell's count, by ROUND or TRUNCATE. A keyword left out is CELL or ROUND. Without COUNT, or with ASIS, the weights stay as they are.

VARIABLES switches to integer mode: a range follows each list and covers all of it, and values are cut to whole numbers. Rows and columns are every value of the range, with or without cases; a value outside it makes the case missing. Statistics use the values that occur. TABLES then names only variables of VARIABLES, and TO and ALL follow their order there. MISSING=REPORT (integer mode only) shows user-missing row and column values, marked (missing): their cells hold counts only, stay out of totals, percentages and statistics, and the cases count as missing.

BARCHART adds a clustered bar chart per table: a cluster per row category, a bar per column category, counts summed over the layers. FORMAT also accepts BOX, NOBOX, INDEX, NOINDEX, PIVOT and NOPIVOT, which change nothing, with one warning.

WRITE writes one record per cell to the file of the PROCEDURE OUTPUT before the command, after an Output File Summary table. Columns 1-16 hold the split-file group (from 1), table number and count as whole numbers; then come the row, column and control values in 8-column fields (whole numbers in integer mode, else as the variable prints, text to the left). Records go a table column at a time, the last control variable changing slowest. CELLS (also bare WRITE) writes the non-empty cells without a missing value, or with them under MISSING=INCLUDE or REPORT; ALL (integer mode only) every cell of the ranges. A long string variable or more than eight dimensions is an error.

METHOD adds significances from the tables with the observed row and column totals, next to the asymptotic ones, on whole counts (rounded, or truncated with TRUNCATE). EXACT enumerates those tables: the chi-square table gets the exact two-sided significance and point probability of Pearson, likelihood ratio, Fisher's test (any size) and linear-by-linear (also one-sided), and the measures tables an Exact Significance column for every measure except gamma, eta and the symmetric lambda. TIMER(minutes) bounds each statistic (0: no bound); one that runs over or needs too much memory keeps only its asymptotic value, with a warning. MC instead draws SAMPLES random tables with those totals from the SET SEED generator and shows the share at least as extreme, with its interval at CIN percent. McNemar-Bowker and CMH stay asymptotic.

## Example

```text
GET FILE='survey.sav'.
CROSSTABS /TABLES=recommend BY region.
CROSSTABS /TABLES=switching BY age_group /CELLS=COUNT COLUMN /STATISTICS=CHISQ.
WEIGHT BY weight.
CROSSTABS /TABLES=satisfaction BY gender BY region /CELLS=COLUMN.
```

The example reads the sample data file [survey.sav](../sample/survey.sav), an invented customer survey; save it in the folder you run the syntax from.

## Example output

### Would recommend us to a friend by Region

| Would recommend us to a friend | Statistic | Region / North | Region / South | Region / East | Region / West | Total |
| --- | --- | --- | --- | --- | --- | --- |
| No | Count | 20 | 23 | 37 | 33 | 113 |
| Yes | Count | 55 | 39 | 44 | 49 | 187 |
| Total | Count | 75 | 62 | 81 | 82 | 300 |

`statscraft help CROSSTABS` prints the same text in the terminal. [All commands](README.md)
