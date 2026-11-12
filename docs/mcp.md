# The MCP server

`statscraft mcp` lets an AI agent work with Statscraft through tools instead of shell commands. It speaks the Model Context Protocol (MCP) over standard input and output, so any agent app that runs local MCP servers can use it: Claude Desktop, Claude Code, ChatGPT desktop and Codex, Cursor, VS Code with Copilot and others. It ships with the `statscraft` command line, so there is nothing more to install, and the data never leave your computer.

The easy way to connect an agent is `statscraft connect`, which finds the agent apps you have installed and adds Statscraft to each one. Claude Desktop also installs it from a bundle with a double click, and Claude Code from a plugin; see [Connect your AI agent](connect.md). To set one up by hand, give it the command `statscraft mcp`, with the full path to `statscraft` if it is not on your PATH.

## The tools

| Tool | Input | What it gives back |
|---|---|---|
| `describe` | `file` (a .sav or .zsav file), or `session` | the dictionary and counts, and for a session what it keeps; the same JSON as `statscraft describe --json` |
| `list_commands` | none | every supported command with a one-line summary, as `statscraft help` |
| `command_help` | `command` | the syntax, rules and an example of one command, as `statscraft help COMMAND` |
| `check_syntax` | `syntax` or `file`, and optional `lines`, `session`, `folder`, `vega_lite` | the result of a check, as `statscraft check --json`; nothing is kept or written |
| `run_syntax` | the same as `check_syntax`, and optional `xlsx`; or `window` (with optional `lines`) instead of `syntax` and `file` | the result of a run, as `statscraft run --json`; the session keeps the new state, and `trail` says where its syntax was kept |
| `window_status` | none | whether a Statscraft window is open, and if so its datasets and output documents |
| `list_syntax_windows` | none | the syntax windows of the open window: `id`, `name`, `file`, `unsaved`, `lines`, and whether it is the Agent window, the designated one or the one the person used last |
| `read_syntax` | `window` | the text of a syntax window, each line with its number |
| `edit_syntax` | `window`, `mode`, `text`, and `lines` or `before` | changes the text: `replace` the lines `lines` (an empty text deletes them), `insert` before line `before`, `append`, or `replace_all`; gives the lines the new text takes |
| `new_syntax_window` | optional `name` and `text` | a new syntax window, empty or with the text |
| `open_syntax_file` | `path` | the .sps file in a syntax window, or the window that has it open already |
| `save_syntax` | `window`, optional `path` and `overwrite` | the window written to its own file or to `path` |

- `syntax` is the text of one or more commands; `file` is the absolute path of a .sps file. Give one of them.
- `lines` limits the run to the commands within a line range, such as `"12-20"`. Line numbers stay those of the whole text.
- `folder` is the absolute folder that relative file names in the syntax start from. Default: your home folder. Give absolute paths in the syntax to be sure.
- `xlsx` also writes every table and chart of the run to an Excel workbook, one sheet per table; a chart as its data table with an editable chart beside it.
- `vega_lite` set to true gives every chart also as a Vega-Lite specification, as `--vega-lite` does on the command line.
- `window` names a syntax window of the open window by its id or its name, as `list_syntax_windows` shows them. The syntax window tools work only while the window is open; without it they fail with "no Statscraft window is running".

A run or check returns the JSON described in [the command line](command-line.md#json-output) as structured content, and the tables and charts as Markdown text with a summary line for apps that show only text. A syntax or run error is a normal result: `ok` is false and `errors` gives the line, column and what was expected, so the agent can fix the syntax and try again. A call the server cannot carry out at all, such as a missing file or a bad argument, is marked as a tool error.

The server also sends the working loop to the agent when it connects, and offers it as the prompt `analysis`: describe the data first, look up each command, check, run in small steps, read `ok`, `errors` and the warnings, and report the numbers from the result tables instead of computing them.

## Every step leaves syntax

Whatever an agent does through `run_syntax` stays readable as syntax, so the person can check it, change it and run it again without the agent:

- In the open window, each successful run is added to a syntax window named Agent. It opens with the first run; if the person closes it, the next run opens a new one. Each run comes under a comment line with the time, such as `* Agent 14:32.`, followed by exactly the syntax that ran. The person can edit it, run lines from it with Ctrl+Enter and save it like any syntax window. Runs sent with `statscraft run -s gui` from the command line go there too.
- A run from a syntax window (`run_syntax` with `window`) is not copied: that window's text already is its record.
- In a file session, each successful run is added to `syntax.sps` in the session's folder, under a comment line with the date and time.
- A failed run (`ok` false) changed nothing and leaves nothing; neither does a check.
- `trail` in the result, and a line `syntax kept in:` in the text, say where the syntax went: `syntax window Agent`, or the path of the file.

## Syntax windows

With the window open, an agent can work in the person's syntax windows as the person would:

- `list_syntax_windows` and `read_syntax` show what is there, unsaved changes included. Line numbers are those a run counts.
- `edit_syntax` changes the text in the editor; the person sees it at once and can take it back with Ctrl+Z in that editor. It never runs anything and writes no file.
- `run_syntax` with `window` runs the window's text, or with `lines` the commands those lines touch, exactly as Run and Ctrl+Enter in that window do: relative file names start from the window's file, and errors are marked in its margin.
- `new_syntax_window` and `open_syntax_file` open a window; a window an agent opens takes pasted syntax from the dialogs, as a window the person opens does. The Agent window never does.
- `save_syntax` writes the window to its own file, or to `path`. It does not replace any other existing file unless `overwrite` is true, and never a file that another syntax window holds.

When the person asks to fix or explain their syntax, the agent reads the window, says what it would change, makes the change with `edit_syntax` and runs it from the window.

## Sessions and the window

A session keeps the data, settings, macros and named datasets between calls, as with `statscraft run -s NAME` (see [sessions](command-line.md#sessions-and-line-ranges)).

- When the Statscraft window is open, calls without a `session` go to the window (the session `gui`). You see every table appear in the designated output window and every command in the log, and you can keep working in the window between the agent's steps.
- When no window is open, calls without a `session` use a session named `work`.
- File sessions of the MCP server live in your user data folder, not in the folder the agent app started the server in: `~/.local/share/statscraft/sessions` on Linux (or `$XDG_DATA_HOME/statscraft/sessions`), `~/Library/Application Support/Statscraft/sessions` on macOS, and `%LOCALAPPDATA%\Statscraft\sessions` on Windows. Delete a session's folder for a fresh start, or ask the agent to use a new session name.

## Safety

- The server can do what the command line can do, and nothing more. Syntax can read files, and it can write them with `SAVE`, `AGGREGATE OUTFILE`, `OUTPUT EXPORT` or the `xlsx` argument. `check_syntax` writes nothing; nothing is written without one of those.
- The tools tell the agent app which of them only read (`describe`, `list_commands`, `command_help`, `check_syntax`, `window_status`, `list_syntax_windows`, `read_syntax`), which change only the window's syntax windows (`edit_syntax`, `new_syntax_window`, `open_syntax_file`), and that `run_syntax` and `save_syntax` can change files. Agent apps ask you before a tool runs, unless you allowed it; allow `run_syntax` only for agents you trust with your files.
- The server talks only to the app that started it, over standard input and output. It opens no network port. Calls to the window use the window's local connection, described in [AI agents](ai-agents.md#safety-of-the-live-session).
