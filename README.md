# Statscraft

English · [Español](README.es.md) · [简体中文](README.zh-CN.md)

Statscraft is a free, open-source statistics package for the social sciences, built AI-native from the first line. It is an alternative to SPSS, PSPP, Stata and SAS: it reads .sav files and runs your existing syntax, on Windows, macOS and Linux. Two programs written in Rust share one engine: the command line `statscraft` and the desktop window `statscraft-gui`.

[Download](https://statscraft.org/download.html) · [Documentation](https://docs.statscraft.org) · [Website](https://statscraft.org)

## What it does

- **Data files.** Reads and writes .sav and .zsav files with their labels, missing values and multiple response sets. Opens Excel workbooks and text files, and writes Excel, CSV and Stata files.
- **Data preparation.** Compute, recode, select, weight, aggregate, merge and restructure, with macros and the matrix language.
- **Custom tables.** Counts, percentages and means by groups, with totals, subtotals, multiple response sets and significance tests marked by letters.
- **Statistics.** Frequencies, crosstabs, means, t tests and analysis of variance; linear, logistic and many other regressions; general, generalized and mixed linear models; factor, cluster and tree analysis; survival analysis, forecasting, complex samples, Bayesian tests and power analysis.
- **Output.** Tables and charts to Excel, Word, PowerPoint, PDF and web pages. Charts go into the office programs as charts you can edit.
- **AI agents.** An MCP server, JSON output, sessions that keep the data between runs, and errors with line and column, so an agent can do the analysis while you watch it in the window.

The [command pages](docs/commands/README.md) list every command.

Statscraft is an independent project. It is not affiliated with or endorsed by the makers of other statistics software or the ArtCraft suite. Product names are trademarks of their owners.

Status: early development. Licensed under MIT.

## Documentation

The documentation is online at [docs.statscraft.org](https://docs.statscraft.org); the same pages are in `docs/`.

- [Getting started](docs/getting-started.md): from installing to a custom table in Excel, with a sample file.
- [Install and update](docs/install.md): every package, uninstalling and updates.
- [The window](docs/window.md): the data editor, syntax editor, output, menus and custom tables.
- [The command line](docs/command-line.md): every subcommand and flag, sessions, JSON output, exit codes, Excel export and the live window session.
- [Databases](docs/databases.md): store datasets in SQLite, PostgreSQL, MySQL, MariaDB or Dolt, and recode them there.
- [AI agents](docs/ai-agents.md): how an agent should work with Statscraft, and instructions to paste into yours.
- [Connect your AI agent](docs/connect.md): add Statscraft to the AI agents on your computer.
- [The MCP server](docs/mcp.md): the tools an agent app gets, sessions and safety.
- [Plugins](docs/plugins.md): programs that color and comment the cells of custom tables.
- [Commands](docs/commands/README.md): one page per command.
- [Changelog](CHANGELOG.md): what is in each release.

## Contributing

[CONTRIBUTING.md](CONTRIBUTING.md) explains how to build from source, run the tests and release a version. The [developer notes](docs/developers/README.md) and [Writing a plugin](docs/developers/plugins.md) hold the details behind the user guides.
