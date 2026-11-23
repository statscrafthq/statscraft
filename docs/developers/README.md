# Developer notes

How Statscraft keeps its state and talks to the outside world. The user guides describe what a user does and sees; this page holds the details behind it. [CONTRIBUTING.md](../../CONTRIBUTING.md) explains how to build, test and release, and [Writing a plugin](plugins.md) the plugin protocol.

## The window's settings file

The window keeps its settings in `statscraft/window.txt` in the user's config folder: `%APPDATA%` on Windows, `$XDG_CONFIG_HOME` or `~/.config` elsewhere. It holds:

- the size and position of the last data, syntax and output window;
- the folder of the last file dialog and the ten recent files;
- the column widths of the Variables tab;
- every choice of Edit > Settings, one line each, such as:
  - `output-shows titles tables charts` for Show in the output, and `export hidden` for the Export dialog's Hidden items too;
  - `tvars names` and `tnumbers both` for Variables in tables and Values in tables, and `mioutput observed pooled` for the output of multiply imputed data. The window runs the matching `SET TVARS`, `SET TNUMBERS` and `SET MIOUTPUT` when it starts;
  - `updates on` or `updates off`;
  - the plugin lines described in [Writing a plugin](plugins.md#what-the-window-keeps).

The command line never reads this file.

## Updates

The window and `statscraft update` ask the GitHub API for the newest published release (drafts and prereleases do not count) and compare its tag `vX.Y.Z` with their own version. The window asks at most once a day, in the background. The request carries the User-Agent `statscraft/VERSION` and nothing else; there is no account, token or telemetry. An update also downloads the release's `SHA256SUMS` and the one package, and checks the package against it before anything is run or replaced. On a mismatch nothing changes and the file is deleted.

What an update does, by installation:

- **Windows installer**: the new installer goes to a temporary folder and runs silently for the same per-user installation. The window closes first, after asking about unsaved work, and the installer opens it again. If the window stays at the old version, `statscraft-setup.log` in `%TEMP%` says why.
- **Tarball**: `statscraft` and `statscraft-gui` in its `bin` folder are replaced, each written next to the old one and then renamed over it.
- **Portable Windows .zip** (recognized by `LICENSE.txt` next to the programs): the two programs in that folder are replaced the same way. Windows cannot replace a running program, so the old one is renamed aside first and deleted the next time Statscraft starts.
- **.deb**: the package is saved in `~/Downloads`. The window installs it with `pkexec apt-get install`; when pkexec is missing or fails, it shows the `sudo apt install` command instead. `statscraft update` only prints that command and never runs sudo.
- **macOS**: nothing is replaced. The updater does not change files inside Statscraft.app, which a signed app would not allow; the window links to the new package.
- **Built from source or copied elsewhere**: the window names the folder it runs from and says to install the new version the same way; `statscraft update` refuses with exit code 1.

## The live window session

On start, the window listens on a random TCP port of `127.0.0.1` and writes the address and a random token to `statscraft/gui.json`: in `$XDG_RUNTIME_DIR` on Linux, `%LOCALAPPDATA%` on Windows, `~/.config` otherwise. The file is readable only by the user and removed when the window closes. Every request must carry the token. The command line reads it from the file and sends it only to a loopback address. Requests larger than 64 MB are refused. With several windows open, the one started last answers. [Safety of the live session](../ai-agents.md#safety-of-the-live-session) says what this means for users.

## Session folders

A command-line session lives in `.statscraft/sessions/<name>/` under the current folder. The MCP server keeps its sessions in the user data folder instead (see [Sessions and the window](../mcp.md#sessions-and-the-window)). A name may hold letters, digits, `-` and `_`.

| What | Kept | Where in the session folder |
|---|---|---|
| The active dataset: data, dictionary, weight, file label, documents, attributes, multiple response sets | yes | `data.zsav` |
| SPLIT FILE and FILTER of the active dataset | yes | `state.json` |
| Where the random number generators stand (SAMPLE, UNIFORM, NORMAL, RV functions), so the next run goes on where the last one stopped | yes | `state.json` |
| Named datasets (DATASET NAME, COPY, DECLARE) and which one is active, each with its split and filter | yes | `datasets/` |
| Macros (DEFINE) and the macro settings MEXPAND, MPRINT, MNEST, MITERATE | yes | `macros.sps` |
| File handles (FILE HANDLE), SET values, PRESERVE levels not yet restored, the CD folder | yes | `settings.sps`, as FILE HANDLE, SET, PRESERVE and CD commands |
| The syntax of every run that worked, oldest first: the trail of how the session got where it is | yes | `syntax.sps`, each run under a comment line such as `* Agent 2026-11-12 14:32.` |
| TEMPORARY, N OF CASES, scratch variables, VECTOR | no: the end of a run is a data pass, so they take effect or end in the run that gives them | |
| An open DO IF, LOOP, or a DATA LIST without its BEGIN DATA | no: the run fails | |
| The folder of the syntax file, where INSERT and INCLUDE start | no: each run starts from its own file | |
| Output documents (OUTPUT NEW, NAME, ACTIVATE) with their tables, charts, hidden items, labels, plugin comments, the designated one and the name counter | yes | `outputs/`: `names.json` and one `<n>.scout` per document |
| The workbooks OUTPUT EXPORT wrote (CREATESHEET and MODIFYSHEET add only to one written in the same run), the files REPORT wrote, the TITLE text | no | |

A run saves the session when it changed anything in the table above, or when the session did not exist yet. A dataset keeps the encoding of the file it came from; when a label or value no longer fits that code page, the session keeps it in UTF-8 instead. When the trail `syntax.sps` cannot be written, the run fails and the session stays as it was.

## Output files

OUTPUT SAVE writes an output document as a Statscraft output file (`.scout`), and OUTPUT OPEN reads one back.

A file may come from anyone, so OUTPUT OPEN checks every table and chart before anything shows it. A chart passes the checks of [charts as input](../command-line.md#charts-as-input), with limits wide enough for what procedures make (100000 categories, 2000000 values, strings of 32768 characters), and the indexes of a dendrogram, an icicle plot and a tree diagram must point where they should. A table's rows must have a cell for every column and its formats the shape of its rows, within 1000000 rows, 10000 columns and 10000000 cells. An item that fails is not shown: a warning names it and says why, the rest of the file opens, and OUTPUT SAVE writes the item back as it was read. Fields a newer Statscraft added are dropped.

The file is UTF-8 JSON, so an agent or a script reads it directly, with no Statscraft call (shortened here):

```
{
  "format": "statscraft-output",
  "version": 1,
  "written_by": "statscraft 0.8.1",
  "items": [
    {
      "kind": "command",
      "command": "CROSSTABS",
      "syntax": "CROSSTABS region BY gender.",
      "warnings": [],
      "items": [
        {
          "kind": "table",
          "title": "region by gender",
          "row_headers": 2,
          "columns": ["", "", "Female", "Male", "Total"],
          ...
          "rows": [
            ["North", "Count", 22, 19, 41],
            ...
          ]
        }
      ]
    }
  ]
}
```

- `items` holds one `command` item per command, in order, with the syntax exactly as it ran, its warnings and its own items. A `table` item is a table as in [the run result](../command-line.md#run-result); a `chart` item holds the chart under `chart`, as in [charts](../command-line.md#charts).
- Keys come in a fixed order and each table row is one line, so two saved files diff well. A whole number has no fraction. A table number that is not finite is written as the text the table shows.
- `version` rises only when a field changes its meaning. A file from a newer Statscraft is refused with a message that says so; older versions are read and brought up to date. An item of a kind this version does not know is kept and written back as it was.
- A path ending in `.spv`, `.spo` or `.spw` (another program's output files) is saved as `.scout` under the same name, with a warning; OUTPUT OPEN of such a path reads that `.scout` file. A password on OUTPUT SAVE /PASSPROTECT is an error, since a Statscraft output file is never encrypted.
- OUTPUT OPEN of a missing or foreign file is a warning, and the run goes on.
