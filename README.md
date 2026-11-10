# StatsCraft

A statistics workbench written in Rust: a command-line engine first, a desktop application after.

StatsCraft is an independent project. It is not affiliated with any software vendor, the GNU PSPP project or the ArtCraft suite.

Status: early development. Licensed under MIT.

## What works today

```
statscraft describe survey.sav --json           dictionary and counts of a file
statscraft run job.sps                          run a syntax file
statscraft run -s work job.sps                  run it in a session: the dataset is kept between runs
statscraft run -s work --lines 40-55 job.sps    run only the commands on those lines
statscraft run -s work -e "FREQUENCIES age."    run syntax given on the command line
statscraft check -s work job.sps                validate against the session without saving anything
statscraft describe -s work --json              dictionary and counts of the session's dataset
statscraft help FREQUENCIES                     syntax of one command; without a name, the list
```

Add `--json` to `run` and `check` for structured output: result tables, a summary of what changed in the dataset, and errors with line and column.

A run is all or nothing: if one command fails, nothing is saved, so the same lines can be run again after the fix. A line range must cover whole commands.

Commands executed so far: GET, SAVE, VARIABLE LABELS, VALUE LABELS, MISSING VALUES, VARIABLE LEVEL, FORMATS, FREQUENCIES (weighted when the file has a weight variable), EXECUTE, COMPUTE, IF, SELECT IF, STRING, NUMERIC, DO IF / ELSE IF / ELSE / END IF, RECODE, COUNT, SORT CASES, RENAME VARIABLES, DELETE VARIABLES, WEIGHT and AGGREGATE. DESCRIPTIVES (with z-scores through /SAVE) and CROSSTABS (cell percentages, residuals and chi-square tests) run weighted when a weight variable is set, and ADD FILES and MATCH FILES combine the active dataset with saved files. Expressions follow the missing-value rules of the language and cover arithmetic, statistical, missing-value, string and date functions.

MEANS reports means, counts, medians and other statistics of dependent variables within nested groups of control variables, with an optional analysis of variance and test of linearity.
MEANS runs weighted when a weight variable is set. A command that does something differently from what was asked without failing (a skipped string variable, a reversed range) reports a warning, shown with the output and in the JSON as `warnings` per command.

[removed]

MRSETS defines multiple response sets (dichotomy and category sets, saved with the data file), and CTABLES tabulates them like categorical variables, with counts of cases or of responses and percentages based on either, and tests them for significance with the variants of the tests for multiple responses.

Sessions are stored under `.statscraft/sessions/` in the current directory.

## OpenStatSpec transformation plans

```
statscraft plan job.sps --input-schema schema.json     lower syntax to a transformation plan
statscraft plan -e "RECODE q1 (1 = 0)." --input-schema schema.json --profile 0.1
statscraft plan -s work job.sps                        take the input schema from a session's dataset
```

`plan` implements the the reference program syntax frontend profiles 0.1, 0.2 and 0.3 (the default) of OpenStatSpec, an open specification for statistical datasets in SQL databases: it turns a bounded subset of syntax into a language-neutral Transformation Plan 0.1 or 0.2. The subset is RECODE, VARIABLE LABELS and VALUE LABELS (0.1), plus COMPUTE and IF with single values and simple comparisons, FORMATS, VARIABLE LEVEL and EXECUTE (0.2), plus comments, TO lists, NE and NOT, LOWEST and HIGHEST ranges and ADD VALUE LABELS (0.3). Everything else is rejected, even where `statscraft run` accepts it.

The output is one line of canonical JSON: the plan with its SHA-256 plan hash and the hash of the source text, or an error with a stable code, line and column. The schema file has the form `{"variables": [{"name": "q1", "storage_kind": "numeric"}, ...]}`, as the profile's request schema defines it.

## Datasets in SQLite (OpenStatSpec)

```
statscraft db export survey.sav --sqlite study.db [--name survey]   store a dataset in SQLite
statscraft db export -s work --sqlite study.db --name work          store a session's dataset
statscraft db import --sqlite study.db --name survey --out back.sav read it back into a file
statscraft db import --sqlite study.db --name survey -s work        or into a session
statscraft plan job.sps --sqlite study.db --name survey --apply     apply syntax to it in place
```

`db` stores a dataset the way OpenStatSpec's SAV/ZSAV profile 1.0 and its SQLite dialect profile specify: one table `data_<name>` with `__case_ordinal` (1, 2, 3, ... in case order) and one column per variable in variable order, numbers as REAL with system-missing as NULL, strings as TEXT NOT NULL, user-missing codes kept as values, dates as numbers. The dictionary (labels, value labels, missing values, formats, measurement level, role, display settings, attributes, documents, weight variable, multiple response sets) goes into the catalog tables. The database file belongs to OpenStatSpec: `export` sets up the catalog in a new or empty file and refuses a file that holds anything else. Before writing, a preflight checks the column limit (2000 including `__case_ordinal`), identifiers and value sizes; NaN and negative zero are refused because SQLite cannot keep them. A failure leaves no table and no dataset row, only the failed operation and its diagnostic. Variable names that are not plain ASCII get generated column names (`v<position>`); the catalog maps them back. Reading a dataset whose catalog holds items StatsCraft cannot keep (dataset attributes, variable sets, write formats other than the print format) fails unless `--allow-loss` is given.

