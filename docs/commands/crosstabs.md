<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# CROSSTABS

Contingency tables with percentages, residuals and chi-square tests.

## Syntax

```text
CROSSTABS [/]TABLES=rowvars BY colvars [BY layervars ...] [/TABLES=...]
          [/CELLS=COUNT ROW COLUMN TOTAL EXPECTED RESID SRESID ASRESID ALL NONE]
          [/STATISTICS=CHISQ] [/MISSING=TABLE | INCLUDE]
          [/FORMAT=AVALUE | DVALUE TABLES | NOTABLES]
          [/COUNT=[ASIS | CASE | CELL] [ROUND | TRUNCATE]].
```

One table per pair of row and column variable; layer variables split it into blocks.

NOTABLES leaves out the tables, as CELLS=NONE does, keeping the statistics.

COUNT makes weighted counts whole: CASE every case's weight, CELL every cell's count, before anything is computed; ROUND or TRUNCATE. A keyword left out is CELL or ROUND; without COUNT, ASIS, weights stay as they are.

CELLS defaults to COUNT; CELLS alone is COUNT ROW COLUMN TOTAL. Percentages are 0 to 100.

STATISTICS alone is CHISQ. DVALUE orders the rows from the highest value; columns stay ascending.

CHISQ: Pearson, likelihood ratio, linear-by-linear (numeric variables), and for a 2 x 2 table Fisher's exact test and the continuity correction.

MISSING=TABLE leaves out a case missing on any variable of the table; INCLUDE keeps user-missing values as categories. Fisher's test takes whole counts: rounded, or truncated with TRUNCATE.

## Example

```text
CROSSTABS /TABLES=region BY gender /CELLS=COUNT ROW /STATISTICS=CHISQ.
```

`statscraft help CROSSTABS` prints the same text in the terminal. [All commands](README.md)
