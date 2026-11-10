# Getting started

StatsCraft has two programs:

- `statscraft`, the command line. It reads .sav and .zsav files, runs syntax and prints tables as Markdown or JSON.
- `statscraft-gui`, the desktop window over the same engine.

The command line works without the window.

## Install

### Release zips

There is no installer yet. Zips for x86_64 Windows and Linux come from the **Release build** workflow of the GitHub repository, which runs only when someone starts it:

1. On GitHub open **Actions**, choose **Release build**, then **Run workflow** (with the GitHub CLI: `gh workflow run release.yml`).
2. When the run has finished, its page lists `statscraft-windows-x86_64.zip` and `statscraft-linux-x86_64.zip`.
3. Unzip one. It holds `statscraft`, `statscraft-gui`, the README and the license.

On Windows, start `statscraft-gui.exe` to open the window. `statscraft.exe` is the command line and runs in a terminal. Put the folder on your `PATH` to call `statscraft` from anywhere.

### Build from source

You need Rust 1.95 or newer (`rustc --version` shows yours; `rustup update stable` gets a newer one).

```
git clone https://github.com/statscrafthq/statscraft
cd statscraft
cargo build --release -p statscraft -p statscraft-gui
```

The programs are then in `target/release/`. The examples below call them `statscraft` and `statscraft-gui`; put `target/release` on your `PATH`, or write the full path.

## Open a .sav file

All examples run from the root of the repository and use its synthetic test data in `tests/fixtures/`.

```
statscraft describe tests/fixtures/survey_basic.sav
```

prints the size of the file and one line per variable: name, format, measurement level, number of valid values and label.

```
200 cases, 19 variables
resp_id	F6.0	nominal	valid 200	Respondent ID
gender	F1.0	nominal	valid 200	Gender
age	F3.0	scale	valid 200	Age in years
region	F1.0	nominal	valid 200	Region of residence
q1_satisfaction	F2.0	ordinal	valid 179	Q1. Overall, how satisfied are you with our service?
...
```

Add `--json` for the full dictionary: value labels, missing values, counts of system- and user-missing values, the weight variable and multiple response sets. See [the command line](command-line.md#describe).

## Run a syntax file

Save these lines as `job.sps`:

```
GET FILE='tests/fixtures/survey_basic.sav'.
FREQUENCIES VARIABLES=gender region.

RECODE age (LOWEST THRU 34 = 1) (35 THRU 54 = 2) (55 THRU HIGHEST = 3) INTO age_group.
VARIABLE LABELS age_group 'Age group'.
VALUE LABELS age_group 1 'Under 35' 2 '35 to 54' 3 '55 and over'.

CROSSTABS /TABLES=age_group BY gender /CELLS=COUNT COLUMN /STATISTICS=CHISQ.
```

and run it:

```
statscraft run job.sps
```

Every table comes out as Markdown, followed by a summary of the run:

```
### gender: Gender

| Category | Value | Label | Frequency | Percent | Valid Percent | Cumulative Percent |
| --- | --- | --- | --- | --- | --- | --- |
| valid | 1 | Male | 106 | 53 | 53 | 53 |
| valid | 2 | Female | 88 | 44 | 44 | 97 |
| valid | 3 | Other or prefer to self-describe | 6 | 3 | 3 | 100 |
| total |  |  | 200 | 100 |  |  |
...
dataset loaded: 200 cases, 20 variables
ran 6 command(s)
```

Nothing is written to disk unless the syntax says `SAVE OUTFILE='...'`.

To see what a command accepts:

```
statscraft help                 every supported command and the general rules
statscraft help CROSSTABS       the syntax of one command, with an example
```

The same text is in [the command pages](commands/README.md).

## Keep the data between runs: sessions

A session keeps the dataset between runs, so you can run a long script piece by piece:

```
statscraft run -s demo --lines 1-1 job.sps        GET only: the session now holds the file
statscraft run -s demo --lines 4-6 job.sps        RECODE and the labels
statscraft run -s demo -e "FREQUENCIES VARIABLES=age_group."
statscraft describe -s demo
```

The second run prints `variables added: age_group` and `ran 3 command(s), session saved`. A run is all or nothing: when one command fails, the session stays as it was, so you fix the line and run the same range again.

Sessions live in `.statscraft/sessions/<name>/` under the current folder. Delete that folder to start over.

## The window in five minutes

Start the window with a data file and a syntax file:

```
statscraft-gui tests/fixtures/survey_basic.sav job.sps
```

(Both are optional. From a source checkout `cargo run -p statscraft-gui -- tests/fixtures/survey_basic.sav job.sps` does the same.)

1. **Data** shows the cases. Double-click a cell, or press Enter, to change it. View, Value labels in Data View switches value labels on and off.
2. **Variables** has one row per variable. Double-click a label, a format or the missing values to change them.
3. **Syntax** holds `job.sps`. Select lines 4 to 6 and press Ctrl+Enter to run only those commands. F1 shows help for the command under the cursor. An error is marked at its line and column.
4. **Output** collects every table. Copy puts a table on the clipboard for a spreadsheet; File, Export output as Excel saves all tables as a workbook.
5. The menus **Data**, **Analyze** and **Transform** open dialogs that build syntax. OK runs it, Paste puts it in the syntax editor.

Everything the window does is syntax: the log under the tabs lists every command it ran, and Copy syntax there gives a script that repeats your session with `statscraft run`. Edit, Undo last change (Ctrl+Z) takes back the last run that changed the data.

The grid draws only the visible cells, so large files stay quick. Arrows, Page Up and Down, Home and End move the selection, Ctrl+Home and Ctrl+End go to the first and last cell, and Tab and Shift+Tab move to the next and previous cell, committing an edit on the way. Long lines in the syntax editor wrap; View turns that off. The window uses system fonts; set `STATSCRAFT_FONT` or `STATSCRAFT_MONO_FONT` to a .ttf file to choose others.

While the window is open, the command line and AI agents can work in it as the session `gui`:

```
statscraft run -s gui -e "FREQUENCIES VARIABLES=age_group."
```

The table appears in the window's Output tab. See [the command line](command-line.md#the-live-window-session) and [AI agents](ai-agents.md).

## Next

- [The command line](command-line.md): every subcommand, flag, JSON shape and exit code.
- [AI agents](ai-agents.md): how an agent should drive StatsCraft.
- [Commands](commands/README.md): one page per supported command.
- [Differences](differences.md): where StatsCraft differs from other implementations, and what is not supported yet.
