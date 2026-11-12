# Statscraft

Statscraft is a free, open-source statistics package for the social sciences, built AI-native from the first line. It is an alternative to SPSS, PSPP, Stata and SAS: it reads .sav files and runs your existing syntax, on Windows, macOS and Linux. Two programs written in Rust share one engine: the command line `statscraft` and the desktop window `statscraft-gui`.

[Download](https://statscraft.org/download.html) · [Documentation](https://docs.statscraft.org) · [Website](https://statscraft.org)

It is built to be driven by people and AI agents alike:

- sessions that keep the data between partial runs;
- JSON output;
- errors with line and column;
- help generated from the program's command table.

Statscraft is an independent project. It is not affiliated with or endorsed by the makers of the products named above or the ArtCraft suite. Product names are trademarks of their owners.

Status: early development. Licensed under MIT.

```
statscraft describe tests/fixtures/survey_basic.sav --json    dictionary and counts of a file
statscraft run job.sps                                        run a syntax file
statscraft run -s work --lines 1-6 job.sps                    run some lines in a session that keeps the data
statscraft check -s work job.sps                              validate without saving anything
statscraft run job.sps --xlsx tables.xlsx                     also write every table to an Excel workbook
statscraft help CTABLES                                       syntax of one command
statscraft-gui tests/fixtures/survey_basic.sav job.sps        the desktop window
```

(`job.sps` is the example from [Getting started](docs/getting-started.md#run-a-syntax-file).)

## What runs today

The [command pages](docs/commands/README.md) list every supported command. In short:

- **Data files.** .sav and .zsav files, read and written, with labels, missing values, multiple response sets and old encodings. Portable files with IMPORT and EXPORT. Excel workbooks and text files with GET DATA and GET TRANSLATE. SAVE TRANSLATE writes CSV, tab-delimited text and .xlsx.
- **Text data.** DATA LIST reads text data in fixed columns, freefield (FREE) or one case per line (LIST), inline between BEGIN DATA and END DATA or from a file. It warns with the line number of every value it cannot read. Input programs (INPUT PROGRAM, FILE TYPE, REPEATING DATA) build cases from files with several kinds of records.
- **Transformations.** COMPUTE, IF, DO IF, RECODE, COUNT, SELECT IF, AGGREGATE, SORT CASES, merging files (ADD FILES, MATCH FILES, UPDATE, STAR JOIN) and restructuring (VARSTOCASES, CASESTOVARS, FLIP).
  - DO REPEAT repeats transformations with stand-ins replaced by variables or values (END REPEAT PRINT lists the generated commands).
  - LOOP repeats them per case with an index, IF conditions and BREAK.
  - VECTOR names a range of existing or new variables for indexed use, such as `v(#i)`.
  - Scratch variables (`#name`) start at 0 or blank, keep their value from case to case and are gone at the next procedure.
  - LAG gives a variable's value in an earlier case, as that case ended the data pass. A pass that needs earlier cases runs case by case.
  - RANK saves ranks, fractional ranks, percentile groups, proportion estimates, normal and Savage scores as new variables, within groups of BY variables, with the four ways of treating ties.
  - AUTORECODE turns the values of variables into consecutive codes in new variables, labeled with the old values or their labels, in ascending or descending order, with user-missing values last. /GROUP gives several variables one scheme.
- **Selecting and splitting cases.**
  - SPLIT FILE runs every procedure per group of adjacent cases, LAYERED in one table or SEPARATE.
  - FILTER leaves cases out of procedures without deleting them.
  - TEMPORARY makes the changes after it last until the next procedure.
  - N OF CASES keeps the first cases, and SAMPLE a random sample.
  - Split and filter stay with the session's dataset.
- **Procedures.** FREQUENCIES, DESCRIPTIVES, CROSSTABS, MEANS, T-TEST, ONEWAY, CORRELATIONS, NONPAR CORR, PARTIAL CORR, NPAR TESTS, EXAMINE and REPORT, and:
  - linear regression (REGRESSION, with stepwise selection, residuals and diagnostics);
  - scale reliability (RELIABILITY: alpha, split-half and other models, intraclass correlations);
  - logistic regression (LOGISTIC REGRESSION, with categorical covariates, stepwise selection and case diagnostics);
  - the general linear model (UNIANOVA and GLM: factorial ANOVA and ANCOVA with Type I to IV sums of squares, estimated marginal means, post hoc tests, contrasts and repeated measures), and the older ANOVA with MCA tables;
  - factor analysis (FACTOR) and k-means clustering (QUICK CLUSTER);
  - discriminant analysis (DISCRIMINANT, direct and stepwise, with classification);
  - hierarchical clustering (CLUSTER, 37 proximity measures and seven methods);
  - classification trees (TREE), below.
- **Classification trees.** TREE grows a classification tree for a nominal or ordinal dependent by CHAID or exhaustive CHAID. Predictor categories are merged by significance, splits are Bonferroni-adjusted, scale predictors are cut into groups of equal size, and missing values form a floating category, with the usual growth limits. It shows the tree as a table, the model summary, risk, classification and gain tables, and saves the node, the predicted category and the predicted probabilities. CRT and QUEST are not supported yet.
- **Custom tables.** CTABLES with multiple response sets and significance tests, and MULT RESPONSE.
- **The matrix language.** MATRIX ... END MATRIX, and MATRIX DATA.
- **Macros and settings.** DEFINE, INCLUDE, INSERT, SET, SHOW, PRESERVE and RESTORE.
- **Output.** Tables as Markdown, JSON, Excel workbooks, and with OUTPUT EXPORT also HTML, text and CSV. OUTPUT SAVE and OUTPUT OPEN keep output in Statscraft output files (.scout), and OUTPUT MODIFY deletes, hides, renames, sorts, transposes and reformats the items of the output. Charts are drawn in the window and saved as PNG or SVG, and open as editable charts in PowerPoint, Word and Excel.

Not yet: the overlay, matrix and three-dimensional scatterplots of GRAPH, and the fonts, colors and table looks of OUTPUT MODIFY. [Differences and limits](docs/differences.md) has the details.

## Documentation

The documentation is online at [docs.statscraft.org](https://docs.statscraft.org); the same pages are in `docs/`.

- [Getting started](docs/getting-started.md): install, open a file, run syntax, the window in five minutes.
- [Changelog](CHANGELOG.md): what is in each release.
- [The window](docs/window.md): the data editor, syntax editor, output view, menus and dialogs.
- [The command line](docs/command-line.md): every subcommand and flag, sessions, JSON output, exit codes, Excel export, the live window session, OpenStatSpec plans and SQLite.
- [AI agents](docs/ai-agents.md): how an agent should work with Statscraft, and instructions to paste into yours.
- [The MCP server](docs/mcp.md): `statscraft mcp`, the tools an agent app gets, sessions and safety.
- [Connect your AI agent](docs/connect.md): `statscraft connect` adds the Statscraft MCP server to the agents on your computer; the bundle for Claude Desktop, the plugin for Claude Code and the agent skill.
- [Commands](docs/commands/README.md): one page per supported command, generated from the help table.
- [Differences and limits](docs/differences.md): deliberate differences from other implementations, and what is not supported yet.

## Getting a build

The manual workflow **Release build** (Actions, Release build, Run workflow, or `gh workflow run release.yml`) gives a per-user Windows installer and a portable Windows .zip, a .deb for Debian and Ubuntu, a .tar.gz for other Linux, and a .tar.gz for macOS (Apple silicon and Intel) with Statscraft.app and the command line, each with `statscraft` and `statscraft-gui`, an MCP bundle (.mcpb) for Claude Desktop on each of the three systems, the agent skill as a .zip, and a `SHA256SUMS` of them; ticking **draft release** also attaches them to a draft GitHub Release. [Getting started](docs/getting-started.md#install) explains how to install and uninstall them. The packaging scripts are in `packaging/`, the icon in `crates/gui/icons/`. To build from source you need Rust 1.95 or newer: `cargo build --release -p statscraft -p statscraft-gui`.

## Releasing a version

1. Set the new version in `[workspace.package]` of the root `Cargo.toml` (for example `version = "0.2.0"`) and in `plugins/statscraft/.claude-plugin/plugin.json` (a test checks that they agree), run `cargo build` so `Cargo.lock` follows, and commit and push to `main`.
2. Run the **Release build** workflow on `main` with **draft release** ticked (`gh workflow run release.yml -f draft_release=true`). It builds the packages and creates a draft release `v0.2.0` with them, a `SHA256SUMS` file and notes generated from the commits.
3. Review the draft on the Releases page: edit the notes, and try the packages if you like.
4. Publish it. Only then do the windows and `statscraft update` see it; the workflow never publishes by itself.

The updaters compare the release tag `vX.Y.Z` with their own version, so every release needs a higher version in `Cargo.toml`.

## Development

```
cargo test
```

After cloning, run `git config core.hooksPath .githooks` once. The hooks keep vendor and product names of other companies out of the repository.

Test data lives in `tests/fixtures/`. It is synthetic and generated by the scripts in that folder.

The command pages in `docs/commands/` are generated from the command table in `crates/syntax/src/table.rs`. A test fails when they are out of date; regenerate them with:

```
UPDATE_DOCS=1 cargo test -p statscraft-syntax --test command_docs
```

The OpenStatSpec conformance tests (`crates/openstatspec/tests/conformance.rs` and the database runners) read the published manifests from a local checkout of the specification: the folder in `OPENSTATSPEC_DIR`, or `openStatSpec/specification` next to any parent folder of the repository. Without it the test is skipped.
