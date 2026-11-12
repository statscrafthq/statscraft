---
name: statscraft-analysis
description: Analyze survey and statistics data (.sav, .zsav, .sps) with Statscraft, through its MCP tools or the statscraft command line. Use when the person asks for frequencies, crosstabs, custom tables, means, tests, recodes or other work on a data file, or mentions Statscraft or statistics command syntax.
license: MIT
---

# Analysis with Statscraft

Statscraft runs statistics command syntax (GET FILE, COMPUTE, RECODE, FREQUENCIES, CROSSTABS, CTABLES and many more) on data files that stay on the person's computer. You write the syntax; Statscraft computes every number.

## Which way to call it

- **MCP tools** (preferred): `describe`, `list_commands`, `command_help`, `check_syntax`, `run_syntax`, `window_status`. They appear when the Statscraft MCP server is connected.
- **Command line**, when you have a shell but no tools: `statscraft describe FILE --json`, `statscraft help [COMMAND]`, `statscraft check -s NAME --json FILE.sps`, `statscraft run -s NAME --json FILE.sps` (add `--lines A-B` for a range, or `-e "SYNTAX"` for inline syntax).
- Neither available: tell the person to install Statscraft and run `statscraft connect` once, then start a new conversation. See https://docs.statscraft.org/connect.html.

Give absolute paths for data files, in tool arguments and in the syntax.

## The loop

1. **Know the data before writing anything.** Call `describe` on the file. Note the exact variable names, which codes carry value labels, the user-missing values, the weight variable and any multiple response sets. Do not guess names or codes.
2. **Look the commands up.** `list_commands` shows what Statscraft supports; `command_help` for each command you plan to use shows its subcommands, keywords and defaults. Write only what the help shows. A command or keyword that is not there will fail.
3. **Check first.** `check_syntax` parses the syntax and tries it on a copy of the data, so it also catches unknown variables and type mismatches. It never keeps or writes anything.
4. **Run in small steps.** `run_syntax` keeps the result in a session, so a later call builds on an earlier one: open the file once, then transform, then analyze. Data, weight, filter, settings, macros and named datasets all carry over. Use the same `session` name throughout, or leave it out to get the default.
5. **Read every reply.** Look at `ok` first. When it is false, `errors` gives the line, the column, what was expected and often a hint; fix exactly that spot and send the same syntax again. A failed run changes nothing, so retrying is safe. When it is true, still read the `warnings` of each command (they say where Statscraft did something other than asked) and `changes` (variables added, changed or removed, cases before and after).
6. **Report from the tables.** Every statistic you give the person comes from the `tables` in the reply. Never calculate a percentage, mean, test or total yourself, not even to fill a gap; run another command instead. The numbers are raw: round only when you write them up, and say what the base is (weighted or not, which cases were excluded).

## The open window

If the person has the Statscraft window open (`window_status` tells you), calls without a session go to it: they see every table appear in the Output tab and every command in the log. They can change the data there too, so call `describe` again before relying on names or counts from earlier.

## Files

Nothing is written to disk unless the syntax asks for it: SAVE, XSAVE, AGGREGATE OUTFILE, OUTPUT EXPORT, or the `xlsx` argument of `run_syntax`. Write files only when the person asks, and never overwrite their source data file; save to a new name.

## A short example

The person asks for satisfaction by region in `/home/anna/data/clinic_visits.sav`.

1. `describe` with `file` = that path: finds `region` (labels 1 North, 2 South, 3 East) and `satisf` (1 to 5, 9 = no answer as user-missing).
2. `command_help` for CROSSTABS.
3. `check_syntax` with:
   ```
   GET FILE='/home/anna/data/clinic_visits.sav'.
   CROSSTABS TABLES=satisf BY region /CELLS=COUNT COLUMN.
   ```
4. `run_syntax` with the same text, then read `ok`, the warnings and the table.
5. Report the column percentages as the table gives them, and mention that code 9 was left out as missing.
