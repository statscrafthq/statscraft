# The command line

```
statscraft describe   dictionary and counts of a data file or a session
statscraft run        run syntax
statscraft check      validate syntax without saving or writing anything
statscraft help       syntax of a command, or the list of commands
statscraft plan       lower syntax to an OpenStatSpec transformation plan
statscraft db         store datasets in an OpenStatSpec database and read them back
statscraft update     check for a newer release, or update this installation to it
statscraft mcp        serve these tools to an AI agent over MCP
statscraft connect    add the Statscraft MCP server to the AI agents on this computer
```

`statscraft --version` prints the version, and `statscraft <subcommand> --help` lists the flags of a subcommand. The examples run from the root of the repository and use the synthetic files in `tests/fixtures/`; `job.sps` is the file from [Getting started](getting-started.md#run-a-syntax-file).

## describe

```
statscraft describe FILE [--json]
statscraft describe -s SESSION [--json]
```

| Flag | Meaning |
|---|---|
| `FILE` | a .sav or .zsav file |
| `-s`, `--session NAME` | describe a session instead: its dataset and the state it keeps; `gui` is the open window |
| `--json` | the full dictionary as JSON instead of one line per variable |

Give either a file or a session. The text form prints `cases, variables` and then, per variable, name, format, measurement level, valid count and label, separated by tabs. For a session it goes on with what the session keeps besides the data, one line each when there is something: `settings:`, `PRESERVE levels:`, `directory:`, `active dataset:`, `datasets:`, `macros:`, `output documents:`, `designated output:`. In JSON these are the object `session` (see [Sessions](#sessions-and-line-ranges)).

The datasets and output documents in `session`:

| Field | What |
|---|---|
| `active_dataset` | the name of the active dataset, the one every command works on; absent when it has no name |
| `datasets` | every named dataset, the active one first: `name`, and `cases` and `variables` when it holds data (a name only declared with DATASET DECLARE has none) |
| `output_documents` | every open output document in the order it was opened: `name` and the number of `tables` |
| `designated_output` | the document the next tables go to (OUTPUT NEW and OUTPUT ACTIVATE set it); absent when none is open, and the next table then starts `Document1` |

```json
"session": {
  "active_dataset": "DataSet2",
  "datasets": [
    {"name": "DataSet2", "cases": 200, "variables": 19},
    {"name": "DataSet1", "cases": 150, "variables": 12}
  ],
  "output_documents": [{"name": "Document1", "tables": 4}],
  "designated_output": "Document1"
}
```

The open window (`-s gui`) lists what the window holds, the same way. A file session keeps no output documents between runs, so there the list is empty.

## run

```
statscraft run [FILE | - | -e TEXT] [-s SESSION] [--lines FIRST-LAST] [--json] [--vega-lite] [--encoding NAME] [--xlsx FILE [--xlsx-one-sheet]]
```

| Flag | Meaning |
|---|---|
| `FILE` | a syntax file; `-` reads the syntax from standard input |
| `-e`, `--execute TEXT` | syntax given on the command line instead of a file |
| `-s`, `--session NAME` | load the dataset from this session before the run and save it after; `gui` is the open window |
| `--lines FIRST-LAST` | run only the commands inside these lines (1-based, inclusive), for example `40-55` |
| `--json` | print the result as JSON instead of Markdown |
| `--vega-lite` | also give every chart as a Vega-Lite specification (see [Charts](#charts)) |
| `--encoding NAME` | encoding of the syntax file or standard input, such as `windows-1257` or `1252`; without it a file that is valid UTF-8 (with or without a byte order mark) is read as UTF-8 and any other file in the system's legacy code page |
| `--xlsx FILE` | also write every table and chart of the run to an Excel workbook, one sheet per table |
| `--xlsx-one-sheet` | with `--xlsx`, put all tables on one sheet, one below the other |

Without `--json`, every table is printed as a Markdown table under a `###` title, every chart as a table of its numbers whose caption names the kind of chart, warnings as `warning: line N: COMMAND: message`, then a summary: the dataset loaded, variables added, changed and removed, the case count when it changed, and `ran N command(s)`, with `, session saved` when the session was written.

```
statscraft run job.sps
statscraft run -s demo -e "FREQUENCIES VARIABLES=gender."
echo "DESCRIPTIVES VARIABLES=age monthly_spend." | statscraft run -s demo -
```

A run is all or nothing. When any command fails, nothing is saved: not the session, not the workbook. Files that `SAVE` wrote before the failing command stay on disk.

## check

```
statscraft check [FILE | - | -e TEXT] [-s SESSION] [--lines FIRST-LAST] [--json] [--vega-lite] [--encoding NAME]
```

The same flags as `run`, without `--xlsx`. `check` runs the commands on a copy of the session's dataset and skips every file write, so it finds both syntax errors and errors that depend on the data (an unknown variable, a string where a number is needed). The session and the disk stay unchanged. The summary says `checked N command(s)`, and in JSON `saved` is always false.

```
statscraft check -s demo -e "COMPUTE age10 = age / 10."
```

```
variables added: age10
checked 1 command(s)
```

Every syntax error of the source is reported at once, one per bad command. Errors found while running stop at the first failing command.

## help

```
statscraft help                     the supported commands with one line each, then the general rules
statscraft help FREQUENCIES         syntax, notes and an example of one command
statscraft help freq                names may be lowercase and shortened, as in syntax
```

A command of the language that Statscraft does not run yet (for example `statscraft help GRAPH`) gets "is a command of the language, but it is not supported yet". An unknown name is an error. The same text, one page per command, is in [commands](commands/README.md).

## Sessions and line ranges

A session is what one window would keep from one run to the next, stored on disk in `.statscraft/sessions/<name>/` under the current folder. A name may hold letters, digits, `-` and `_`.

| What | Kept | Where in the session folder |
|---|---|---|
| The active dataset: data, dictionary, weight, file label, documents, attributes, multiple response sets | yes | `data.zsav` |
| SPLIT FILE and FILTER of the active dataset | yes | `state.json` |
| Where the random number generators stand (SAMPLE, UNIFORM, NORMAL, RV functions), so the next run goes on where the last one stopped | yes | `state.json` |
| Named datasets (DATASET NAME, COPY, DECLARE) and which one is active, each with its split and filter | yes | `datasets/` |
| Macros (DEFINE) and the macro settings MEXPAND, MPRINT, MNEST, MITERATE | yes | `macros.sps` |
| File handles (FILE HANDLE), SET values, PRESERVE levels not yet restored, the CD folder | yes | `settings.sps`, as FILE HANDLE, SET, PRESERVE and CD commands |
| TEMPORARY, N OF CASES, scratch variables, VECTOR | no: the end of a run is a data pass, so they take effect or end in the run that gives them | |
| An open DO IF, LOOP, or a DATA LIST without its BEGIN DATA | no: the run fails | |
| The folder of the syntax file, where INSERT and INCLUDE start | no: each run starts from its own file | |

- The first run of a new session starts without data; its syntax usually begins with `GET FILE`. Settings, macros and a CD may come before any data.
- With `--lines`, only the macros defined on those lines are kept; a macro defined elsewhere in the file still serves the run, as before.
- A run saves the session when it changed anything in the table above, or when the session file did not exist yet. A failed run keeps none of its changes. A dataset keeps the encoding of the file it came from; when a label or value no longer fits that code page (a letter of another language), the session keeps it in UTF-8 instead.
- `describe -s NAME` shows the active dataset and, under `session` in the JSON, the SET values given, the number of PRESERVE levels, the CD folder, the active dataset's name, every named dataset with its size, the macros, the output documents and the designated one ([describe](#describe)).
- `check` and `describe` never write it.
- Delete the session's folder to start over.

```
statscraft run -s demo --lines 1-1 job.sps
statscraft run -s demo --lines 4-6 job.sps
```

`--lines` selects the commands that lie inside the range. Blank lines and comments inside it are fine, and a syntax error on a line outside the range does not stop it, but the range must cover whole commands. A range that cuts through one fails:

```
error: line 1, column 1: line range 2-3 cuts through command FREQUENCIES, which occupies lines 1-2; hint: use lines 1-3 to include the whole command
```

Line numbers in the output always refer to the whole file, not to the range.

## JSON output

`run`, `check` and `describe` print pretty JSON with `--json`. `plan` and `db` always print one line of JSON.

### Run result

`statscraft run -s demo --json -e "COMPUTE spend_year = monthly_spend * 12.` followed by a second line `FREQUENCIES VARIABLES=gender."` prints (the table rows are shortened here):

```json
{
  "ok": true,
  "session": "demo",
  "commands": [
    {
      "name": "COMPUTE",
      "first_line": 1,
      "last_line": 1,
      "tables": [],
      "warnings": []
    },
    {
      "name": "FREQUENCIES",
      "first_line": 2,
      "last_line": 2,
      "tables": [
        {
          "title": "Gender",
          "row_headers": 3,
          "columns": ["Category", "Value", "Label", "Frequency", "Percent", "Valid Percent", "Cumulative Percent"],
          "column_groups": [[], [], [], [], [], [], []],
          "rows": [
            ["valid", 1.0, "Male", 106.0, 53.0, 53.0, 53.0],
            ["total", null, null, 200.0, 100.0, null, null]
          ]
        }
      ],
      "warnings": []
    }
  ],
  "changes": {
    "cases_before": 200,
    "cases_after": 200,
    "variables_added": ["spend_year"],
    "variables_removed": [],
    "variables_changed": []
  },
  "dataset": [200, 21],
  "saved": true
}
```

| Field | Meaning |
|---|---|
| `ok` | true |
| `session` | the session name; absent without `-s` |
| `commands` | one entry per command that ran, in order, with its first and last line in the source |
| `commands[].tables` | the tables of that command |
| `commands[].charts` | the charts of that command (see [Charts](#charts)); absent when it made none |
| `commands[].warnings` | what the command did differently from what was asked, without failing, for example `"no statistics for string variables: plan"` |
| `changes` | the case count before and after (`null` when there was no dataset), and the names of variables added, removed and changed. A variable counts as changed when its dictionary entry or any value differs |
| `dataset` | `[cases, variables]` of the active dataset after the run; absent when there is none |
| `saved` | true when the session file was written (for `gui`: when the run changed the window's session). Always false without a session and for `check` |

A table:

| Field | Meaning |
|---|---|
| `title` | the table title |
| `row_headers` | how many leading columns are row labels |
| `columns` | the innermost header of each column |
| `column_groups` | for each column, the header levels above it, outermost first; empty lists for a one-level header |
| `rows` | the cells: a number, a text or `null` for an empty cell. Numbers are raw, not rounded |
| `formats` | optional, in the shape of `rows`: the display format of each cell, such as `"PCT5.1"`, or `""` |
| `caption` | optional text under the table |

A custom table with two header levels, from `CTABLES /TABLE region [COUNT, COLPCT.COUNT PCT5.1] BY gender.` (first row only, shortened):

```json
{
  "title": "Custom Tables",
  "row_headers": 2,
  "columns": ["", "", "Count", "Column %", "Count", "Column %", "Count", "Column %"],
  "column_groups": [["", ""], ["", ""], ["Gender", "Male"], ["Gender", "Male"], ["Gender", "Female"], ...],
  "rows": [["Region of residence", "North", 23.0, 21.69811320754717, 11.0, 12.5, 0.0, 0.0], ...],
  "formats": [["", "", "F40.0", "PCT5.1", "F40.0", "PCT5.1", "F40.0", "PCT5.1"], ...]
}
```

### Charts

FREQUENCIES (BARCHART, PIECHART, HISTOGRAM), CROSSTABS (BARCHART), EXAMINE (PLOT), ONEWAY (PLOT MEANS), REGRESSION (RESIDUALS, SCATTERPLOT, PARTIALPLOT), FACTOR (PLOT), CLUSTER (PLOT) and TREE (the tree diagram) make charts. A chart is data: its kind, its labels and its numbers, with nothing about colors or layout. Labels follow SET TVARS, TNUMBERS and OVARS as tables do. `statscraft run --json -e "..."` with `CROSSTABS /TABLES=region BY gender /BARCHART.` gives (shortened):

```json
{
  "kind": "bar",
  "tier": "presentation",
  "orientation": "vertical",
  "grouping": "clustered",
  "title": "Region of residence by Gender",
  "category_axis": { "title": "Region of residence", "format": "F1.0", "variable": "region" },
  "value_axis": { "title": "Count", "format": "F8.0" },
  "legend": "Gender",
  "categories": [
    { "label": "North", "value": 1.0 },
    { "label": "South", "value": 2.0 }
  ],
  "series": [
    { "name": "Male", "statistic": "count", "variable": "gender", "value": 1.0, "values": [23.0, 23.0] },
    { "name": "Female", "statistic": "count", "variable": "gender", "value": 2.0, "values": [11.0, 23.0] }
  ]
}
```

| Field | Meaning |
|---|---|
| `kind` | `bar`, `pie`, `line`, `area`, `scatter`, `histogram`, `boxplot`, `stem_leaf`, `normal_qq`, `detrended_normal_qq`, `normal_pp`, `spread_level`, `dendrogram`, `icicle` or `tree_diagram` |
| `tier` | `presentation` (a chart the office programs can hold as an editable chart) or `statistical` (a plot drawn exactly, such as the histogram) |
| `orientation` | `vertical` (categories along the bottom) or `horizontal`; absent for a pie |
| `grouping` | `clustered`, `stacked` or `percent` (stacked to 100%); bars, lines and areas only |
| `title`, `footnote` | the title, as a table of the same command would have it, and an optional note such as the mean, standard deviation and N of a histogram |
| `category_axis`, `value_axis` | each with its `title`, the display `format` of its numbers, `maximum` when the syntax set the top of the axis, and the `variable` it shows |
| `legend` | what tells the series apart, such as the column variable; absent with one series |
| `categories` | in drawing order: the `label`, the `value` it stands for (`null` for a slice of all missing values), `missing` when it holds missing values, and `bin` (`[from, to]`) for a histogram bin, which holds its lower bound but not its upper one |
| `series` | each with its `name`, the `statistic` its numbers are (`count`, weighted when a weight is set, `percent`, `mean` or `value`), the `variable` and `value` it stands for, and `values`, one number per category |
| `normal` | a histogram's normal curve: `mean`, `std_dev` and `total`; its height at x is total times the bin width times the normal density |
| `plot` | the data of a statistical plot other than the histogram, by its `shape`; `categories` and `series` are then empty, the category axis is the horizontal one and the value axis the vertical one |

The shapes of `plot`:

- `boxes`: `boxes`, each with its `label`, `group` (the cluster, when boxes are grouped), `n`, `lower_whisker`, `lower_hinge`, `median`, `upper_hinge`, `upper_whisker`, and the marked cases in `outliers` and `extremes` (each a `value` and its `case` label).
- `stem_leaf`: `stem_width`, `each_leaf` (cases per leaf), `lines` (each a `frequency`, the `stem` as shown and the `leaves` as digits) and `low_extremes`, `high_extremes` (a `frequency` and the `bound`).
- `points`: `groups`, each a `name` and `points` (`x`, `y`, an optional `z` and `label`); `lines`, straight reference lines `y = intercept + slope x`; and `z_axis` when the points have depth. Q-Q, P-P, spread-versus-level, scatter, partial regression and loading plots use it.
- `merges`: `items` (the case labels), `order` (the layout of the items, so that no branches cross), `merges` (each joins cluster `right` into cluster `left`, both named by an item index, at `height`, with `rescaled` from 1 to 25) and, for an icicle plot, `solutions`, the numbers of clusters it shows.
- `tree`: `nodes`, each with its `id`, `parent`, the `split` condition that leads to it, and `statistics` as name and value pairs.

With `--vega-lite` each chart also has a member `vega_lite`: a [Vega-Lite](https://vega.github.io/vega-lite/) v5 specification with the numbers inline, which any Vega-Lite viewer draws as it is. In Markdown output the specification follows the chart's data table as a `json` code block. SPLIT FILE gives a chart per split group, the group named in the title.

### Error

On any failure `--json` prints this on standard output, and the exit code is 1:

```json
{"ok":false,"errors":[{"kind":"syntax","message":"unexpected end of command","line":1,"column":18,"expected":["a value","a variable","a function","'('"]}],"saved":false}
```

| Field | Meaning |
|---|---|
| `kind` | `syntax` (the text cannot be parsed), `run` (a command failed on the data), `usage` (wrong flags or arguments), `session` or `file` (reading or writing failed) |
| `message` | what went wrong |
| `line`, `column` | where, 1-based. Syntax errors have both; run errors have the first line of the command |
| `command` | for run errors, the command that failed |
| `expected` | for syntax errors, what would have been valid at that point |
| `hint` | a suggestion, such as `"did you mean FREQUENCIES?"` |

More examples:

```json
{"ok":false,"errors":[{"kind":"run","message":"unknown variable agee","command":"FREQUENCIES","line":2}],"saved":false}
{"ok":false,"errors":[{"kind":"syntax","message":"unknown command FREQUENCIS","line":1,"column":1,"hint":"did you mean FREQUENCIES?"}],"saved":false}
```

Without `--json` the same errors go to standard error as `error: line 1, column 18: unexpected end of command; expected: a value, a variable, a function, '('`.

### Warnings

Warnings do not fail a run. They are in `commands[].warnings` of the run result, and in text output as:

```
warning: line 1: DESCRIPTIVES: no statistics for string variables: plan
```

### Dictionary

`statscraft describe tests/fixtures/survey_basic_phpwriter.sav --json` (shortened):

```json
{
  "encoding": "UTF-8",
  "compression": "bytecode",
  "case_count": 200,
  "variable_count": 19,
  "file_label": "Statscraft synthetic customer survey",
  "weight_variable": "weight",
  "multiple_response_sets": [
    {"name": "$q5", "type": "dichotomy", "label": "Q5. Contact channels used", "counted_value": 1,
     "variables": ["q5_1", "q5_2", "q5_3", "q5_4", "q5_5"]},
    {"name": "$q6", "type": "category", "label": "Q6. Other providers mentioned", "counted_value": null,
     "variables": ["q6_mention1", "q6_mention2", "q6_mention3"]}
  ],
  "variables": [
    {
      "name": "q2_recommend",
      "type": "numeric",
      "label": "Q2. How likely are you to recommend us to a friend or colleague?",
      "format": "F2.0",
      "measure": "scale",
      "value_labels": {"0": "Not at all likely", "10": "Extremely likely", "97": "Not applicable", "98": "Don't know", "99": "Refused"},
      "missing_values": [],
      "missing_range": [97.0, 99.0],
      "n_valid": 176,
      "n_system_missing": 0,
      "n_user_missing": 24
    },
    {
      "name": "plan",
      "type": "string",
      "width": 10,
      "label": "Current subscription plan",
      "format": "A10",
      "measure": "nominal",
      "value_labels": {},
      "missing_values": [],
      "missing_range": null,
      "n_valid": 200,
      "n_system_missing": 0,
      "n_user_missing": 0
    }
  ]
}
```

| Field | Meaning |
|---|---|
| `encoding`, `compression` | of the file (`none`, `bytecode` or `zlib`) |
| `file_label` | absent when the file has none |
| `weight_variable` | the weight variable, or `null` |
| `multiple_response_sets` | absent when there are none |
| `variables[].type` | `numeric` or `string`; strings also have `width` in bytes |
| `variables[].measure` | `nominal`, `ordinal`, `scale` or `unknown` |
| `variables[].value_labels` | value (as text) to label |
| `variables[].missing_values`, `missing_range` | the user-missing values: up to three values, or a `[low, high]` range |
| `n_valid`, `n_system_missing`, `n_user_missing` | counts over all cases |

## Exit codes

| Code | When |
|---|---|
| 0 | success, including runs with warnings |
| 1 | the syntax, the run, a file or a session failed; also `plan` and `db` when they print `"ok":false` |
| 2 | the command line itself is wrong: an unknown flag, a missing value, flags that conflict |
| 10 | `update --check`: a newer release is published |

## update

```
statscraft update --check          current and latest version; exit code 10 when an update exists
statscraft update --check --json   the same as JSON
statscraft update                  update this installation to the latest release
```

The latest release is the newest published release on the GitHub repository; drafts and prereleases do not count. `update --check` prints the running and the latest version and exits with 0 when this copy is up to date (also when no release is published yet) and with 10 when a newer one exists, so a script can ask `statscraft update --check || ...`. With `--json` it prints one line:

```
{"current":"0.1.0","latest":"0.2.0","update":true,"notes":"https://github.com/statscrafthq/statscraft/releases/tag/v0.2.0"}
```

`latest` is null when no release is published yet, and `notes` is the release page when there is an update.

`update` does what the window's Update now does, for the way this copy was installed:

| Installed with | What `update` does |
|---|---|
| the Windows installer | downloads the new installer and starts it silently for the same per-user installation; it closes an open window first |
| the portable Windows .zip, with `LICENSE.txt` next to the programs | downloads the .zip and replaces `statscraft.exe` and `statscraft-gui.exe` in that folder; the old ones are renamed aside and deleted on a later start |
| the tarball, under a prefix such as `~/.local` | downloads the tarball and replaces `statscraft` and `statscraft-gui` in its `bin` folder |
| the .deb | downloads the package into `~/Downloads` and prints the one command that installs it, `sudo apt install ~/Downloads/statscraft_X_amd64.deb`; it never runs sudo itself |
| anything else (a build from source, a copy elsewhere, the command line from the macOS package) | refuses with exit code 1, says to update it the way it was installed and gives the release page |

Every download is checked against the `SHA256SUMS` file of the release before anything is run or replaced; on a mismatch nothing changes and the file is deleted. The installer, the tarball and the .zip go to a temporary folder that is removed after use. Errors print as for the other subcommands, exit code 1.

The only requests go to GitHub over HTTPS: one to the API for the latest release and, on update, the download of `SHA256SUMS` and of the one package. They carry the User-Agent `statscraft/VERSION` and nothing else about you; there is no account, token or telemetry.

## connect

`statscraft connect` adds an entry that starts `statscraft mcp` to every AI agent installed on this computer and says what to restart. `statscraft connect AGENT` connects one, `--list` only shows the state, `--remove [AGENT]` takes the entry out again, and `--json` prints the result as JSON. [Connect your AI agent](connect.md) lists the agents, the files it changes and how to make the same change by hand.

## Excel export

```
statscraft run job.sps --xlsx tables.xlsx
statscraft run job.sps --xlsx all.xlsx --xlsx-one-sheet
statscraft run -s gui -e "FREQUENCIES VARIABLES=gender." --xlsx gender.xlsx
```

Every table and chart of the run goes into the workbook, with or without a session, and also from the window session. With one sheet per table, each sheet is named after its table title (`Gender`, `Region of residence`, ...). With `--xlsx-one-sheet` there is one sheet `Tables` with the tables one below the other.

Each table has its title in bold, the column headers with merged cells for every level, the row labels, thin borders and its caption below. Numbers stay numbers, with a number format that shows them as the output does: a percentage cell holds 45.2 and shows 45.2%. Text such as significance letters and hidden small counts (`<5`) stays text. A bar, pie, line or area chart goes in as its data table with an editable chart beside it, drawn from those cells: change a number and the chart follows. The chart sets no colors, so it takes the workbook's theme. A histogram goes in as its data table only. The text output on the terminal is the same as without `--xlsx`.

OUTPUT EXPORT in the syntax writes the same charts into a workbook (/XLSX), a Word document (/DOC) or a presentation (/PPT); see `statscraft help "OUTPUT EXPORT"`.

A workbook that cannot be written fails the run, and the session is then not saved.

## The live window session

While `statscraft-gui` is open, the session name `gui` means the window:

```
statscraft run -s gui -e "GET FILE='tests/fixtures/survey_basic.sav'."
statscraft run -s gui --json job.sps
statscraft check -s gui -e "COMPUTE x = age / 10."
statscraft describe -s gui --json
```

- The window runs the syntax as its own Run button does: the tables appear in the designated output window, the commands in the log, and the data windows show the new data.
- The command line prints what it prints for a file session, with `"session": "gui"`. `saved` is true when the run changed the window's session (the table under [Sessions](#sessions-and-line-ranges)); nothing is written to disk unless the syntax says `SAVE`.
- The window keeps the same state between runs as a file session, in memory until it closes; `describe -s gui` shows it the same way.
- Relative paths in the syntax are read from the folder where `statscraft` was started, and the window stays in that folder.
- One run goes at a time. A request waits while the window is busy.
- When no window is open, `-s gui` fails with "no Statscraft window is running". With several windows open, the one started last answers.

How it connects, and why it is safe: see [AI agents](ai-agents.md#safety-of-the-live-session).

## mcp

`statscraft mcp` serves describe, help, check and run as MCP tools over standard input and output, for agent apps that start local MCP servers. It runs until the app closes the connection. [The MCP server](mcp.md) lists the tools and says where their sessions live.

## plan

```
statscraft plan [FILE | - | -e TEXT] (--input-schema FILE | -s SESSION | --sqlite DB --name DATASET [--apply [--actor NAME]])
                [--profile 0.1|0.2|0.3] [--alias NAME]
```

`plan` turns a bounded part of the language into an OpenStatSpec Transformation Plan: one line of canonical JSON with the plan, its SHA-256 `plan_hash` and the `source_hash` of the syntax, or an error with a stable `code`, `line` and `column`.

| Flag | Meaning |
|---|---|
| `--input-schema FILE` | JSON of the form `{"variables": [{"name": "q1", "storage_kind": "numeric"}]}` |
| `-s`, `--session NAME` | take the input schema from a session's dataset |
| `--sqlite DB --name DATASET` | take it from a dataset in an OpenStatSpec SQLite database |
| `--apply` | with `--sqlite`, apply the plan to that dataset in place, in one transaction with one audit row |
| `--actor NAME` | who applies the plan, for the audit row; default `$USER` |
| `--profile` | the frontend profile, `0.3` by default |
| `--alias` | the name of the input dataset in the plan, `input` by default |

Profile 0.1 accepts RECODE, VARIABLE LABELS and VALUE LABELS; 0.2 adds COMPUTE and IF with single values and simple comparisons, FORMATS, VARIABLE LEVEL and EXECUTE; 0.3 adds comments, TO lists, NE and NOT, LOWEST and HIGHEST ranges and ADD VALUE LABELS. Everything else is rejected, even where `run` accepts it. With `schema.json` holding `{"variables": [{"name": "q1", "storage_kind": "numeric"}, {"name": "q2", "storage_kind": "numeric"}]}`:

```
statscraft plan -e "RECODE q1 (1 = 0)." --input-schema schema.json --profile 0.1
statscraft plan -s demo -e "RECODE q1_satisfaction (98, 99 = SYSMIS)."
statscraft plan -e "FREQUENCIES q1." --input-schema schema.json
```

The last one prints `{"error":{...,"message":"FREQUENCIES is not supported by frontend profile 0.3",...},"ok":false,...}`, whose `code` is the profile's code for an unsupported command, and exits with 1.

## db

```
statscraft db export survey.sav --sqlite study.db [--name survey]   store a dataset in SQLite
statscraft db export survey.sav --postgres URL                      or in PostgreSQL
statscraft db export survey.sav --mysql URL                         or in MySQL or MariaDB
statscraft db export survey.sav --dolt URL                          or in Dolt
statscraft db export -s work --sqlite study.db --name work          store a session's dataset
statscraft db import --sqlite study.db --name survey --out back.sav read it back into a file
statscraft db import --sqlite study.db --name survey -s work        or into a session
statscraft db capabilities                                          versions and limits per database
statscraft plan job.sps --sqlite study.db --name survey --apply     apply syntax to it in place
```

`db` stores a dataset the way OpenStatSpec's SAV/ZSAV profile 1.0 and its SQL dialect profiles specify.

- One table `data_<name>` holds the cases: `__case_ordinal` (1, 2, 3, ... in case order) and one column per variable, in variable order.
- Numbers are binary64, with system-missing as NULL. Strings are text that is never NULL. User-missing codes are kept as values, and dates as numbers.
- The dictionary goes into the catalog tables: labels, value labels, missing values, formats, measurement level, role, display settings, attributes, documents, weight variable and multiple response sets.
- Variable names that are not plain ASCII get generated column names (`v<position>`); the catalog maps them back.
- The database belongs to OpenStatSpec. `export` sets up the catalog in a new or empty one and refuses one that holds anything else.

| Flag | Meaning |
|---|---|
| `--sqlite FILE`, `--postgres URL`, `--mysql URL`, `--dolt URL` | the database; see [Databases](#databases) |
| `--name` | the dataset name: ASCII letters, digits and `_`. For `export` it defaults to the file or session name |
| `--out FILE` | `import` writes this .sav or .zsav file |
| `-s`, `--session NAME` | `export` reads, or `import` replaces, this session's dataset |
| `--allow-loss` | `import` goes on when the catalog holds items a dataset cannot keep |

Each prints one line of JSON, such as `{"case_count":200,"dataset":"survey_basic","dataset_id":"...","ok":true,"table":"data_survey_basic","variable_count":19}`, or `{"ok":false,"error":{"code":...,"message":...}}` with exit code 1.

```
statscraft db export tests/fixtures/survey_basic.sav --sqlite study.db
statscraft db export -s demo --sqlite study.db --name work
statscraft plan -e "RECODE q1_satisfaction (98, 99 = SYSMIS)." --sqlite study.db --name survey_basic --apply --actor demo
statscraft db import --sqlite study.db --name survey_basic --out back.sav
statscraft db import --sqlite study.db --name work -s fromdb
```

### Writing and reading

- Before writing, a preflight checks the column limit, identifiers, value sizes and the row size against the profile and the server's settings. SQLite refuses NaN and negative zero, which it cannot keep.
- A failure leaves no table and no dataset row, only the failed operation and its diagnostic.
- The rows go in bulk inside the same transaction:
  - PostgreSQL takes them with `COPY` in the binary format;
  - MySQL and MariaDB in prepared multi-row INSERTs of at most 4 MiB, each within `max_allowed_packet`;
  - Dolt in multi-row INSERTs of literal values of at most 256 KiB, each within `max_allowed_packet`;
  - SQLite through one prepared INSERT per case.
- Reading streams the rows from the server into the dataset instead of holding the whole result first.
- Reading a dataset whose catalog holds items Statscraft cannot keep (dataset attributes, variable sets, write formats other than the print format) fails unless `--allow-loss` is given. The losses are then listed in the output.

### Databases

| | SQLite | PostgreSQL | MySQL, MariaDB | Dolt |
|---|---|---|---|---|
| Option | `--sqlite FILE` | `--postgres postgres://user@host:port/db[?schema=NAME]` | `--mysql mysql://user@host:port/db` | `--dolt mysql://user@host:port/db` |
| The catalog lives in | the whole file | schema NAME (default `openstatspec`) | database `db` | database `db`, on the session's branch |
| Server versions | bundled SQLite | 17.x, 18.x, UTF8 encoding | MySQL 8.4.x, 9.7.x; MariaDB 11.4.x, 11.8.x, 12.3.x; 16 KiB InnoDB pages | reads 2.2.x from 2.2.2 on; writes 2.2.2 and 2.2.3 |
| Columns, `__case_ordinal` included | 2,000 | 1,600 | 1,017 | 4,097 |
| Numbers | REAL; NaN and negative zero refused | DOUBLE PRECISION, every bit pattern kept | DOUBLE; NaN, infinities and negative zero refused | DOUBLE; NaN, infinities and negative zero refused |
| Text | TEXT NOT NULL | TEXT NOT NULL; U+0000 refused | TEXT NOT NULL, binary collation, at most 65,535 bytes | LONGTEXT NOT NULL |
| Row | the length limit | a heap tuple of 8,160 bytes (NULLs are free) | the InnoDB record of 8,126 bytes: 8 per number, 41 (MySQL) or 21 (MariaDB) per string; one INSERT within `max_allowed_packet` | a table definition below 65,504 bytes: 8 per number, 20 per string |
| Identifiers | 255 bytes | `max_identifier_length` (63 bytes) | 64 characters | 64 characters |
| New variables in `plan --apply` | yes, in the same transaction | yes, in the same transaction | no: `schema_change_not_atomic` before any change | no: `schema_change_not_atomic` |

MySQL, MariaDB and Dolt commit DDL implicitly. So `export` creates the table first, writes the rows and the catalog in one transaction, and drops the table again if that fails. A plan that would create a variable is refused before anything changes; create the column and its catalog variable in a separate step first.

The product is decided by what the server says about itself, not by the protocol: `--mysql` refuses a Dolt server, and `--dolt` refuses MySQL and MariaDB.

### Dolt

- Statscraft never commits, branches, merges or resets.
- `db export` leaves its changes in the working set for you to commit.
- `db import` and the input schema of `plan` write nothing at all (OpenStatSpec's Database I/O Execution Policy v1, which `db capabilities` declares). Allowed losses are therefore reported in the output instead of in the catalog.
- `plan --apply` needs `--expected-branch` and `--expected-head`. The session must be on that branch at that commit with a clean working set, and the branch must still point at that commit when the apply completes. The audit row records both, and the changes stay uncommitted.
- The session's branch is the database's default branch, or the one named as `db/branch` in the URL.

### Passwords and TLS

A URL without a password takes it from the `STATSCRAFT_DB_PASSWORD` environment variable. Passwords are never printed.

Connections use plain TCP unless the URL asks for TLS. TLS always verifies the server's certificate chain, against the CA file when one is given and otherwise against the bundled Mozilla root certificates. It also checks the host name unless the mode says otherwise. Statscraft never makes an unverified TLS connection, and refuses the modes that fall back to plain TCP (`prefer`, `allow`, `PREFERRED`).

```
statscraft db export survey.sav --postgres "postgres://analyst@db.example.org/study?sslmode=verify-full"
statscraft db export survey.sav --postgres "postgres://analyst@10.0.0.5/study?sslmode=verify-ca&sslrootcert=/etc/ssl/study-ca.pem"
statscraft db export survey.sav --mysql "mysql://analyst@db.example.org/study?ssl-mode=VERIFY_IDENTITY&ssl-ca=/etc/ssl/study-ca.pem"
```

| PostgreSQL `sslmode` | MySQL, MariaDB, Dolt `ssl-mode` | Connection |
|---|---|---|
| not given, `disable` | not given, `DISABLED` | plain TCP |
| `verify-full`, `require` | `VERIFY_IDENTITY`, `REQUIRED` | TLS; the certificate chain and the host name are checked |
| `verify-ca` | `VERIFY_CA` | TLS; the certificate chain is checked, the host name is not |

- `sslrootcert=FILE` (PostgreSQL) or `ssl-ca=FILE` (MySQL, MariaDB, Dolt) names a PEM file of the CA certificates to trust, for a server whose certificate comes from a private CA.
- A CA file without a mode means `verify-full`.
- With a CA file, PostgreSQL trusts only that file; the MySQL client trusts the bundled roots as well.
- Note that `require` and `REQUIRED` check more than in the PostgreSQL and MySQL command line clients, where they encrypt without checking the certificate.

### Applying a plan in place

`plan --name DATASET` with a database option takes the input schema from that dataset. With `--apply` it then applies the plan to the same table in one transaction: direct UPDATEs, `ALTER TABLE ... ADD COLUMN` for new variables, catalog updates and one audit row in `transformation_apply` (plan and source hashes, actor, operation count). Nothing is copied, and on any error nothing changes. `--actor` names who applies the plan (default `$USER`).
