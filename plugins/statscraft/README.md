# Statscraft plugin for Claude Code

This plugin gives Claude Code two things:

- the Statscraft MCP server (`statscraft mcp`), with the tools `describe`, `list_commands`, `command_help`, `check_syntax`, `run_syntax` and `window_status`, and for the syntax windows of the open window `list_syntax_windows`, `read_syntax`, `edit_syntax`, `new_syntax_window`, `open_syntax_file` and `save_syntax`, and `add_chart`, which puts a chart of the agent's own into the output
- the skill `statscraft-analysis`, which teaches the working loop: describe the data, look up each command, check, run in steps, read the reply, and report numbers only from the result tables

## Install

Install Statscraft first ([Getting started](https://docs.statscraft.org/getting-started.html)). Then, in Claude Code:

```
/plugin marketplace add statscrafthq/statscraft
/plugin install statscraft@statscraft
```

Start a new session, and ask Claude to describe a data file.

## The `statscraft` program must be on PATH

The plugin starts the server as `statscraft mcp`, so Claude Code must find `statscraft` on your PATH. The Windows installer, the .deb package and the Linux tarball (unpacked into `~/.local`) put it there. The portable Windows .zip and the macOS download do not, until you add the folder to PATH yourself, as [Getting started](https://docs.statscraft.org/getting-started.html#install) describes.

If `statscraft --version` does not work in a new terminal, do not use the plugin's server: run `statscraft connect claude-code` with the full path of the program instead. It adds the server to Claude Code with that full path, so PATH does not matter. The skill of the plugin works either way.

If Claude Code was connected with `statscraft connect` before and `statscraft` is on PATH, the server runs twice, once for each entry. `statscraft connect --remove claude-code` takes out the older entry.