`plan --sqlite DB --name DATASET` takes the input schema from that dataset; with `--apply` it then applies the plan to the same table in one transaction: direct UPDATEs, `ALTER TABLE ... ADD COLUMN` for new variables, catalog updates and one audit row in `transformation_apply` (plan and source hashes, actor, operation count). Nothing is copied, and on any error nothing changes. `--actor` names who applies the plan (default `$USER`).

## The window

```
cargo run -p statscraft-gui -- survey.sav job.sps     open a data file and a syntax file
```

`statscraft-gui` is a desktop window over the same engine, in four tabs:

- **Data**: the cases in a grid that draws only the visible cells, with value labels on or off. Double-click a cell or press Enter to change it; Enter commits, Escape cancels. Arrows, Page Up and Down, Home and End move the selection, Ctrl+Home and Ctrl+End go to the first and last cell, and Tab and Shift+Tab move to the next and previous cell, committing an edit on the way.
- **Variables**: one row per variable with name, type, format, width, decimals, label, value labels, missing values and measure. Double-click to change a field.
- **Syntax**: an editor with line numbers. Run all, or Run selection (Ctrl+Enter) for the commands on the selected lines. Errors show at their line and column, and the help panel (F1) follows the command under the cursor.
- **Output**: every result table with its multi-level header, an outline, Copy (tab-separated, for a spreadsheet) and Export as HTML.

The Analyze menu has dialogs for Frequencies, Descriptives, Crosstabs and Custom Tables (rows, columns, means, percentages, totals and significance tests); the Transform menu has Compute and Recode. A dialog shows the syntax it builds: OK runs it, Paste puts it in the syntax editor.

Syntax is the only command layer: every change and every result in the window comes from syntax run by the engine, exactly as `statscraft run` runs it. Opening a file runs `GET FILE`, editing a cell runs `IF ($CASENUM = n) var = value.`, editing a label runs `VARIABLE LABELS`, and so on. The log under the tabs lists every command with what it changed, and Copy syntax there gives a script that repeats the session.

Edit, Undo last change (Ctrl+Z outside a text field) puts back the dataset from before the last run that changed it, one step; the log marks that run as undone and Copy syntax leaves it out. Long lines in the syntax editor wrap at the window edge; View turns that off.

Opening a file and every run happen off the drawing thread: the status bar shows what is running and the window stays responsive. One run goes at a time; requests from the command line wait their turn.

### Driving the window from the command line or an AI agent

While the window is open it is a live session named `gui`:

```
statscraft run -s gui -e "GET FILE='survey.sav'."         the window opens the file
statscraft run -s gui --json job.sps                      runs job.sps in the window
statscraft check -s gui -e "COMPUTE x = age / 10."       validates against the window's data
statscraft describe -s gui --json                        dictionary of the window's dataset
```

The window runs the syntax exactly as its own Run button does: the tables appear in Output, the commands in the log, and the data grid shows the result. The command line prints what it prints for a file session, with `"session": "gui"`; `saved` is true when the run changed the window's dataset (nothing is written to disk unless the syntax says SAVE). Relative paths in the syntax are read from the folder where `statscraft` was started, and the window stays in that folder.

So an AI agent with a shell can work in the open window: start `statscraft-gui`, then tell the agent to use `statscraft run -s gui --json` for every step and `statscraft help COMMAND` for the syntax. The person watching sees each table appear and can keep working in the window between the agent's runs.

How it connects: on start the window listens on a random TCP port of 127.0.0.1 and writes the address and a random token to `statscraft/gui.json` in `$XDG_RUNTIME_DIR` on Linux or `%LOCALAPPDATA%` on Windows (`~/.config` otherwise), readable only by the user. A request without the token is refused, and the command line sends the token only to a local address. When no window is open, `-s gui` fails with "no StatsCraft window is running". With several windows open, the one started last answers.

The window uses system fonts (Segoe UI and Consolas on Windows, DejaVu, Noto or Liberation on Linux); set `STATSCRAFT_FONT` or `STATSCRAFT_MONO_FONT` to a .ttf file to choose others.

## Getting a build for Windows or Linux

The workflow **Release build** builds `statscraft` and `statscraft-gui` for x86_64 Windows and Linux. It runs only when started by hand: on GitHub open Actions, choose Release build, then Run workflow (or `gh workflow run release.yml`). When it finishes, the run page lists two artifacts, `statscraft-windows-x86_64.zip` and `statscraft-linux-x86_64.zip`, each with both programs, this README and the license. There is no installer yet: unzip and start `statscraft-gui.exe`. The window opens without a console; `statscraft.exe` is the command line and runs in a terminal.

## Development

```
cargo test
```

Test data lives in `tests/fixtures/`. It is synthetic and generated by the scripts in that folder.

The OpenStatSpec conformance tests (`crates/openstatspec/tests/conformance.rs` and `sqlite_conformance.rs`) read the published manifests from a local checkout of the specification: the folder in `OPENSTATSPEC_DIR`, or `openStatSpec/specification` next to any parent folder of the repository. Without it the test is skipped.
