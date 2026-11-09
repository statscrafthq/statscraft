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

[removed]

MRSETS defines multiple response sets (dichotomy and category sets, saved with the data file), and CTABLES tabulates them like categorical variables, with counts of cases or of responses and percentages based on either, and tests them for significance with the variants of the tests for multiple responses.

Sessions are stored under `.statscraft/sessions/` in the current directory.

## The window

```
cargo run -p statscraft-gui -- survey.sav job.sps     open a data file and a syntax file
```

`statscraft-gui` is a desktop window over the same engine, in four tabs:

- **Data**: the cases in a grid that draws only the visible cells, with value labels on or off. Double-click a cell to change it.
- **Variables**: one row per variable with name, type, format, width, decimals, label, value labels, missing values and measure. Double-click to change a field.
- **Syntax**: an editor with line numbers. Run all, or Run selection (Ctrl+Enter) for the commands on the selected lines. Errors show at their line and column, and the help panel (F1) follows the command under the cursor.
- **Output**: every result table with its multi-level header, an outline, Copy (tab-separated, for a spreadsheet) and Export as HTML.

The Analyze menu has dialogs for Frequencies, Descriptives, Crosstabs and Custom Tables (rows, columns, means, percentages, totals and significance tests); the Transform menu has Compute and Recode. A dialog shows the syntax it builds: OK runs it, Paste puts it in the syntax editor.

Syntax is the only command layer: every change and every result in the window comes from syntax run by the engine, exactly as `statscraft run` runs it. Opening a file runs `GET FILE`, editing a cell runs `IF ($CASENUM = n) var = value.`, editing a label runs `VARIABLE LABELS`, and so on. The log under the tabs lists every command with what it changed, and Copy syntax there gives a script that repeats the session.

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

## Development

```
cargo test
```

Test data lives in `tests/fixtures/`. It is synthetic and generated by the scripts in that folder.
