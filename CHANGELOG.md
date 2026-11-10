# Changelog

## 0.1.0 (2026-11-10, early release)

The first version for people who want to try StatsCraft and tell us what breaks. Expect rough edges; please report them (Issues, Bug report).

### What it does

- Reads and writes .sav and .zsav files, including labels, missing values, multiple response sets, dates and old encodings.
- Runs syntax compatible with the reference program: transformations (COMPUTE, IF, RECODE, COUNT, DO IF, SELECT IF, DO REPEAT, LOOP, VECTOR, AGGREGATE, AUTORECODE, file merging, SPLIT FILE, FILTER, TEMPORARY and more) and procedures (FREQUENCIES, DESCRIPTIVES, CROSSTABS, CTABLES, MEANS, T-TEST, ONEWAY, CORRELATIONS, NONPAR CORR, EXAMINE).
- Custom tables: stacking, nesting, layers, totals and subtotals, percentages, multiple response sets, significance tests with column letters, effective base.
- Desktop window: data and variable views, syntax editor, output view, dialogs that write syntax, Excel and HTML export.
- Command line built for scripts and AI agents: sessions, running parts of a file by line range, JSON output with precise error positions, and a live link to the open window.
- OpenStatSpec: store datasets in SQLite, PostgreSQL, MySQL, MariaDB or Dolt and apply transformation plans in place.

### Known limits

- The Windows installer is not signed yet, so Windows shows an "unknown publisher" warning; see [Getting started](docs/getting-started.md).
- No charts yet. Some commands and subcommands are not supported; see [Differences](docs/differences.md) and `statscraft help`.
