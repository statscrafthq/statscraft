# StatsCraft

A statistics workbench written in Rust: a command-line engine first, a desktop application after. It reads .sav files, runs syntax compatible with the reference program syntax, and is built to be driven by people and AI agents alike: sessions that keep the data between partial runs, JSON output, errors with line and column, and help generated from the parser.

StatsCraft is an independent project. It is not affiliated with any software vendor, the GNU PSPP project or the ArtCraft suite.

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

What runs today: reading and writing data files, labels and missing values, transformations (COMPUTE, IF, DO IF, RECODE, COUNT, SELECT IF, AGGREGATE, SORT CASES, merging files), FREQUENCIES, DESCRIPTIVES, CROSSTABS, MEANS, T-TEST, ONEWAY, CORRELATIONS, NONPAR CORR, EXAMINE, and custom tables (CTABLES) with multiple response sets and significance tests. The [command pages](docs/commands/README.md) list every command.

DATA LIST reads text data in fixed columns, freefield (FREE) or one case per line (LIST), inline between BEGIN DATA and END DATA or from a file, and warns with the line number of every value it cannot read.

SPLIT FILE runs every procedure per group of adjacent cases, LAYERED in one table or SEPARATE, FILTER leaves cases out of procedures without deleting them, TEMPORARY makes the changes after it last until the next procedure, N OF CASES keeps the first cases and SAMPLE a random sample; split and filter stay with the session's dataset.

Scratch variables (`#name`) start at 0 or blank, keep their value from case to case and are gone at the next procedure; LAG gives a variable's value in an earlier case as that case ended the data pass, and a pass that needs earlier cases runs case by case.

DO REPEAT repeats transformations with stand-ins replaced by variables or values (END REPEAT PRINT lists the generated commands), LOOP repeats them per case with an index, IF conditions and BREAK, and VECTOR names a range of existing or new variables for indexed use such as `v(#i)`.

AUTORECODE turns the values of variables into consecutive codes in new variables, labelled with the old values or their labels, in ascending or descending order, with user-missing values last and one scheme for several variables with /GROUP.

## Documentation

- [Getting started](docs/getting-started.md): install, open a file, run syntax, the window in five minutes.
- [Changelog](CHANGELOG.md): what is in each release.
- [The window](docs/window.md): the data editor, syntax editor, output view, menus and dialogs.
- [The command line](docs/command-line.md): every subcommand and flag, sessions, JSON output, exit codes, Excel export, the live window session, OpenStatSpec plans and SQLite.
- [AI agents](docs/ai-agents.md): how an agent should work with StatsCraft, and instructions to paste into yours.
- [Commands](docs/commands/README.md): one page per supported command, generated from the help table.
- [Differences and limits](docs/differences.md): deliberate differences from other implementations, and what is not supported yet.

## Getting a build

The manual workflow **Release build** (Actions, Release build, Run workflow, or `gh workflow run release.yml`) gives a per-user Windows installer, a .deb for Debian and Ubuntu, and a .tar.gz for other Linux, each with `statscraft` and `statscraft-gui`; ticking **draft release** also attaches them to a draft GitHub Release. [Getting started](docs/getting-started.md#install) explains how to install and uninstall them. The packaging scripts are in `packaging/`, the icon in `crates/gui/icons/`. To build from source you need Rust 1.95 or newer: `cargo build --release -p statscraft -p statscraft-gui`.

## Releasing a version

1. Set the new version in `[workspace.package]` of the root `Cargo.toml` (for example `version = "0.2.0"`), run `cargo build` so `Cargo.lock` follows, and commit and push to `main`.
2. Run the **Release build** workflow on `main` with **draft release** ticked (`gh workflow run release.yml -f draft_release=true`). It builds the three packages and creates a draft release `v0.2.0` with them, a `SHA256SUMS` file and notes generated from the commits.
3. Review the draft on the Releases page: edit the notes, and try the packages if you like.
4. Publish it. Only then do the windows and `statscraft update` see it; the workflow never publishes by itself.

The updaters compare the release tag `vX.Y.Z` with their own version, so every release needs a higher version in `Cargo.toml`.

## Development

```
cargo test
```

Test data lives in `tests/fixtures/`. It is synthetic and generated by the scripts in that folder.

The command pages in `docs/commands/` are generated from the command table in `crates/syntax/src/table.rs`. A test fails when they are out of date; regenerate them with:

```
UPDATE_DOCS=1 cargo test -p statscraft-syntax --test command_docs
```

The OpenStatSpec conformance tests (`crates/openstatspec/tests/conformance.rs` and the database runners) read the published manifests from a local checkout of the specification: the folder in `OPENSTATSPEC_DIR`, or `openStatSpec/specification` next to any parent folder of the repository. Without it the test is skipped.
