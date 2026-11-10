# The window

```
statscraft-gui survey.sav job.sps     open a data file and a syntax file
```

`statscraft-gui` is a desktop window over the same engine, in four tabs. Files dropped on the window from the file manager open too: a .sav or .zsav file as data, a .sps file in the syntax editor (one of each at a time; the status bar names the files left out).

- **Data**: the cases in a grid that draws only the visible cells, with value labels on or off. Double-click a cell or press Enter to change it; Enter commits, Escape cancels. Arrows, Page Up and Down, Home and End move the selection, Ctrl+Home and Ctrl+End go to the first and last cell, and Tab and Shift+Tab move to the next and previous cell, committing an edit on the way. A column is as wide as the variable's display width (the width a .sav file keeps for each variable); drag the border at the right of its name to change it, which runs `VARIABLE WIDTH var (n).`, so the width is saved with the data.
- **Variables**: one row per variable with name, type, format, width, decimals, label, value labels, missing values and measure. Double-click to change a field. Drag a column border to resize it; these widths are a window setting.
- **Syntax**: an editor with line numbers and colours for command names, subcommands, keywords, strings, numbers, comments, macro words (`!name`) and the period that ends a command. Run all, or Run selection (Ctrl+Enter) for the commands on the selected lines; on a line with no command (the empty line under a command just typed) it runs the command above. Errors show at their line and column. While typing, a list offers what fits at the cursor: command names at the start of a command (abbreviations such as FRE or VAL LAB find the full name), subcommands after `/`, keywords of the command, the variables of the open data with their labels, and function names where an expression goes. Ctrl+Space opens the list at any point; arrows choose, Enter or Tab puts the choice in, Escape closes it. A command in the list, or a command name under the mouse, shows its summary and syntax; F1 opens the full help of the command at the cursor in a small window.
- **Output**: every result table with its multi-level header, beside a navigator that groups the tables by run and command (a run folds away with a click on it). Columns are as wide as their numbers, which align right; long row labels and column headers wrap between words, and a row label repeated under the same outer labels is shown once beside all its rows. The caption and notes follow each table. Click a table, or its name in the navigator, to select it (Ctrl+click for more, Ctrl+A for all); the syntax of its command shows under the navigator, exactly as it ran, with Copy, Paste (into the syntax editor) and Run again, and pointing at a command in the navigator shows it too; then Copy (Ctrl+C) puts it on the clipboard with its layout for a word processor or spreadsheet and as plain text beside it, Copy as text gives tab-separated text, Delete removes it, and Export selection saves only the selected tables as a web page or an Excel workbook. Right-click a table for the same. Each table is measured once and only the tables in view are drawn, so hundreds of tables scroll smoothly.

File, Export output as HTML saves the whole output as one web page; File, Export output as Excel saves it as a workbook, with One sheet per table or All tables on one sheet, laid out as `--xlsx` does.

The menus have dialogs that build syntax:

- **Data**: Define Multiple Response Sets (dichotomy or category sets, counted value, labels, deleting sets), Sort Cases, Split File (groups compared in the same tables or in tables of their own, sorting the cases first, or off), Select Cases (a filter that leaves the other cases out of analyses: `COMPUTE filter_$ = (condition).` and `FILTER BY filter_$.`; SELECT IF, which deletes them; or all cases again with `FILTER OFF.` and `USE ALL.`), Weight Cases, Aggregate (into every case, a new dataset of one case per group, or a file) and Merge Files with Add Cases and Add Variables (with keys, or a lookup table).
[removed]
- **Transform**: Compute, Recode, Count Values within Cases, Rank Cases (every RANK function with new names or the default ones, ranks within groups, from the smallest or the largest value, ties, the proportion formula, user-missing values) and Automatic Recode (AUTORECODE into new variables, from the largest value, one scheme for all, blank strings as missing, a table of the values).

A dialog shows the syntax it builds: OK runs it, Paste puts it in the syntax editor. Options left at their defaults add nothing to the syntax. Warnings of a run appear under its command in the log, and the status bar says how many there were. At its right the status bar names the split, filter and weight in force, such as `Split by region (layered)`, `Filter: filter_$` and `Weight: weight`; a click on one opens Split File, Select Cases or Weight Cases.

Syntax is the only command layer: every change and every result in the window comes from syntax run by the engine, exactly as `statscraft run` runs it. Opening a file runs `GET FILE`, editing a cell runs `IF ($CASENUM = n) var = value.`, editing a label runs `VARIABLE LABELS`, and so on. The log under the tabs lists every command with what it changed, and Copy syntax there gives a script that repeats the session. Macros defined by one Run (DEFINE, or INCLUDE of a macro library) stay defined for the later ones while the window is open, with the macro settings of SET.

View, Paper chooses how the Data and Variable grids, the syntax text and the output tables look: Always light (the default) keeps them dark text on a light sheet while menus, panels and dialogs follow the system's dark or light theme, as a spreadsheet or a document page does; Follow the system theme draws them in the system theme too.

Help, Documentation opens these pages online; Help, About shows the version, the license (MIT) and links to the source code and the documentation.

The window remembers its size and position, the folder of the last file dialog, the ten files opened or saved last (File, Recent files), the paper setting, the Variable View's column widths and the update check setting in `statscraft/window.txt` in the user's config folder: `%APPDATA%` on Windows, `$XDG_CONFIG_HOME` or `~/.config` elsewhere.

Edit, Undo last change (Ctrl+Z outside a text field) puts back the dataset from before the last run that changed it, one step; the log marks that run as undone and Copy syntax leaves it out. Long lines in the syntax editor wrap at the window edge; View turns that off.

Opening a file and every run happen off the drawing thread: the status bar shows what is running and the window stays responsive. One run goes at a time; requests from the command line wait their turn.

## Updates

The window asks GitHub at most once a day, in the background and never holding up the start, whether a newer release is published. Checks are on by default and say nothing unless a newer version is out. Help, Check for updates automatically turns them on and off (the line `updates on` or `updates off` in `window.txt`), and Help, Check for updates checks at once and also says when there is nothing new.

When a newer release is out, a small bar under the menu says so, with a link to the release notes, **Update now** and **Later**. Later hides the bar until the next check. Update now downloads the package for this installation, checks it against the release's `SHA256SUMS` and then:

- installed with the Windows installer: closes the window, runs the new installer silently for the same per-user installation (same folder, file types and Start menu entry) and the installer opens the window again. Save your work first.
- installed from the tarball: replaces `statscraft` and `statscraft-gui` in its `bin` folder (each written next to the old one, then renamed over it) and offers **Restart now**.
- installed from the .deb: saves the package in `~/Downloads` and shows the one command that installs it, `sudo apt install ~/Downloads/statscraft_X_amd64.deb`, with a button that copies it. The window never runs sudo.
- built from source or copied elsewhere: there is no Update now; the bar says to install the new version the way this copy was installed.

The check is one request to the GitHub API with the User-Agent `statscraft/VERSION`; nothing about you or your data is sent. `statscraft update` does the same from the command line.

## Driving the window from the command line or an AI agent

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
