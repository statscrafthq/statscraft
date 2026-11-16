<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# CROSSTABS

Contingency tables with percentages, residuals and chi-square tests.

## Syntax

```text
CROSSTABS [/]TABLES=rowvars BY colvars [BY layervars ...] [/TABLES=...]
          [/CELLS=COUNT ROW COLUMN TOTAL EXPECTED RESID SRESID ASRESID ALL NONE]
          [/STATISTICS=CHISQ PHI CC LAMBDA UC BTAU CTAU GAMMA CORR KAPPA RISK MCNEMAR NONE] [/MISSING=TABLE | INCLUDE]
          [/FORMAT=AVALUE | DVALUE TABLES | NOTABLES]
          [/COUNT=[ASIS | CASE | CELL] [ROUND | TRUNCATE]] [/BARCHART].
```

First a Case Processing Summary: per table the valid, missing and total cases (weighted).

One table per pair of row and column variable; layer variables split it into blocks.

NOTABLES leaves out the tables, as CELLS=NONE does, keeping the statistics.

COUNT makes weighted counts whole: CASE every case's weight, CELL every cell's count, before anything is computed; ROUND or TRUNCATE. A keyword left out is CELL or ROUND; without COUNT, ASIS, weights stay as they are.

CELLS defaults to COUNT; CELLS alone is COUNT ROW COLUMN TOTAL. Percentages are 0 to 100.

STATISTICS alone is CHISQ. DVALUE orders the rows from the highest value; columns stay ascending.

CHISQ: Pearson, likelihood ratio, linear-by-linear (numeric variables), and for a 2 x 2 table Fisher's exact test and the continuity correction.

PHI (phi and Cramer's V), CC (contingency coefficient), BTAU, CTAU (Kendall's tau-b, tau-c), GAMMA and CORR (Spearman and Pearson, numeric variables only) go in a Symmetric Measures table; LAMBDA (lambda and Goodman and Kruskal's tau) and UC (uncertainty coefficient) in a Directional Measures table. MCNEMAR adds to the chi-square table the exact McNemar test (2 x 2) or the McNemar-Bowker test (larger), for rows and columns with the same values. KAPPA works on the square table over the values of both variables (same type), unseen pairs counted 0. RISK gives a Risk Estimate table for 2 x 2 tables: the odds ratio and the risk for each column category, with 95% intervals. Each measure gives its value, asymptotic standard error, approximate T and significance; a table with one row or column gets none.

MISSING=TABLE leaves out a case missing on any variable of the table; INCLUDE keeps user-missing values as categories. Fisher's test takes whole counts: rounded, or truncated with TRUNCATE.

BARCHART adds a clustered bar chart per table: a cluster per row category, a bar per column category, the counts of the table summed over its layers.

## Example

```text
CROSSTABS /TABLES=region BY gender /CELLS=COUNT ROW /STATISTICS=CHISQ.
```

`statscraft help CROSSTABS` prints the same text in the terminal. [All commands](README.md)
