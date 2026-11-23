# Getting started

This page takes you from installing Statscraft to an Excel workbook with a custom table. You work in the window, with a sample file you download below. At the end you see the same work on the command line.

## Install

Download the package for your system from the [download page](https://statscraft.org/download.html). Each package holds both programs: the window and the `statscraft` command line.

**Windows.** Run `statscraft-X-windows-x86_64-setup.exe` (`X` is the version). It installs for your user only, without administrator rights, and adds Statscraft to the Start menu. If Windows warns about an unknown publisher, choose **More info**, then **Run anyway**.

**macOS.** Double-click `statscraft-X-macos-universal.tar.gz` to unpack it, and drag `Statscraft.app` to **Applications**. The first time you open it, macOS says it cannot verify the app: click **Done**, then open **System Settings > Privacy & Security** and click **Open Anyway** next to the message about Statscraft.

**Debian and Ubuntu.** Install the package with `sudo apt install ./statscraft_X_amd64.deb`. Statscraft is then in the applications menu.

**Other Linux.** Unpack the tarball into `~/.local`: `tar -xzf statscraft-X-linux-x86_64.tar.gz --strip-components=1 -C ~/.local`.

[Install and update](install.md) has the details: the portable Windows version, the command line on macOS, how to uninstall and how updates work.

## Open the sample file

Download the sample file [survey.sav](sample/survey.sav). It is an invented customer survey of 300 people, with labels on every variable and value.

1. Start Statscraft. An empty data window opens.
2. Choose **File > Open data...** and pick `survey.sav`.

The data window now shows the cases, one row per respondent. **Show value labels**, above the grid, switches between the codes and their labels.

## Look at the variables

Click the **Variables** tab. Each row is one variable, with its label, value labels and missing values. This page uses these:

| Variable | What it holds |
|---|---|
| `region` | North, South, East or West |
| `satisfaction` | overall satisfaction from 1 (very dissatisfied) to 5 (very satisfied); 9 means "Don't know" and is a missing value |
| `ch_web` to `ch_email` | five yes or no variables, one per channel the customer used |

The five channel variables also form a multiple response set, `$channels`: one question with several answers allowed. **Data > Define Multiple Response Sets...** shows it.

Double-click a label to change it. Every change runs a short command, which is how Statscraft keeps a record of your work (more on that [below](#save-the-syntax)).

## Make a frequency table

1. Choose **Analyze > Descriptive Statistics > Frequencies...**.
2. Click `region` in the list of variables, then click **Add** beside **Variables**.
3. Click **OK**.

An output window opens with the table:

| Region | Frequency | Percent | Valid Percent | Cumulative Percent |
|---|---:|---:|---:|---:|
| North | 75 | 25.0 | 25.0 | 25.0 |
| South | 62 | 20.7 | 20.7 | 45.7 |
| East | 81 | 27.0 | 27.0 | 72.7 |
| West | 82 | 27.3 | 27.3 | 100.0 |
| Total | 300 | 100.0 | | |

The log under the output shows the command that made it: `FREQUENCIES VARIABLES=region.`

## Build a custom table

A custom table puts several questions side by side and marks the differences that are statistically significant. This one shows satisfaction and the channels used, by region.

1. Choose **Analyze > Custom Tables...**.
2. In the list on the left, click `satisfaction`, and `$channels (set)` under **Multiple response sets**. Then click **Add** beside **Rows**.
3. Click `region`, then **Add** beside **Columns**.
4. Under **Summaries**, clear **Count** and check **Column %**.
5. Under **Significance tests**, check **Compare column proportions**.
6. Under **Letters of the column comparisons**, choose **In the main table, column keys**.
7. Click **OK**.

The output window shows the table. Each region has a column of percentages and, beside it, a column of letters:

| | North | (a) | South | (b) | East | (c) | West | (d) |
|---|---:|---|---:|---|---:|---|---:|---|
| **Overall satisfaction with the service** | | | | | | | | |
| Very dissatisfied | 4.3% | | 6.9% | | 15.1% | d | 2.6% | |
| Dissatisfied | 20.0% | | 8.6% | | 34.2% | b | 28.9% | b |
| Neutral | 15.7% | | 32.8% | | 35.6% | a | 38.2% | a |
| Satisfied | 37.1% | c d | 27.6% | | 12.3% | | 15.8% | |
| Very satisfied | 22.9% | c | 24.1% | c | 2.7% | | 14.5% | |
| **Channels used** | | | | | | | | |
| Website | 70.0% | | 71.2% | | 78.8% | | 76.3% | |
| Mobile app | 24.3% | | 27.1% | | 31.3% | | 36.8% | |
| Phone | 37.1% | | 59.3% | | 56.3% | | 56.6% | |
| Store | 22.9% | | 23.7% | | 23.8% | | 21.1% | |
| Email | 45.7% | | 44.1% | | 33.8% | | 35.5% | |

Each region has a key: a for North, b for South, c for East and d for West. A letter beside a percentage says that it is significantly higher than the percentage of the region with that key. For example, "c d" beside 37.1% says that more customers in the North are satisfied than in the East or the West. A note under the table names the test. [Custom tables](window.md#custom-tables) explains the dialog in full.

## Export to Excel

1. Choose **File > Export...**.
2. Under **Format**, choose **Excel workbook (.xlsx)**, and choose **One sheet per table** or **All tables on one sheet**.
3. Click **Export...** and choose where to save the workbook.

Each table keeps its layout in Excel. Numbers stay numbers, so you can compute with them, and the significance letters stay next to them as text. [Export to Excel](window.md#exporting-all-output) lists every choice.

## Save the syntax

Everything you did ran as syntax: the commands of the Statscraft language. You can save it and run it again later, on this file or on next month's data.

1. Look at the log under the output.
2. Click **Copy syntax**. That copies every command of your session.
3. Choose **File > New > Syntax**, paste with Ctrl+V, and choose **File > Save syntax as...**. Save it as `first-table.sps`.

The file looks like this (your `GET FILE` line has the full path of your file):

```
GET FILE='C:\Users\anna\Documents\survey.sav'.
DATASET NAME DataSet1.
FREQUENCIES VARIABLES=region.
CTABLES
  /TABLE satisfaction [C] [COLPCT.COUNT PCT40.1] + $channels [COLPCT.COUNT PCT40.1] BY region [C]
  /COMPARETEST TYPE=PROP ALPHA=0.05 ADJUST=BONFERRONI MERGE=YES STYLE=SIMPLE.
```

To run it again, open it with **File > Open syntax...** and click **Run all**. Instead of **OK**, every dialog also has **Paste**, which puts its syntax into the syntax window without running it.

## The same on the command line

The `statscraft` command line runs the same syntax without the window. In a terminal, go to the folder with `survey.sav` and `first-table.sps`:

```
statscraft describe survey.sav
statscraft run first-table.sps --xlsx tables.xlsx
```

`describe` lists the variables. `run` prints every table as text and also writes them to `tables.xlsx`. [The command line](command-line.md) describes every option.

## Work with an AI agent

An AI agent can do the same steps for you: look at the variables, write the syntax, run it and read the results. Connect your agent once with **Edit > Settings > AI agents** in the window. [Connect your AI agent](connect.md) shows how, and [Statscraft for AI agents](ai-agents.md) shows how an agent works with Statscraft.

## Next

- [The window](window.md): the data, syntax and output windows, and every menu.
- [Commands](commands/README.md): one page per command, with its syntax and an example.
- [The command line](command-line.md): every subcommand and option.
