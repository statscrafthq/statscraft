# Getting started

StatsCraft has two programs:

- `statscraft`, the command line. It reads .sav and .zsav files, runs syntax and prints tables as Markdown or JSON.
- `statscraft-gui`, the desktop window over the same engine.

The command line works without the window.

## Install

### Getting the installer or package

The installer and packages for x86_64 Windows and Linux are attached to a release on the GitHub repository's **Releases** page. They come from the **Release build** workflow, which runs only when someone starts it:

1. On GitHub open **Actions**, choose **Release build**, then **Run workflow** (with the GitHub CLI: `gh workflow run release.yml`). Tick **draft release** to also get a draft release with the files attached; a maintainer publishes it by hand.
2. When the run has finished, its page lists the files below, where `X` is the version:
   - `statscraft-X-windows-x86_64-setup.exe`, the Windows installer
   - `statscraft_X_amd64.deb`, the package for Debian and Ubuntu
   - `statscraft-X-linux-x86_64.tar.gz`, the same files for any other Linux

### Windows

Run `statscraft-X-windows-x86_64-setup.exe`. It needs no administrator rights and installs for your user only:

- both programs go to `%LOCALAPPDATA%\Programs\StatsCraft`
- the Start menu gets **StatsCraft**, which opens the window
- that folder is added to your `PATH`, so `statscraft` works in any terminal you open after the installation
- .sav and .zsav files open in the window, and .sps files in its syntax editor. Where another program is already the default for one of these types, it stays the default, and StatsCraft is offered under **Open with**.

Windows may warn that the installer comes from an unknown publisher, because it is not signed yet. Choose **More info**, then **Run anyway**.

To uninstall, open **Settings**, **Apps**, **Installed apps**, find StatsCraft and choose **Uninstall**. That removes the programs, the Start menu entry, the `PATH` entry and the file types.

### Debian and Ubuntu

```
sudo apt install ./statscraft_X_amd64.deb
```

puts `statscraft` and `statscraft-gui` in `/usr/bin`, adds StatsCraft to the applications menu and opens .sav, .zsav and .sps files in the window. `sudo apt remove statscraft` uninstalls it.

### Other Linux

The tarball holds a `bin` and a `share` folder. Unpack it into `~/.local` and refresh the file types:

```
tar -xzf statscraft-X-linux-x86_64.tar.gz --strip-components=1 -C ~/.local
update-mime-database ~/.local/share/mime
update-desktop-database ~/.local/share/applications
```

(`update-desktop-database` comes with desktop-file-utils; where it is missing, skip that line.) `~/.local/bin` must be on your `PATH`, which it is by default on most distributions once the folder exists. To uninstall, delete what the tarball put there:

```
cd ~/.local
rm bin/statscraft bin/statscraft-gui share/applications/statscraft.desktop share/mime/packages/statscraft.xml
rm share/icons/hicolor/*/apps/statscraft.png share/icons/hicolor/scalable/apps/statscraft.svg
rm -r share/doc/statscraft
update-mime-database ~/.local/share/mime
```

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
