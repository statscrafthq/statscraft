# Getting started

Statscraft has two programs:

- `statscraft`, the command line. It reads .sav and .zsav files, runs syntax and prints tables as Markdown or JSON.
- `statscraft-gui`, the desktop window over the same engine.

The command line works without the window.

## Install

### Download

There are packages for x86_64 Windows and Linux, and for macOS on Apple silicon and Intel. Download the one for your system from the [latest release](https://github.com/statscrafthq/statscraft/releases/latest) on GitHub. `X` stands for the version:

- `statscraft-X-windows-x86_64-setup.exe`, the Windows installer
- `statscraft-X-windows-x86_64.zip`, the same programs for Windows without an installer
- `statscraft_X_amd64.deb`, the package for Debian and Ubuntu
- `statscraft-X-linux-x86_64.tar.gz`, the same files for any other Linux
- `statscraft-X-macos-universal.tar.gz`, Statscraft.app and the command line for macOS
- `SHA256SUMS`, the checksums of these files

Every package holds both programs, `statscraft` and `statscraft-gui`.

### Windows

Run `statscraft-X-windows-x86_64-setup.exe`. It needs no administrator rights and installs for your user only:

- both programs go to `%LOCALAPPDATA%\Programs\Statscraft`
- the Start menu gets **Statscraft**, which opens the window
- that folder is added to your `PATH`, so `statscraft` works in any terminal you open after the installation
- .sav and .zsav files open in the window, and .sps files in its syntax editor. Where another program is already the default for one of these types, it stays the default, and Statscraft is offered under **Open with**.

Windows may warn that the installer comes from an unknown publisher, because it is not signed yet. Choose **More info**, then **Run anyway**.

To uninstall, open **Settings**, **Apps**, **Installed apps**, find Statscraft and choose **Uninstall**. That removes the programs, the Start menu entry, the `PATH` entry and the file types.

### Windows without an installer

The portable `statscraft-X-windows-x86_64.zip` needs no installation and no administrator rights, so it also works where you may not install programs. Unpack it anywhere you can write, for example to a folder in Documents or on a USB stick (in Explorer: right-click, **Extract All**). The folder `statscraft-X-windows-x86_64` holds:

- `statscraft-gui.exe`, the window: double-click it to start Statscraft
- `statscraft.exe`, the command line: run it in a terminal from that folder, or add the folder to your `PATH`
- `LICENSE.txt` and `README.md`

Nothing is registered: there is no Start menu entry and no file types, so open files with File > Open data or drag them onto `statscraft-gui.exe`. Windows may warn about an unknown publisher the first time, as for the installer: **More info**, then **Run anyway**. To remove it, delete the folder.

Keep `LICENSE.txt` next to the programs: the updater recognizes the portable copy by it, and updates the two programs in that folder.

### macOS

`statscraft-X-macos-universal.tar.gz` runs on Apple silicon and on Intel Macs. Double-click it in Finder to unpack it. The folder `statscraft-X-macos-universal` holds:

- `Statscraft.app`, the window: drag it to **Applications**
- `statscraft`, the command line
- `LICENSE` and `README.md`

Statscraft is not signed with an Apple Developer ID yet, so the first time you open it macOS says it cannot verify the app. Click **Done** (not **Move to Trash**), then:

1. Choose Apple menu > **System Settings**, then **Privacy & Security** in the sidebar.
2. Under **Security**, find the message about Statscraft and click **Open Anyway**. The button is there for about an hour after you tried to open the app.
3. Enter your login password and click **OK**.

After that, Statscraft.app opens with a double-click like any other app.

The command line needs the same once. Move it to a folder on your `PATH`, for example:

```
mkdir -p ~/.local/bin
mv ~/Downloads/statscraft-X-macos-universal/statscraft ~/.local/bin/
```

(add `export PATH="$HOME/.local/bin:$PATH"` to `~/.zshrc` if that folder is not on your `PATH` yet). The first `statscraft --version` is blocked; allow it under **Privacy & Security** as above and run it again. Instead of that, you can clear the download mark in the terminal: `xattr -d com.apple.quarantine ~/.local/bin/statscraft`.

Finder offers Statscraft under **Open With** for .sav, .zsav and .sps files. Opening a file this way should open it in the window, as if it had been dropped there; this has not been tried on a Mac yet. If Statscraft starts without the file, use File > Open data, and please report it on the [Issues page](https://github.com/statscrafthq/statscraft/issues) of the GitHub repository. To uninstall, move Statscraft.app to the Bin and delete the `statscraft` program.

### Debian and Ubuntu

```
sudo apt install ./statscraft_X_amd64.deb
```

puts `statscraft` and `statscraft-gui` in `/usr/bin`, adds Statscraft to the applications menu and opens .sav, .zsav and .sps files in the window. `sudo apt remove statscraft` uninstalls it.

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

### Updating

When a new version is published, the window says so in a bar under the menu: **Update now** installs it, **Later** hides the bar. From the command line, `statscraft update --check` tells whether there is one and `statscraft update` installs it. How it works for each kind of installation:

- Windows: the new installer is downloaded, checked and run silently. The window asks about unsaved work, closes, and opens again in the new version.
- Windows without an installer: the new .zip is downloaded and checked, and the two programs in its folder are replaced; restart the window to use the new one. The old programs are deleted the next time Statscraft starts.
- Tarball under `~/.local`: the two programs in `~/.local/bin` are replaced; restart the window to use the new one.
- Debian and Ubuntu: the new .deb is downloaded and checked into `~/Downloads`, and you install it with the command shown, `sudo apt install ~/Downloads/statscraft_X_amd64.deb`.
- macOS: the bar has a **Download** link to the new `statscraft-X-macos-universal.tar.gz`. Unpack it and replace Statscraft.app in Applications (and the `statscraft` program) with the new ones. The first start of the new version needs **Open Anyway** again. The updater does not change files inside the app itself, which a later signed version would not allow. On macOS, `statscraft update` changes nothing: it stops with an error that names the release page.

Every download is checked against the checksums published with the release before it is used. The window checks at most once a day, sending only one request to GitHub and nothing about you or your data. It is on by default and silent unless there is an update; Edit > Settings > Updates turns it off. See [the window](window.md#updates) and [the command line](command-line.md#update).

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
### Gender

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

A session also keeps the macros its runs defined (DEFINE, or a macro library brought in with INCLUDE) and the macro settings of SET (MEXPAND, MPRINT, MNEST, MITERATE), so a library included once serves every later run.

Sessions live in `.statscraft/sessions/<name>/` under the current folder. Delete that folder to start over.

## The window in five minutes

Start the window with a data file and a syntax file:

```
statscraft-gui tests/fixtures/survey_basic.sav job.sps
```

(Both are optional. From a source checkout `cargo run -p statscraft-gui -- tests/fixtures/survey_basic.sav job.sps` does the same.)

Each file opens in a window of its own, and all windows work on one session:

1. The data window shows the dataset. Its **Data** tab shows the cases: double-click a cell, or press Enter, to change it. **Show value labels**, above the grid, switches value labels on and off. Its **Variables** tab has one row per variable: double-click a label, a format or the missing values to change them.
2. The syntax window holds `job.sps`. Select lines 4 to 6 and press Ctrl+Enter to run only those commands. F1 shows help for the command at the cursor. An error is marked at its line and column.
3. The output window opens with the first table and collects every table after it. **Copy** puts a table on the clipboard for a spreadsheet; File > Export output as Excel saves all its tables as a workbook.
4. The menus **Data**, **Analyze** and **Transform** open dialogs that build syntax. **OK** runs it, **Paste** puts it in the syntax window.
5. Open a second data file and it gets a second data window. A click in a data window makes its dataset the active one, the one every command works on.

Everything the window does is syntax. The log in the syntax and output windows lists every command it ran, and **Copy syntax** there gives a script that repeats your session with `statscraft run`. Edit > Undo last change (Ctrl+Z) takes back the last run that changed the data.

A few more things to know:

- The grid draws only the visible cells, so large files stay quick.
- Arrows, Page Up and Page Down, Home and End move the selection. Ctrl+Home and Ctrl+End go to the first and last cell. Tab and Shift+Tab move to the next and previous cell, committing an edit on the way.
- Long lines in the syntax editor wrap; Edit > Settings > Editor > Wrap long lines turns that off.
- The window brings its own fonts. Set `STATSCRAFT_FONT` or `STATSCRAFT_MONO_FONT` to a .ttf file to use another.

[The window](window.md) describes every window, menu and dialog.

While the window is open, the command line and AI agents can work in it as the session `gui`:

```
statscraft run -s gui -e "FREQUENCIES VARIABLES=age_group."
```

The table appears in the output window. See [the command line](command-line.md#the-live-window-session) and [AI agents](ai-agents.md).

## Next

- [The window](window.md): every window, menu, dialog and setting.
- [The command line](command-line.md): every subcommand, flag, JSON shape and exit code.
- [AI agents](ai-agents.md): how an agent should drive Statscraft.
- [Commands](commands/README.md): one page per supported command.
- [Differences and limits](differences.md): where Statscraft differs from other implementations, and what is not supported yet.
