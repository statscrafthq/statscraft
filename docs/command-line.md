# The command line

```
statscraft describe   dictionary and counts of a data file or a session
statscraft run        run syntax
statscraft check      validate syntax without saving or writing anything
statscraft chart      add a chart given as JSON to the output
statscraft export     export the output of the open window, as its File > Export
statscraft output     show what an output window of the open window holds
statscraft undo       take back the last change to the data in the open window, as its Edit > Undo
statscraft help       syntax of a command, or the list of commands
statscraft plan       lower syntax to an OpenStatSpec transformation plan
statscraft db         store datasets in an OpenStatSpec database and read them back
statscraft update     check for a newer release, or update this installation to it
statscraft mcp        serve these tools to an AI agent over MCP
statscraft connect    add the Statscraft MCP server to the AI agents on this computer
```

`statscraft --version` prints the version, and `statscraft <subcommand> --help` lists the flags of a subcommand. The examples use the [sample file](getting-started.md#open-the-sample-file) `survey.sav` or a data file of your own, and `job.sps` stands for a syntax file of your own. `plan` and `db` have a page of their own: [Databases](databases.md).

## describe

```
statscraft describe FILE [--json]
statscraft describe -s SESSION [--json]
```

| Flag | Meaning |
|---|---|
| `FILE` | a .sav or .zsav file |
| `-s`, `--session NAME` | describe a session instead: its dataset and the state it keeps; `gui` is the open window |
| `--json` | the full dictionary as JSON instead of one line per variable, with the data file attributes under `attributes` (each name with its list of values) when there are any |

Give either a file or a session. The text form prints `cases, variables` and then, per variable, name, format, measurement level, valid count and label, separated by tabs. For a session it goes on with what the session keeps besides the data, one line each when there is something: `settings:`, `PRESERVE levels:`, `directory:` (where relative file names start: the folder of the last CD, else, for the window, its last caller's folder), `active dataset:`, `datasets:`, `macros:`, `output documents:`, `designated output:`. In JSON these are the object `session` (see [Sessions](#sessions-and-line-ranges)).

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

The open window (`-s gui`) lists what the window holds, the same way. A file session keeps its output documents between runs the same way, so there the list shows what earlier runs collected.

## run

```
statscraft run [FILE | - | -e TEXT] [-s SESSION] [--lines FIRST-LAST] [--json] [--vega-lite] [--encoding NAME] [--xlsx FILE [--xlsx-one-sheet] [--xlsx-no-titles] [--plugin PROGRAM]... [--plugin-template FILE]...] [--charts DIR [--chart-format svg|png]]
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
| `--xlsx-no-titles` | with `--xlsx`, leave out the title row above each table and chart; variable labels, captions and what plugins add stay |
| `--plugin PROGRAM` | with `--xlsx`, let a plugin color, comment and lay out the custom tables (see [plugins](plugins.md)); its settings come from the active data's attributes, with `-s gui` those of the window's active data. Give it more than once for several plugins: they run in that order and their answers merge, the later one winning where two style the same thing |
| `--plugin-template FILE` | with `--plugin`, lay a design template file, made by Export in the window's Plugin Settings ([plugins](plugins.md)), over the data's attributes for this run, as an active template does in the window. One file per plugin, each holding exactly one template of one of the `--plugin` programs |
| `--charts DIR` | also write every chart of the run as a picture into this folder (see [Chart pictures](#chart-pictures)) |
| `--chart-format svg\|png` | with `--charts`, the picture format; default `svg` |

Without `--json`, every table is printed as a Markdown table under a `###` title, every chart as a table of its numbers whose caption names the kind of chart, warnings as `warning: line N: COMMAND: message` (the command named once: a message that names it itself, also with a subcommand as in `FACTOR /SAVE:`, keeps that name, and the name of a split group comes after it), then a summary: the dataset loaded, variables added, changed and removed, the case count when it changed, and `ran N command(s)`, with `, session saved` when the session was written.

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

An unknown name is an error. The same text, one page per command, is in [commands](commands/README.md).

## Sessions and line ranges

A session keeps your work between runs, as the window does while it is open. Give it a name with `-s`, and each run builds on the one before:

```
statscraft run -s demo -e "GET FILE='survey.sav'."
statscraft run -s demo -e "RECODE age (LOWEST THRU 39 = 1) (40 THRU HIGHEST = 2) INTO young_old."
statscraft run -s demo -e "FREQUENCIES VARIABLES=young_old."
```

A session keeps:

- the active dataset with its weight, split and filter, and every named dataset;
- the macros (DEFINE, or a macro library brought in with INCLUDE) and the macro settings of SET;
- file handles, SET values, PRESERVE levels and the folder of the last CD;
- where the random numbers stand, so the next run goes on where the last one stopped;
- the output documents and their tables;
- the syntax of every run that worked, in the file `syntax.sps`: a record of how the session got where it is. Run it from the top on the original data to repeat the work.

TEMPORARY, N OF CASES, scratch variables and VECTOR end with the run that gives them. An open DO IF or LOOP fails the run.

- The first run of a new session starts without data, so it usually begins with `GET FILE`.
- A run is all or nothing: when one command fails, the session stays as it was. Fix the line and run it again.
- `describe -s NAME` shows the active dataset and what the session keeps (see [describe](#describe)).
- Sessions live in `.statscraft/sessions/<name>/` under the current folder. Delete that folder to start over.
- Tables pile up in the session's output from run to run. `OUTPUT CLOSE ALL.` empties it. `--xlsx` writes only the tables of its own run; to write the tables of several runs at once, end with `OUTPUT EXPORT` (see [Excel export](#excel-export)).

`--lines` runs a part of a file, so you can work through a long script piece by piece:

```
statscraft run -s demo --lines 1-1 job.sps
statscraft run -s demo --lines 4-6 job.sps
```

The range must cover whole commands. Blank lines and comments inside it are fine, and a syntax error outside it does not stop it. A range that cuts through a command fails:

```
error: line 1, column 1: line range 2-3 cuts through command FREQUENCIES, which occupies lines 1-2; hint: use lines 1-3 to include the whole command
```

Line numbers in the output always refer to the whole file, not to the range.

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
statscraft update --check --plugin PROGRAM [--json]
                                   whether a plugin offers a newer version; installs nothing
statscraft update                  update this installation to the latest release
```

The latest release is the newest published release on the GitHub repository; drafts and prereleases do not count. `update --check` prints the running and the latest version and exits with 0 when this copy is up to date (also when no release is published yet) and with 10 when a newer one exists, so a script can ask `statscraft update --check || ...`. With `--json` it prints one line:

```
{"current":"0.1.0","latest":"0.2.0","update":true,"notes":"https://github.com/statscrafthq/statscraft/releases/tag/v0.2.0"}
```

`latest` is null when no release is published yet, and `notes` is the release page when there is an update.

`update --check --plugin PROGRAM` asks a [plugin](plugins.md#updates) instead, as the window's Check in Edit > Settings > Plugins does: it prints the plugin's name, its version and, when the plugin offers a newer one, that version and its notes; exit code 10 when there is an update, else 0. With `--json`: `{"plugin":"Acme Tables","current":"0.2.1","latest":"0.2.2","update":true,"notes":"Faster tables"}`. It never installs the plugin: `--plugin` only goes with `--check`, and Update in the window's plugin settings installs it. A plugin that does not offer updates, or cannot check, is an error with its reason.

`update` does what the window's **Update now** does, for the way this copy was installed: see [Updates](install.md#updates). On Debian and Ubuntu it only downloads the package and prints the command that installs it. A copy built from source, or the command line from the macOS package, is not updated: the command says how to update it instead, with exit code 1.

Every download is checked against the checksums published with the release before anything is run or replaced; on a mismatch nothing changes. The check sends nothing about you or your data. Errors print as for the other subcommands, exit code 1.

## connect

`statscraft connect` adds an entry that starts `statscraft mcp` to every AI agent installed on this computer and says what to restart. `statscraft connect AGENT` connects one, `--list` only shows the state, `--remove [AGENT]` takes the entry out again, and `--json` prints the result as JSON. [Connect your AI agent](connect.md) lists the agents, the files it changes and how to make the same change by hand.

## Excel export

```
statscraft run job.sps --xlsx tables.xlsx
statscraft run job.sps --xlsx all.xlsx --xlsx-one-sheet
statscraft run -s gui -e "FREQUENCIES VARIABLES=region." --xlsx region.xlsx
```

Every table and chart of the run goes into the workbook, one sheet per table, named after its title. `--xlsx-one-sheet` puts them all on one sheet `Tables`, one below the other. `--xlsx-no-titles` leaves out the title row above each table and chart, for example to stack custom tables on one sheet. The workbook looks as the window's export makes it: see [Exporting all output](window.md#exporting-all-output). The text output on the terminal is the same as without `--xlsx`.

A workbook that cannot be written fails the run, and the session is then not saved.

To collect the tables of several runs in one workbook, end the last run with OUTPUT EXPORT, which writes the whole output document of the session: `OUTPUT EXPORT /XLSX DOCUMENTFILE='all.xlsx'.` OUTPUT EXPORT also writes Word documents (/DOC), presentations (/PPT), web pages (/HTML) and picture files of the charts (see [Chart pictures](#chart-pictures)).

## Output files

OUTPUT SAVE writes an output document as a Statscraft output file (`.scout`), and OUTPUT OPEN reads one back as a new document. The window saves and opens the same files ([the window](window.md#saving-output)).

```
statscraft run -e "GET FILE='survey.sav'.
CROSSTABS region BY gender.
OUTPUT SAVE OUTFILE='region.scout'."
statscraft run -e "OUTPUT OPEN FILE='region.scout'.
OUTPUT EXPORT /XLSX DOCUMENTFILE='region.xlsx' /HTML DOCUMENTFILE='region.html'."
```

The second run converts the saved output without the data: OUTPUT OPEN makes the file's document the designated one, and OUTPUT EXPORT writes it. Tables of commands after OUTPUT OPEN join that document, so open, run and save again adds to the file.

A file may come from anyone, so OUTPUT OPEN checks every table and chart before it shows it. An item that fails the checks is left out with a warning that says why, and the rest of the file opens.

The file is plain JSON, one table row per line, so an agent or a script can read it directly, with no Statscraft call. Each command is an item with the syntax exactly as it ran, its warnings, and its tables and charts in the form of [the run result](#run-result). The [developer notes](https://github.com/statscrafthq/statscraft/blob/main/docs/developers/README.md#output-files) describe the layout.

## Chart pictures

```
statscraft run job.sps --charts charts
statscraft run job.sps --charts charts --chart-format png
```

Every chart of the run is written to the folder, which is made if it is missing, as `01-<title>.svg`, `02-<title>.svg` and so on: numbered in the order of the output, the title in lower case with hyphens between its words. An existing file of the same name is replaced. The pictures are 640 pixels wide on white paper (a PNG has twice the pixels, for sharp text), drawn as the window draws them: flat bars, lines, areas, dots and slices in petrol and amber, muted labels and thin rule lines, in Schibsted Grotesk with tabular figures. Drawn are bar charts in both orientations, side by side, stacked and stacked to 100%, pie, line, area and scatter charts, the scatterplot matrix, the histogram with its normal curve, boxplots, the point plots (normal Q-Q and P-P, detrended Q-Q, spread versus level, residual and loading plots) with their reference lines, the dendrogram, the icicle plot and the tree diagram. A stem-and-leaf display and a plot in three dimensions get no file; their data table is in the output. An SVG names the font and falls back to the system's sans-serif font where Schibsted Grotesk is not installed; a PNG has it built in.

The text output on the terminal is the same as without `--charts`. The charts are written after the run, so a folder that cannot be written fails the command but leaves the session as the run saved it.

The syntax writes pictures too. OUTPUT EXPORT with an image subcommand and no document saves every chart of the output document, numbered from 0 after IMAGEROOT:

```
OUTPUT EXPORT /PNG IMAGEROOT='charts/june'.
OUTPUT EXPORT /JPG IMAGEROOT='charts/june' PERCENTSIZE=50 GRAYSCALE=YES.
```

The first line gives `charts/june0.png`, `charts/june1.png` and so on; the folder must exist. PERCENTSIZE=100 is the PNG of `--charts`, 1280 pixels wide; 50 halves it. PNG, JPG, BMP and TIF are written. EMF and EPS give a warning and are not written. A web page from OUTPUT EXPORT /HTML holds its charts as pictures inside the page, and /TEXT saves them beside the text file, named after it.

## The live window session

While `statscraft-gui` is open, the session name `gui` means the window:

```
statscraft run -s gui -e "GET FILE='survey.sav'."
statscraft run -s gui --json job.sps
statscraft check -s gui -e "COMPUTE x = age / 10."
statscraft describe -s gui --json
```

- The window runs the syntax as its own Run button does: the tables appear in the designated output window, the commands in the log, and the data windows show the new data.
- The command line prints what it prints for a file session, with `"session": "gui"`. `saved` is true when the run changed what the window's session keeps (see [Sessions](#sessions-and-line-ranges)); nothing is written to disk unless the syntax says `SAVE`.
- The window keeps the same state between runs as a file session, in memory until it closes; `describe -s gui` shows it the same way.
- Relative paths in the syntax start from the folder where `statscraft` was started, unless a CD is in force in the window's session: a CD starts from that folder too and then stays until the next CD, as in a file session. INSERT and INCLUDE in a syntax file given to `run` start from the file's folder, as in a file session. A run leaves the caller's folder to the window's own later runs, and `describe` shows it as `directory`; a check uses it only for itself. A syntax window that an agent runs starts from its own file's folder, or from the agent's folder when it has no file. A folder that is gone makes relative files fail with their own error; nothing is read or written anywhere else.
- Unlike a file session, the window names files in its messages by their full path, since it resolves relative names against the caller's folder.
- One run goes at a time. A request waits while the window is busy.
- When no window is open, `-s gui` fails with "no Statscraft window is running". With several windows open, the one started last answers.

How it connects, and why it is safe: see [AI agents](ai-agents.md#safety-of-the-live-session).

## export

```
statscraft export FILE [--format xlsx|html|pdf] [--one-sheet] [--selected] [--hidden]
```

Writes the output of the open `statscraft-gui` window to a file, exactly as its File > Export does (see [the window](window.md#exporting-all-output)): the designated output window, with what its plugins put on the tables.

| Flag | Meaning |
|---|---|
| `FILE` | the file to write; a relative path starts in the current folder. An existing file is replaced |
| `--format xlsx\|html\|pdf` | an Excel workbook, a web page or a PDF document; without it the extension of `FILE` says (`.xlsx`, `.html` or `.htm`, `.pdf`) |
| `--one-sheet` | Excel: all tables on one sheet, one below the other, instead of one sheet per table |
| `--selected` | only the tables selected in the output window; fails when none is selected |
| `--hidden` | hidden items too; without it only what the output window shows |

```
statscraft run -s gui -e "FREQUENCIES region."
statscraft export tables.xlsx --one-sheet
```

```
output exported to /home/me/work/tables.xlsx
```

It fails when no window is open, when the window has no output, and when the file cannot be written. An export waits until the plugins have answered for the tables of the last run.

## output

```
statscraft output [--document NAME] [--json]
```

Shows what an output window of the open `statscraft-gui` window holds, as the person sees it there: every command heading and every table and chart of the session in order, also those from earlier runs and other callers. Without `--document` it reads the designated output window; `NAME` is an output document as `describe -s gui` lists it. It changes nothing in the window.

The first line sums up the document and says which kinds of element the window shows (Edit > Settings: command headings, titles, tables, notes, charts). Then each heading and table follows as `run` prints them, a chart as its data table; `(hidden)` marks a heading or table hidden in the output, `(selected)` a table selected in the output window.

```
statscraft run -s gui -e "FREQUENCIES region."
statscraft output
```

```
output document Document1: 1 commands, 1 tables, 0 charts, 0 hidden, 1 selected; the window shows titles, tables, notes, charts

## FREQUENCIES

### Region of residence (selected)
...
```

With `--json` it prints the window's answer: `document`, `shows` (`headings`, `titles`, `tables`, `notes`, `charts`, each true or false) and `commands`, each with `id`, `heading`, `hidden` and `items`; an item has `id`, `hidden`, `selected` and a `table` or a `chart` in the form `run --json` gives them. It fails when no window is open, when the window has no output window, and when no output document has that name.

## undo

```
statscraft undo
```

Takes back the last run that changed the data in the open `statscraft-gui` window, exactly as its Edit > Undo last change does (see [the window](window.md#syntax-is-the-only-command-layer)): the datasets are as they were before that run, the output stays, and the log marks the run as undone. It goes one step back, and an undo cannot itself be taken back. It prints the first line of the syntax it took back:

```
statscraft run -s gui -e "COMPUTE total = q1 + q2."
statscraft undo
```

```
undone: COMPUTE total = q1 + q2.
```

It fails when no window is open, and when no run since the last undo changed the data. File sessions have no undo.

## mcp

`statscraft mcp` serves describe, help, check and run as MCP tools over standard input and output, for agent apps that start local MCP servers. It runs until the app closes the connection. [The MCP server](mcp.md) lists the tools and says where their sessions live.

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
| `trail` | where the syntax of this run was kept: the absolute path of the session's `syntax.sps`, or for `gui` the syntax window, such as `syntax window Agent`. Absent without a session and for `check` |
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

GRAPH, FREQUENCIES (BARCHART, PIECHART, HISTOGRAM), CROSSTABS (BARCHART), EXAMINE (PLOT), ONEWAY (PLOT MEANS), REGRESSION (RESIDUALS, SCATTERPLOT, PARTIALPLOT), LOGISTIC REGRESSION (CLASSPLOT), UNIANOVA and GLM (PLOT), DISCRIMINANT (PLOT), FACTOR (PLOT), CLUSTER (PLOT) and TREE (the tree diagram, PLOT) make charts. A chart is data: its kind, its labels and its numbers, with nothing about colors or layout. Labels follow SET TVARS, TNUMBERS and OVARS as tables do. `statscraft run --json -e "..."` with `CROSSTABS /TABLES=region BY gender /BARCHART.` gives (shortened):

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
| `kind` | `bar`, `pie`, `line`, `area`, `error_bar` (a point per value with its error bar), `range_bar` (a floating bar between each pair of series), `drop_line` and `high_low` (points with a line from the lowest to the highest value of each category; for `high_low` the third series is the close), `difference` (two lines with the area between them shaded), `scatter`, `dot` (a dot plot: points as for `scatter`, captioned as a dot plot), `histogram`, `boxplot`, `stem_leaf`, `normal_qq`, `detrended_normal_qq`, `normal_pp`, `spread_level`, `dendrogram`, `icicle`, `tree_diagram` or `territorial_map` |
| `tier` | `presentation` (a chart the office programs can hold as an editable chart) or `statistical` (a plot drawn exactly, such as the histogram) |
| `orientation` | `vertical` (categories along the bottom) or `horizontal`; absent for a pie |
| `grouping` | `clustered`, `stacked` or `percent` (stacked to 100%); bars, lines and areas only |
| `title`, `subtitle`, `footnote` | the title, as a table of the same command would have it, an optional line under it (GRAPH /SUBTITLE), and an optional note such as the mean, standard deviation and N of a histogram |
| `category_axis`, `value_axis` | each with its `title`, the display `format` of its numbers, `maximum` when the syntax set the top of the axis, and the `variable` it shows |
| `legend` | what tells the series apart, such as the column variable; absent with one series |
| `categories` | in drawing order: the `label`, the `value` it stands for (`null` for a slice of all missing values), `missing` when it holds missing values, and `bin` (`[from, to]`) for a histogram bin, which holds its lower bound but not its upper one |
| `series` | each with its `name`, the `statistic` its numbers are (`count`, weighted when a weight is set, `percent`, `mean` or `value`), the `variable` and `value` it stands for, and `values`, one number per category |
| `normal` | a histogram's normal curve: `mean`, `std_dev` and `total`; its height at x is total times the bin width times the normal density |
| `error_bars` | bars, lines and points with error bars, such as a profile plot with ERRORBAR or a GRAPH ERRORBAR chart: a `label` saying what a bar spans (`95% CI`, `2 SE`) and `margins`, for each series a list with the half length of its bar at each category (the bar runs from the value minus it to the value plus it; an empty list for a series without bars, `null` where a value has none); `lower`, only where bars reach less far below than above (the interval of a median), lists the length below in the same way, and `margins` is then the length above |
| `second_axis` | a bar chart's second value axis on the right (`title`, `format`, `maximum`) for its last series, which is drawn as a line over the bars, such as the cumulative percentage of a GRAPH PARETO chart |
| `plot` | the data of a statistical plot other than the histogram, by its `shape`; `categories` and `series` are then empty, the category axis is the horizontal one and the value axis the vertical one |

The shapes of `plot`:

- `boxes`: `boxes`, each with its `label`, `group` (the cluster, when boxes are grouped), `n`, `lower_whisker`, `lower_hinge`, `median`, `upper_hinge`, `upper_whisker`, and the marked cases in `outliers` and `extremes` (each a `value` and its `case` label).
- `stem_leaf`: `stem_width`, `each_leaf` (cases per leaf), `lines` (each a `frequency`, the `stem` as shown and the `leaves` as digits) and `low_extremes`, `high_extremes` (a `frequency` and the `bound`).
- `points`: `groups`, each a `name` and `points` (`x`, `y`, an optional `z` and `label`); `lines`, straight reference lines `y = intercept + slope x`; and `z_axis` when the points have depth. Q-Q, P-P, spread-versus-level, scatter, partial regression and loading plots use it.
- `matrix`: `variables`, the axes in order (each with its `title` and `format`), and `groups`, each a `name` and `cases`, each case with its `values` in the order of the variables (`null` where missing) and an optional `label`. GRAPH SCATTERPLOT(MATRIX) uses it, with kind `scatter`.
- `merges`: `items` (the case labels), `order` (the layout of the items, so that no branches cross), `merges` (each joins cluster `right` into cluster `left`, both named by an item index, at `height`, with `rescaled` from 1 to 25) and, for an icicle plot, `solutions`, the numbers of clusters it shows.
- `tree`: `nodes`, each with its `id`, `parent`, the `split` condition that leads to it, and `statistics` as name and value pairs.
- `regions`: `regions`, each with its `name`, its `center` (`[x, y]`, such as a group centroid) and `vertices`, the corners of its convex polygon in order around it (empty when the region is empty). The DISCRIMINANT territorial map uses it.

With `--vega-lite` each chart also has a member `vega_lite`: a [Vega-Lite](https://vega.github.io/vega-lite/) v5 specification with the numbers inline, which any Vega-Lite viewer draws as it is. In Markdown output the specification follows the chart's data table as a `json` code block. SPLIT FILE gives a chart per split group, the group named in the title.

### Charts as input

Statscraft reads a chart back in the form it writes it, so an agent or a script can put a chart that no procedure makes into the output. The chart is drawn, saved and exported like any other: in the window's output, OUTPUT SAVE, OUTPUT EXPORT, Copy and Open in the office programs.

```
statscraft chart sales.json -s work
statscraft chart sales.json -s gui
cat sales.json | statscraft chart - --json
```

`statscraft chart FILE` (or `-` for standard input) takes `-s`, `--json`, `--vega-lite`, `--xlsx` and `--charts` as `run` does. It runs the syntax BEGIN CHART, a Statscraft extension, with the file's JSON between BEGIN CHART and END CHART, each on its own line; the same block works in any syntax file, so OUTPUT EXPORT in the same run writes the chart out. The MCP tool `add_chart` does the same. See [BEGIN CHART](commands/begin-chart.md).

The JSON is one of two forms.

The chart JSON above, as `--json` writes it. A chart from a run reads back as the same chart, so the simplest way to make one is to take a procedure's chart and change it. `"version": 1` may be given; a higher version is refused. The member `vega_lite` that `--vega-lite` adds is ignored. A field the chart does not have is an error, so a misspelt one is not lost silently; a field that is `null`, `false` or empty counts as left out. `kind`, `tier`, `title`, `category_axis` and `value_axis` (each with a `title`), `categories` and `series` (each with `name`, `statistic` and `values`) are required. A number may be `null` where there is none. A statistical plot gives its data in `plot` with the shape `points` (kinds `scatter`, `normal_qq`, `detrended_normal_qq`, `normal_pp`, `qq`, `detrended_qq`, `pp`, `detrended_pp`, `roc`, `spread_level`) or `boxes` (kind `boxplot`); the other shapes are made only by their procedures.

A [Vega-Lite](https://vega.github.io/vega-lite/) specification of one view, the part that maps cleanly to a Statscraft chart:

| Mark | Encodings | Gives |
|---|---|---|
| `bar` | `x` and `y`, one of them `quantitative`; optional `color`, and `xOffset` or `yOffset` on the field of `color` | a bar chart: vertical when `y` holds the values, horizontal when `x` does. With `color` the bars stack, as in Vega-Lite; with an offset they stand side by side; `stack: "normalize"` stacks them to 100% |
| `line` | `x` (the categories) and `y` (`quantitative`), optional `color` | a line chart, a line per color |
| `area` | the same | an area chart; with `color` stacked, or to 100% with `stack: "normalize"` |
| `point` | `x` and `y`, both `quantitative`, optional `color` | a scatter plot, a group per color; a row without both numbers is left out |
| `arc` | `theta` (`quantitative`) and `color` | a pie, a slice per color |

- The rows are inline in `data.values`, each an object. Every row gives one value: a category (and color) given twice is an error, since nothing is aggregated.
- A channel takes `field`, `type` (`quantitative`, `nominal`, `ordinal` or `temporal`; `color` and the offsets default to `nominal`), `title` and `sort`: `"ascending"` (the default, as in Vega-Lite), `"descending"`, `null` for the order of the data, or a list of values that come first. `axis`, `legend` and `scale` are accepted and change nothing; so are `width`, `height`, `config` and the other members that only set how the chart looks, and the properties of `mark` besides `type`.
- `title` is a string, or an object with `text` and `subtitle`. Without one the chart is named "values by categories" after the axis titles. An axis title is the channel's `title`, else its field.
- Refused, with a message that names what is not supported: other marks, encodings such as `size` or `shape`, `aggregate`, `bin`, `timeUnit`, a fixed `value` or `datum`, `transform`, `layer`, `facet`, `repeat` and the concatenations, `params`, data from `url` or by `name`, and a Vega (not Vega-Lite) schema.

Limits, for both forms: at most 100 series, 3000 categories, 100000 values (series times categories, points, or rows), strings of 1000 characters and a JSON text of 16 MB. A chart whose parts do not fit together, such as a series with fewer values than there are categories, is refused before anything draws it.

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

For a weighted survey file with two multiple response sets, `statscraft describe FILE --json` prints (shortened):

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
