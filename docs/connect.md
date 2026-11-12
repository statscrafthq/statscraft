# Connect your AI agent

Statscraft has an MCP server built in: `statscraft mcp`. An AI agent that speaks MCP can then look at your data, look up syntax and run it through Statscraft, without a shell. Your data stay on your computer.

One command adds the server to every agent installed on your computer:

```
statscraft connect
```

It prints which agents it found, connects each of them and says what to restart. Run it again at any time: an agent that is already connected stays as it is.

Claude Desktop and Claude Code have a way without a terminal too: [the bundle](#claude-desktop-double-click-the-bundle) and [the plugin](#claude-code-the-plugin). To teach an agent how to work with Statscraft, add [the skill](#teach-your-agent-the-skill).

## The commands

| Command | What it does |
|---|---|
| `statscraft connect` | connects every agent it finds |
| `statscraft connect AGENT` | connects one agent |
| `statscraft connect --list` | shows which agents are installed and connected, and changes nothing |
| `statscraft connect --remove` | takes Statscraft out of every agent that has it |
| `statscraft connect --remove AGENT` | takes it out of one agent |

Add `--json` to any of them for output an agent can read. The exit code is 1 when an agent could not be connected.

The agents are `claude-desktop`, `claude-code`, `codex` (also `chatgpt`), `cursor`, `copilot` (also `vscode`), `gemini` and `antigravity`.

## What it changes

Every agent gets one entry named `statscraft`. It starts the Statscraft program you ran `connect` with, by its full path, with the argument `mcp`. No shell is involved, and nothing depends on PATH. If you move or reinstall Statscraft in another folder, run `statscraft connect` again.

| Agent | What changes |
|---|---|
| Claude Desktop | `mcpServers.statscraft` in `claude_desktop_config.json`: on Windows in `%APPDATA%\Claude\` and, for the Store version, in `%LOCALAPPDATA%\Packages\Claude_...\LocalCache\Roaming\Claude\`; on macOS in `~/Library/Application Support/Claude/`; on Linux in `~/.config/Claude/` |
| Claude Code | runs `claude mcp add --scope user statscraft -- PATH mcp`, so Claude Code keeps the entry itself |
| ChatGPT desktop and Codex | the table `[mcp_servers.statscraft]` in `~/.codex/config.toml` (or under `CODEX_HOME`) |
| Cursor | `mcpServers.statscraft` in `~/.cursor/mcp.json` |
| VS Code with Copilot | `mcpServers.statscraft` in `~/.copilot/mcp-config.json` (or under `COPILOT_HOME`) |
| Gemini CLI | `mcpServers.statscraft` in `~/.gemini/settings.json` |
| Antigravity | `mcpServers.statscraft` in `~/.gemini/config/mcp_config.json` |

On Windows, `~` is your user folder, for example `C:\Users\anna`.

How the files are treated:

- Only the `statscraft` entry changes. Every other setting and every other server stays, in the same order, with the same indentation and line endings. Comments in `config.toml` stay too.
- Before the first change of a file, a copy of it is saved next to it as `FILE.statscraft-backup`. Later changes keep that first copy.
- A file is written to a temporary file first and then put in place, so an agent never reads half a file.
- A file that is not plain JSON, for example one with comments, is not touched. You get a message that names it, and you can add the entry by hand as shown below.
- An agent counts as installed when its settings folder exists (for Claude Code: when the `claude` command is on PATH). A missing settings file is created only for an installed agent.

## Restart the agent

An agent reads its settings when it starts. After connecting:

| Agent | What to do |
|---|---|
| Claude Desktop | quit it completely, also from the tray or menu bar, and open it again; closing the window is not enough |
| Claude Code | start a new session |
| ChatGPT desktop, Codex | quit and open it again |
| Cursor, VS Code, Antigravity | quit and open it again |
| Gemini CLI | start a new session |

Then ask the agent what tools Statscraft gives it, or ask it to describe a data file.

## Undo

`statscraft connect --remove` takes the `statscraft` entry out again and leaves everything else as it is. To go back to the file exactly as it was before the first change, put the `.statscraft-backup` copy back in its place. Settings you changed in the meantime are then lost too.

## Claude Desktop: double-click the bundle

Every release has an MCP bundle for each system on the [Releases page](https://github.com/statscrafthq/statscraft/releases):

- `statscraft-X-windows-x86_64.mcpb` for Windows
- `statscraft-X-macos-universal.mcpb` for macOS, Apple silicon and Intel
- `statscraft-X-linux-x86_64.mcpb` for Linux

Download the one for your system and double-click it, or drag it onto Claude Desktop's **Settings**, **Extensions**. Claude Desktop shows the tools Statscraft gives and asks you to install it. That is all: no terminal and no settings file.

The bundle carries its own copy of the `statscraft` program, so it works even where Statscraft is not installed. That copy does not update with `statscraft update`; install the bundle of the new release instead. The window, when you have it, still shows everything the agent runs.

Known limits: the macOS program is not signed yet, so macOS may stop it; if Claude Desktop says the extension cannot start, use `statscraft connect` instead. Claude Desktop for Linux is a beta, and bundles have not been tried there.

## Claude Code: the plugin

In Claude Code, type:

```
/plugin marketplace add statscrafthq/statscraft
/plugin install statscraft@statscraft
```

and start a new session. The plugin adds the MCP server and [the skill](#teach-your-agent-the-skill). It starts the server as `statscraft mcp`, so `statscraft` must be on your PATH. The Windows installer, the .deb package and the Linux tarball put it there; the portable Windows .zip and the macOS download do not, until you add it as [Getting started](getting-started.md#install) shows. Check with `statscraft --version` in a new terminal. Without PATH, run `statscraft connect claude-code` with the full path of the program: the skill from the plugin still works with that entry.

If Claude Code was connected with `statscraft connect` before, it now has the server twice. `statscraft connect --remove claude-code` takes out the older entry.

## Teach your agent: the skill

The skill `statscraft-analysis` tells an agent how to work with Statscraft: look at the data first, look up each command, check, run in small steps, read every reply, report the numbers from the result tables and never compute them itself, and write no file unless asked. It follows the [Agent Skills](https://agentskills.io) format, so many agent apps read it. It is one folder with one `SKILL.md`, in [`plugins/statscraft/skills/statscraft-analysis`](https://github.com/statscrafthq/statscraft/tree/main/plugins/statscraft/skills/statscraft-analysis) of the repository.

- **Claude Code**: it comes with the plugin.
- **Claude Desktop and claude.ai**: download `statscraft-X-skill.zip` from the [Releases page](https://github.com/statscrafthq/statscraft/releases) and upload it as a skill in the settings.
- **ChatGPT desktop and Codex**: ask Codex `$skill-installer install https://github.com/statscrafthq/statscraft/tree/main/plugins/statscraft/skills/statscraft-analysis`, then restart it.
- **Other agents** that read Agent Skills: unpack the zip into their skills folder, so that it holds `statscraft-analysis/SKILL.md`.

The skill uses the MCP tools when they are there, and the `statscraft` command line when the agent has only a shell.

## Connect by hand

Prefer to edit the files yourself? Use the full path of your `statscraft` program. `statscraft connect --list --json` prints it under `server`; `where statscraft` on Windows and `which statscraft` elsewhere show it too. The Windows installer puts it in `C:\Users\NAME\AppData\Local\Programs\Statscraft\statscraft.exe`. The examples below use `/usr/bin/statscraft`. In JSON, every backslash of a Windows path is written twice: `"C:\\Users\\anna\\AppData\\Local\\Programs\\Statscraft\\statscraft.exe"`.

**Claude Code**, one line in a terminal:

```
claude mcp add --scope user statscraft -- /usr/bin/statscraft mcp
```

**Claude Desktop** (`claude_desktop_config.json`), **Gemini CLI** (`~/.gemini/settings.json`) and **Antigravity** (`~/.gemini/config/mcp_config.json`): add the entry under `mcpServers`, next to any servers already there.

```json
{
  "mcpServers": {
    "statscraft": {
      "command": "/usr/bin/statscraft",
      "args": ["mcp"]
    }
  }
}
```

**Cursor** (`~/.cursor/mcp.json`):

```json
{
  "mcpServers": {
    "statscraft": {
      "type": "stdio",
      "command": "/usr/bin/statscraft",
      "args": ["mcp"]
    }
  }
}
```

**VS Code with Copilot** (`~/.copilot/mcp-config.json`):

```json
{
  "mcpServers": {
    "statscraft": {
      "type": "stdio",
      "command": "/usr/bin/statscraft",
      "args": ["mcp"],
      "tools": ["*"]
    }
  }
}
```

**ChatGPT desktop and Codex** (`~/.codex/config.toml`):

```toml
[mcp_servers.statscraft]
command = "/usr/bin/statscraft"
args = ["mcp"]
```

ChatGPT in the browser cannot start programs on your computer, so it cannot use Statscraft this way.

## For agents

An agent with a shell can connect itself. Run `statscraft connect --list --json` first, then `statscraft connect AGENT --json` for the agent you are, and tell the person what the reply's `restart` field says. [Statscraft for AI agents](ai-agents.md) describes how to work once connected.
