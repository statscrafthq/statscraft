# Changelog

## 0.1.1 (2026-11-10)

- FREQUENCIES supports every subcommand (statistics with standard errors, formats, percentiles, ntiles, grouped data, order); chart requests give a warning instead of an error.
- RANK with every function, tie rule and fraction method.
- Encrypted database connections (TLS with certificate checks) and much faster storing and reading of large datasets in PostgreSQL, MySQL, MariaDB, Dolt and SQLite.
- Update checks run quietly in the background; the window no longer shows a notice about them on the first start.

## 0.1.0 (2026-11-10, early release)

The first version for people who want to try StatsCraft and tell us what breaks. Expect rough edges; please report them (Issues, Bug report).

### What it does

- Reads and writes .sav and .zsav files, including labels, missing values, multiple response sets, dates and old encodings.
- Macros (DEFINE with every argument kind, !LET, !IF, !DO and the string functions), INCLUDE and INSERT, syntax files in old encodings, SET, DATASET and the dictionary commands, GET DATA for text files.
- Runs syntax compatible with the reference program: transformations (COMPUTE, IF, RECODE, COUNT, DO IF, SELECT IF, DO REPEAT, LOOP, VECTOR, AGGREGATE, AUTORECODE, file merging, SPLIT FILE, FILTER, TEMPORARY and more) and procedures (FREQUENCIES, DESCRIPTIVES, CROSSTABS, CTABLES, MEANS, T-TEST, ONEWAY, CORRELATIONS, NONPAR CORR, EXAMINE).
- Custom tables: stacking, nesting, layers, totals and subtotals, percentages, multiple response sets, significance tests with column letters, effective base.
- Desktop window: data and variable views, syntax editor, output view, dialogs that write syntax, Excel and HTML export.
- Command line built for scripts and AI agents: sessions, running parts of a file by line range, JSON output with precise error positions, and a live link to the open window.
- OpenStatSpec: store datasets in SQLite, PostgreSQL, MySQL, MariaDB or Dolt and apply transformation plans in place.

### Known limits

- The Windows installer is not signed yet, so Windows shows an "unknown publisher" warning; see [Getting started](docs/getting-started.md).
- No charts yet. Some commands and subcommands are not supported; see [Differences](docs/differences.md) and `statscraft help`.
