# Statscraft for AI agents

Statscraft is built so that an AI agent with a shell can do the analysis work: everything is syntax, every result is JSON, every error has a line and a column, and a session keeps the data between steps. This page says how an agent should work, and gives a block of instructions you can paste into your agent.

## The loop

1. **Look at the data.** `statscraft describe FILE --json` gives the dictionary: variable names, types, labels, value labels, missing values, valid counts, the weight variable and multiple response sets. Read it before writing syntax, so names and codes are right.
2. **Look up the syntax.** `statscraft help` lists the supported commands; `statscraft help COMMAND` gives the syntax, the rules and an example. Use only what the help shows. A command that is not supported yet fails with a clear error.
3. **Write syntax to a file** (`job.sps`), one command per line or block, each ending with a period.
4. **Check it.** `statscraft check -s NAME --json job.sps` parses every command and runs it on a copy of the session's data, so it finds unknown variables and type errors too, and writes nothing.
5. **Run it in pieces.** `statscraft run -s NAME --json --lines FIRST-LAST job.sps` runs the commands in those lines and keeps the result in the session. The next range builds on it. Line numbers stay those of the file. The session keeps what one window would keep between runs: the data with its weight, split and filter, the named datasets, the macros (run the INCLUDE of a macro library once and call its macros in later runs), the file handles, the SET values, PRESERVE levels, the CD folder and where the random numbers stand. TEMPORARY, N OF CASES and scratch variables end with the run that gives them. `statscraft describe -s NAME --json` shows the kept state under `session`; the full list is in [the command line](command-line.md#sessions-and-line-ranges).
6. **Read the JSON.** `ok` says whether the run worked. `changes` names the variables added, changed and removed and the case counts before and after; `dataset` gives the size; `commands[].tables` holds the results with raw numbers; `commands[].warnings` says where a command did something other than asked.
7. **Fix errors where they point.** On failure `errors[]` gives `kind`, `message`, `line` and, for syntax errors, `column`, `expected` and often a `hint`. Fix that line and run the same range again. A failed run saves nothing, settings included, so a retry starts from the same state.

A short session:

```
statscraft describe tests/fixtures/survey_basic.sav --json
statscraft help RECODE
statscraft check -s work --json job.sps
statscraft run -s work --json --lines 1-1 job.sps
statscraft run -s work --json --lines 4-6 job.sps
statscraft run -s work --json -e "FREQUENCIES VARIABLES=age_group."
statscraft describe -s work --json
```

A syntax error tells the agent where and what:

```json
{"ok":false,"errors":[{"kind":"syntax","message":"unexpected end of command","line":1,"column":18,"expected":["a value","a variable","a function","'('"]}],"saved":false}
```

A misspelled command comes with a suggestion:

```json
{"ok":false,"errors":[{"kind":"syntax","message":"unknown command FREQUENCIS","line":1,"column":1,"hint":"did you mean FREQUENCIES?"}],"saved":false}
```

See [the command line](command-line.md#json-output) for every field.

Good habits:

- Use `--json` for every step and read the `ok` field first; the exit code is 0 on success and 1 on failure.
- Keep the whole analysis in one `.sps` file and run it by line ranges, so the file is the record of what was done and runs again from the top with `statscraft run job.sps`.
- Use a new session name for a fresh start, or delete `.statscraft/sessions/NAME/`.
- Numbers in the JSON are raw. Round them only when you report them.
- Add `--xlsx tables.xlsx` to a run when the person wants the tables in Excel.

## Working in the open window

When the person has `statscraft-gui` open, the agent can work in it instead of a file session: the session name `gui` sends the syntax to the window, which runs it like its own Run button. The person sees each table appear in Output and each command in the log, and can keep working in the window between the agent's runs.

```
statscraft run -s gui --json -e "GET FILE='tests/fixtures/survey_basic.sav'."
statscraft describe -s gui --json
statscraft check -s gui --json -e "COMPUTE x = age / 10."
statscraft run -s gui --json job.sps
```

The replies are the same JSON as for a file session, with `"session": "gui"`. Relative paths in the syntax are read from the folder where the agent runs `statscraft`. When no window is open, the reply is the error "no Statscraft window is running".

Because the person may also change the data in the window, the agent should run `statscraft describe -s gui --json` again before it relies on names or counts it saw earlier.

## Instructions to paste into your agent

```text
You can analyse data with Statscraft, a statistics program driven by the established
statistics command syntax (COMPUTE, RECODE, FREQUENCIES, CTABLES and so on). Use the `statscraft` command line in the shell.

- Start with `statscraft describe <file> --json` to learn the variables, labels and missing values.
- Look up syntax with `statscraft help` and `statscraft help <COMMAND>`. Use only commands and
  options that the help lists.
- Write the syntax to a .sps file. Before running, check it:
  `statscraft check -s work --json <file>.sps`.
- Run it in steps that keep the data between them:
  `statscraft run -s work --json --lines FIRST-LAST <file>.sps`, or
  `statscraft run -s work --json -e "<syntax>"` for a single command. The session keeps data,
  settings, macros and named datasets between runs; `statscraft describe -s work --json` shows them.
- Read the JSON. If "ok" is false, fix the line and column given in "errors" (use "expected" and
  "hint") and run the same lines again; a failed run changes nothing. Read "warnings" of every
  command and "changes" to confirm what happened to the data.
- If I have the Statscraft window open, use `-s gui` instead of `-s work`, so I see every result.
- Do not write data files unless I ask; nothing is saved to disk without a SAVE command.
- Report numbers from the "tables" in the JSON; do not compute them yourself.
```

## Safety of the live session

The window accepts syntax only from programs of the same user on the same computer:

- It listens on a random TCP port of `127.0.0.1` only, so other computers cannot reach it.
- At start it creates a random token and writes the address and the token to `statscraft/gui.json` in `$XDG_RUNTIME_DIR` on Linux, `%LOCALAPPDATA%` on Windows, or `~/.config` otherwise. The file is readable only by the user.
- Every request must carry that token; a request without it is refused. The command line reads the token from the file and sends it only to a loopback address.
- Requests larger than 64 MB are refused.
- When the window closes, it removes its file.

What this means in practice: any program running as you can drive the window, just as it could run `statscraft` itself. Syntax sent to the window can do what syntax can do: read files, and write files with `SAVE` or `AGGREGATE OUTFILE`. Treat an agent that can use `-s gui` like an agent that can run commands in your shell.
