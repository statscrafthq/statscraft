# The window

This page is for analysts who work in the Statscraft window: the data editor, the syntax editor and the output, with their menus and dialogs. [Getting started](getting-started.md) walks through a first table.

The window (`statscraft-gui`) runs on the same engine as the command line. It has three kinds of window, all on one session:

- a [data window](#data-windows) for each open dataset, with the **Data** and **Variables** tabs;
- a [syntax window](#syntax-windows) for each syntax file;
- an [output window](#output-windows) for each output document.

The session has one active dataset, and every command, from any window, works on it. Every change and every result in the windows comes from syntax (see [Syntax is the only command layer](#syntax-is-the-only-command-layer)).

Started without a file, the program opens one empty data window, a place to open files from.

## Opening files

- From a terminal, `statscraft-gui survey.sav job.sps` opens a data file and a syntax file. Both are optional.
- File > Open data opens a .sav or .zsav file at once, in a data window of its own. The empty data window takes the first file.
- An Excel workbook (.xlsx, .xlsm, .xls) or a text file (.csv, .txt, .tab, .dat) opens the Open Data dialog first. It writes `GET DATA`, with `DATASET NAME` after it as for a .sav file:
  - for a workbook: the sheet, whether the first row holds the variable names, and a range such as A1:F200 (empty reads the whole sheet);
  - for a text file: the delimiter (comma, semicolon or tab; the one the first line uses most is chosen), and whether the first line holds the names, with that line shown. Every variable is read as `AUTO`.
- After an Open Data run, File > Save data asks for a new .sav file.
- File > Open output opens a Statscraft output file (.scout) in an output window of its own, by running `OUTPUT OPEN FILE='...'.`. See [saving output](#saving-output).
- File > Recent files lists the ten files opened or saved last.
- Files dropped on the window from the file manager open too. A .sav or .zsav file opens as data, a workbook or text file in the Open Data dialog, a .sps file in a syntax window and each .scout file in an output window. One data file and one syntax file open at a time; the status bar names the files left out.
- The installer on Windows, and the packages for macOS and Linux, let a double-click on a .sav, .zsav, .sps or .scout file open it in Statscraft. When Statscraft runs already, the file opens in that window and its session, and the window comes to the front; starting Statscraft from the menu without a file still opens a new window.

## Exporting data

File > Export data writes `SAVE TRANSLATE`. The file name chosen sets the type: an Excel workbook (.xlsx), a CSV file (.csv), tab-delimited text (.txt) or a Stata file (.dta). The dialog offers:

- a first row of variable names, variable labels or none;
- value labels instead of values;
- user-missing values as empty;
- the variables to export (none chosen: all, in the order of the dataset);
- for a Stata file, instead of the first row and value labels: the Stata release that reads it (8 to 14, default 14) and whole strings (`EDITION=SE`, on by default; off, strings are cut at 80 bytes). Value labels go into the file as Stata value labels.

An existing file is replaced once the file dialog has asked.

## Data windows

Each dataset of the session that holds data has a data window. The title names the file and the dataset, such as `survey.sav [DataSet1]`, and the window of the active dataset says `(active)`.

- Opening a data file opens its window. A file opened from the window gets the next free dataset name (DataSet1, DataSet2 and so on), so the data open before stays open.
- A dataset made by syntax gets a window too: `DATASET COPY`, `DATASET DECLARE` once a command has written data to it, and a `GET` typed in syntax without `DATASET NAME`. A dataset that `DATASET CLOSE` closes loses its window, and `DATASET NAME` keeps the window under the new name.
- A click anywhere in a data window, its menus included, makes its dataset active. That runs `DATASET ACTIVATE name.`, which shows in the log like any run. A click in the window of the active dataset runs nothing.
- Dialogs opened from a data window work on its dataset, which is active by then.
- Each window keeps its own tab, selection and find bar. Grid edits, right-click menus and Delete work in every data window.
- The data a typed `GET` reads without `DATASET NAME` has no name. Such a dataset closes when another one becomes active (a click in another data window, the Active list, opening a file). When it holds unsaved changes, a question comes first: **Save** saves it and goes on, **Don't save** goes on without it, **Cancel** keeps everything as it is.
- File > Close or the window's close button closes the dataset. When its data is unsaved, a question offers **Save**, **Don't save** and **Cancel**. Closing runs `DATASET CLOSE name.`. When the dataset is the active one, another open dataset is activated first, as `DATASET CLOSE` of the active dataset only takes its name away; when it is the last one, `NEW FILE.` follows.
- Closing the last data window does not end the program while other windows are open. Opening a file opens a data window again.

## The Data tab

The cases, in a grid that draws only the visible cells.

### What the grid shows

- **Show value labels**, above the grid, switches value labels on and off.
- Each column draws its values and labels with the variable's alignment. When the file sets none, numbers go to the right and strings to the left.
- A value or label too wide for its column is cut short with an ellipsis and keeps its alignment.
- A system-missing value shows as a small gray dot. An empty string shows as nothing.
- While `FILTER BY` is in force, the row number of every case the filter leaves out has a diagonal stroke through it. The case stays in the grid.
- A column is as wide as the variable's display width (the width a .sav file keeps for each variable). Drag the border at the right of its name to change it. That runs `VARIABLE WIDTH var (n).`, so the width is saved with the data.

### Editing

- Double-click a cell or press Enter to change it. Enter commits, Escape cancels.
- Delete with whole columns selected asks to delete those variables.
- Delete with cells selected clears them, for example `IF (RANGE($CASENUM, 2, 4)) age = $SYSMIS.` (a blank for a string). Ctrl+Z brings them back.

### Keys and selection

- Arrows, Page Up and Page Down, Home and End move the selection.
- Ctrl with an arrow goes to the first or last row or column. Ctrl+Home and Ctrl+End go to the first and last cell.
- Tab and Shift+Tab move to the next and previous cell, committing an edit on the way.
- Shift with these keys, or Shift+click, extends the selection to a block.
- A click on a row number or a column name selects the whole case or variable. Shift+click selects a range, Ctrl+click adds or removes one.
- Dragging across cells selects the block. Dragging across column names or row numbers selects the variables or cases in between.
- Ctrl+C copies the selection as tab-separated text, with the variable names as the first line, for a spreadsheet or a text editor. One cell copies as its text alone. More than two million cells are not copied.

## The Variables tab

One row per variable, with these columns: name, type, format, width, decimals, label, value labels, missing values, columns (the display width, as wide as the Data tab shows it), alignment, measure and role. A variable without an alignment or role in the file shows numbers to the right, strings to the left and the role Input.

### Changing a field

Double-click a field to change it, or press Enter to change the field at the cursor.

- **Type and format.** The type (Numeric, Date or String) and the format open a small dialog. It offers only formats that exist: numbers (plain, comma, dot, dollar, percent, scientific, leading zeros), dates and times, and strings. Width and decimals are numbers within that format's limits. The dialog runs `FORMATS`, or `ALTER TYPE` for a new type or string width. A new type drops value labels and missing values, and the dialog says so.
- **Value labels.** An editor with one row per code and label, to change, add and remove. It runs `VALUE LABELS` with the whole set.
- **Paste many**, in the value labels editor, is a text area for many lines, each a code and a label. A tab, semicolon, equals sign or comma separates them (the first of these that the text holds). It either updates the set (`ADD VALUE LABELS`: new codes are added, the labels of existing codes change) or replaces it (`VALUE LABELS`).
- **Missing values.** An editor for up to three values, or for numbers a range and one value more (LO and HI stand for the lowest and highest value). It runs `MISSING VALUES`.
- When the variable's row is one of several selected rows, the value labels and missing values editors can give the same labels or missing values to the other selected variables of its type.
- **Columns** runs `VARIABLE WIDTH`, **alignment** `VARIABLE ALIGNMENT` and **role** `VARIABLE ROLE`. Alignment and role are chosen from a list, like the measure.

### Keys and selection

- The keys, mouse selection and Ctrl+C work as in the Data tab.
- Delete deletes the variables whose row numbers are selected, with `DELETE VARIABLES`, after a question. Edit > Undo last change brings them back.
- Drag a column border to resize it. These widths are a window setting.

## Finding a variable

Ctrl+F in the Data or Variables tab opens a find bar.

- Typing goes to the first variable whose name or label holds the text, in any case.
- Enter or F3 goes to the next one. Escape closes the bar.
- The Variables tab goes to the variable's row, the Data tab to its column.

## Right-click menus

A right click opens a menu over what it lands on. The top of the menu names what it acts on.

- **On a column name**, or on the selected columns when it lands inside them:
  - Sort ascending or descending by them (`SORT CASES`).
  - Frequencies or Descriptives (numeric variables only), run at once.
  - Rename.
  - Insert variable before: a new numeric variable `new_1`.
  - Copy, Delete.
- **On a cell**:
  - Copy, Edit, Clear value.
  - Only cases where the variable has this value: a filter that leaves the other cases out of analyses (`FILTER BY`). It compares the stored value, not the rounded one shown.
  - Stop filtering.
  - Keep only cases where...: opens Select Cases to delete the other cases.
  - Sort ascending or descending by the variable.
- **On a row number**: Copy, or Delete case.
- **In the Variables tab**, the menu on row numbers is the one of the column names. On a cell it starts with Edit for that field.

How the items behave:

- Every item runs syntax like any other run, so it shows in the log and Ctrl+Z takes a change back.
- Items that run syntax wait while a run is going on. The analyses and sorting are off when there are no cases.
- Some items first show a question with the syntax: deleting variables or a case, sorting while a split is on (sorting ends the split), and Frequencies of more than 100 variables. Only a click on the button runs it; Escape cancels.

## Syntax windows

Each syntax window holds one syntax file, or new syntax, in an editor with line numbers.

- File > New > Syntax opens a new window, named Syntax1, Syntax2 and so on until it is saved. File > Open syntax, Recent files and a dropped .sps file open the file in a new window; a file that is open already brings its window to the front.
- Save syntax and Save syntax as save the window they are chosen in.
- File > Close or the window's close button closes it. When its text is unsaved, a question offers **Save**, **Don't save** and **Cancel**.
- The toolbar has **Run all**, **Run selection** and the **Active** list. The Active list shows the datasets of the session, with the active one chosen. Choosing another runs `DATASET ACTIVATE name.`, which shows in the log like any run.
- Every window runs on the same session and the same active dataset. A run started while another goes on waits its turn, and the status bar says how many runs wait.
- **Paste** in a dialog or the output puts the syntax at the end of the designated syntax window: the one opened last, or the one chosen with Window > Designate window. With no syntax window open, a new one opens.
- **The Agent window.** Syntax that an AI agent or `statscraft run -s gui` runs in this window is added to a syntax window named Agent when it works, each run under a comment line with its time. It opens with the first such run, and again after you close it. Edit it, run lines from it and save it like any syntax window; pasted syntax never goes there. An agent can also read and edit your other syntax windows and run them, and you see each change in the editor as it happens (Ctrl+Z there takes it back). See [the MCP server](mcp.md#syntax-windows).

### The editor

- Colors mark command names, subcommands, keywords, strings, numbers, comments, macro words (`!name` and the words of the macro language) and the period that ends a command.
- A macro defined with DEFINE, in the editor text or in an earlier run of the session, is colored as a macro where it is called, also where a command would stand.
- **Run all** runs the whole text. **Run selection** (Ctrl+Enter) runs the commands on the selected lines. On a line with no command (such as the empty line under a command just typed) it runs the command above.
- An error shows at its line and column.
- Long lines wrap at the window edge. Edit > Settings > Editor > Wrap long lines turns that off.

### Completion

While you type, a list offers what fits at the cursor:

- command names at the start of a command (abbreviations such as FRE or VAL LAB find the full name);
- subcommands after `/`, and the keywords of the command;
- the variables of the open data, with their labels;
- function names where an expression goes;
- the macros defined with DEFINE in the text or the session, where a command goes and inside commands;
- after `!`, the words of the macro language.

Ctrl+Space opens the list at any point. Arrows choose, Enter or Tab puts the choice in, Escape closes the list.

### Help in the editor

- A command in the list, or a command name under the mouse, shows its summary and syntax.
- F1 opens the full help of the command at the cursor in a small window.

## The Window menu

Every window has the same menu bar. The Window menu lists the open windows; a click brings one to the front. The designated syntax window and the designated output window are marked `+`. **Designate window** makes the syntax or output window it is chosen in the designated one of its kind; for an output window it runs `OUTPUT ACTIVATE name.`.

The program ends when the last window closes, whatever its kind.

## Output windows

Each output document of the session has an output window, named after the document (Document1, Document2 and so on, or the name `OUTPUT NAME` gave). The tables of every run go to the designated document, so they appear in its window.

- The first tables of a session start Document1, and its window opens.
- File > New > Output runs `OUTPUT NEW.`: a new window opens and becomes the designated one.
- Window > Designate window, in an output window, runs `OUTPUT ACTIVATE name.`. `OUTPUT NEW`, `OUTPUT ACTIVATE` and `OUTPUT NAME` typed in syntax change the windows the same way.
- File > Close or the window's close button runs `OUTPUT CLOSE name.`. When the window holds output that is not saved, it asks first: Save, Don't save or Cancel. The next tables after the designated window closed start a new document and window.

Each output window shows its tables beside a navigator that groups them by run and command. A click on a run in the navigator folds it away.

### What the output shows

Everything the output shows is an element in the navigator. Under each command come its heading (the line such as "CTABLES, line 36" above its first table) and its tables and charts, each with its title and, when it has them, its notes.

- Each element has a box beside it in the navigator. Clear the box to hide that one element, check it to show it again. A table hidden by `OUTPUT MODIFY` starts with its box clear.
- Edit > Settings > Grids and output > **Show in the output** shows or hides whole kinds: command headings (off by default), titles, tables, notes under tables and charts (on by default).
- An element shows when its kind is on and it is not hidden itself. A table or chart that does not show takes its title and notes with it, and a command whose tables all are hidden has no heading.
- An element that does not show is greyed and in italics in the navigator. The output closes up around it at once.
- A chart's notes are its subtitle and footnote.
- Hiding is a matter of the window: OUTPUT SAVE keeps everything, with the hidden flags `OUTPUT MODIFY` set. A command heading hidden by `OUTPUT MODIFY /SELECT HEADINGS ... VISIBLE=NO` is a cleared Command headings box of that command only; its tables stay.

### How tables look

- Each table has its multi-level header. Its caption and notes follow it.
- Columns are as wide as their numbers, which align right.
- Long row labels and column headers wrap between words.
- A row label repeated under the same outer labels is shown once, beside all its rows.

### Selecting tables

- A run selects nothing, so its tables show without a frame.
- Click a table, or its name in the navigator, to select it. Ctrl+click selects more; Ctrl+A or **Select all** selects all.
- Click a selected table again, press Escape or click beside the tables to clear the selection.
- The syntax of a selected table's command shows under the navigator, exactly as it ran, with **Copy**, **Paste** (into the designated syntax window) and **Run again**. Pointing at a command in the navigator shows its syntax too.

### Working with tables

The buttons over the tables act on the selected tables. A right click on a table offers the same.

- **Copy** (Ctrl+C) puts the tables on the clipboard with their layout, for a word processor or spreadsheet, and as plain text beside it.
- **Copy as text** gives tab-separated text.
- **Delete** removes them from the window and from its output document, so OUTPUT SAVE and OUTPUT EXPORT leave them out too. The log shows the deletion as a comment line. While a run is going, Delete waits: the status bar asks to try again when it ends.
- **Export...** opens the Export dialog (below) set to only them.
- **Clear output** deletes every table, the same way.

Copies and exports (web page, PDF, Excel, from these buttons and from the File menu) take only what the output shows: no hidden tables, and no headings, titles or notes that do not show. A table whose title does not show has no title row in Excel, and its sheet and PDF bookmark keep its name. With **Hidden items too** in the Export dialog on, they take everything; the box is remembered, and copies follow it too.

### Charts

A chart shows in the output as a picture, as wide as the window up to about 760 pixels, on the same paper as the tables.

- Bar, pie, line, area and scatter charts and histograms are drawn, with error bars where the chart has them.
- So are the statistical plots: boxplots, Q-Q and P-P plots, ROC curves, the dendrogram, the icicle plot, the tree diagram and the territorial map.
- A stem-and-leaf display and a plot in three dimensions show as their data table.

A right click on a chart offers, besides Copy, Copy as text and Delete:

- **Show data**: the chart's numbers as a table in its place; **Show chart** brings the picture back.
- **Save chart as PNG...** and **Save chart as SVG...**: the picture as a file, 640 pixels wide (a PNG has twice the pixels), on the paper the output shows.

### Charts in the office programs

Bar, pie, line and area charts go into PowerPoint, Word and Excel as real charts of those programs. Their numbers sit in a workbook inside the chart, so **Edit Data** opens them. They have no colors of their own, so they take the theme of the presentation or document. A statistical plot, such as a histogram or a boxplot, goes in as a picture.

A right click on a drawn chart offers:

- **Open in PowerPoint**: a presentation with one slide, the chart's title as the slide title.
- **Open in Excel**: a workbook with the chart's data table and the chart beside it.
- **Open in Word**: a document holding the chart.

Each writes the file to a temporary folder and opens it with the program the system uses for that type of file, so it may be another office suite. Save it from there to keep it.

**Copy** on a single selected chart puts it on the Windows clipboard as an editable chart, with its data as tab-separated text and its picture as a PNG beside it, so a program that takes only pictures pastes the picture. Pasted into PowerPoint or Word it becomes their own chart, in the theme of the slide or document, and Edit Data opens its numbers. Excel pastes such a chart without its data, as it does with a chart copied in PowerPoint; use Open in Excel instead. On macOS and Linux, Copy puts the chart's data table on the clipboard, as for any table.

### Saving output

The output of a window can be kept as a Statscraft output file (.scout) and opened again later, with each command's syntax, warnings, tables and charts.

- File > Save output writes the document of the output window it is chosen in (elsewhere, of the designated output window) to the file it came from or was saved to last; the first time, it asks for a file. File > Save output as always asks. Both run `OUTPUT SAVE NAME=name OUTFILE='...'.`, so the step is in the log.
- File > Open output runs `OUTPUT OPEN FILE='...'.`. The file opens as a new document in a window of its own, as one run named after the file, and becomes the designated one: the next tables join it, and saving writes old and new together.
- The window keeps track of what is saved. A window whose output changed since it was opened or saved asks before it closes, and so does the program before it ends.
- A .scout file is plain JSON, one table row per line, so any text tool or AI agent can read it. See [the command line](command-line.md#output-files).

### Exporting all output

File > Export... opens the Export dialog for the output window; chosen in a data or syntax window, it exports the designated output window. The dialog asks for the **Format**, for an Excel workbook whether to put **One sheet per table** or **All tables on one sheet**, which **Items** (all items of the output, or only the selected tables) and whether to take **Hidden items too**. **Export...** then asks where to save. The choices stay for the next export.

The command line (`statscraft export`, see [the command line](command-line.md#export)) and AI agents (the MCP tool `export_output`) export the designated output window the same way, with the same choices, while the window is open.

- An Excel workbook is laid out as `--xlsx` does: a chart as its data table with the editable chart, or the picture of a statistical plot, beside it. Plugin comments become cell notes; a plugin's own settings decide whether it writes comments.
- A web page holds the tables, each drawn chart as a PNG picture inside the page.
- A PDF file has A4 pages, as OUTPUT EXPORT /PDF writes it: tables as text, charts as vector drawings, and bookmarks for the commands and their items.
- From syntax, OUTPUT EXPORT writes the tables and charts of an output document to a workbook, a Word document, a presentation, a web page, text or a CSV file.

## Custom tables

**Analyze > Custom Tables...** builds a table of counts, percentages or means by groups, and can mark the groups that differ significantly. Most survey reports are made of such tables. The dialog writes one [CTABLES](commands/ctables.md) command.

For example, with the [sample file](getting-started.md#open-the-sample-file): put `satisfaction` in Rows and `region` in Columns, check **Column %**, check **Compare column proportions**, and click **OK**. The table shows, for each region, how many customers gave each answer, and letters mark the regions that differ. [Getting started](getting-started.md#build-a-custom-table) shows the result.

### Building the table

The list on the left holds the variables of the active dataset, and under **Multiple response sets** the sets of the file. Click one or more names to pick them, then click **Add** beside a box. Click a name in a box to take it out again.

- **Rows (categories or sets)**: the categories of these variables and sets make the rows. Several are stacked, one below the other.
- **Columns (categories or sets)**: the groups you compare, side by side. Several are stacked side by side.
- **Layers (a table for each category)**: a separate table for each category, for example one table per region.
- **Means of (scale variables, in the rows)**: scale variables, such as age or spend. Their mean shows in a row of their own.

The syntax at the bottom of the dialog changes as you go. **OK** runs it, and **Paste** puts it in the syntax window, so you can change it or run it again later on new data.

### Counts and percentages

Choose what each cell shows under **Summaries**:

- **Count**: the number of cases, weighted when a weight is on.
- **Column %**: the percentage of each column. Each column adds up to 100%, so you compare the groups in the columns.
- **Row %**: the percentage of each row.
- **Mean**: the mean of the variables under **Means of**.
- **Totals**: a total row and a total column.
- **Effective count**, with an **Effective base weight**: for weighted survey data, the sample size that the weights leave in effect.

**Hide counts below** hides small counts, for example to protect respondents in a small group. A count below the number shows as `<5`.

### Categories and totals

**Categories, totals and subtotals** opens once a variable is in the table. Choose a variable under **Set the categories of this variable**, then:

- **Sort by** its value, its label, the count or the mean of a scale variable, ascending or descending.
- **Total, labeled**: a total for this variable only, with a label of your own, before or after its categories.
- **Show user-missing values as categories**, such as "Don't know".
- **Subtotals**: **Add subtotal**, give it a label and choose the categories it sums, for example "Top two" for codes 4 and 5. **Hide the categories a subtotal sums** shows only the subtotal. **Show the categories outside the subtotals** keeps the rest.

**Back to the defaults** clears the settings of that variable.

### Significance tests

Tests need a variable in **Columns**.

- **Chi-square test of independence**: one test per pair of a row variable and a column variable, in a table of its own. It says whether the rows depend on the columns at all.
- **Compare column proportions**: for each row, compares the column percentages of every pair of columns.
- **Compare column means**: for each variable under **Means of**, compares the means of every pair of columns.

**Significance level** is 0.05 unless you change it. With many columns, many pairs are tested, so the significance is adjusted: **Bonferroni** (the default), **Benjamini-Hochberg**, or **No adjustment**.

**Letters of the column comparisons** chooses where the results go:

- **In a table of their own** (the default): each column gets a capital letter (A, B, C and so on). A second table repeats the rows, and each cell there shows the letters of the columns it is significantly higher than.
- **In the main table, column keys**: each column gets a key (a, b, c and so on), and a cell shows the keys of the columns it is significantly higher than.
- **In the main table, shared letters (APA)**: columns that share a letter do not differ; columns with no letter in common do.

A note under the table names the test, the level and the adjustment.

### Multiple response sets

A multiple response set is one question with several answers allowed, stored as several variables. In the sample file, five yes or no variables record which channels a customer used, and the set `$channels` groups them.

- Define a set with **Data > Define Multiple Response Sets...**. A dichotomy set counts one value (such as 1 for "used") across its variables. A category set gathers the codes of several variables (first answer, second answer and so on). Saving the data keeps the set in the file.
- Put the set in **Rows** or **Columns** like a variable. Each answer becomes a row or column.
- Percentages of a set are of the respondents, so a column can add up to more than 100%. A respondent who gave none of the answers is left out of the set.
- The tests work with sets too. A note under the table says how they take into account that one respondent can be in several categories.

### Custom tables in Excel

Export the output with **File > Export...** and **Excel workbook** (see [Exporting all output](#exporting-all-output)). Each table keeps its header levels, labels and caption. Numbers stay numbers with the format they show in the window, so a percentage cell holds the exact number and shows it as 37.1%. Significance letters and hidden small counts stay text. To put several tables on one sheet, choose **All tables on one sheet**.

[Plugins](plugins.md) can color and comment the cells of custom tables in the window and in the workbook.

## Menus

The menu bar is the same in every window: File, Edit, Data, Analyze, Graphs, Transform, Window and Help. The items of Data, Analyze, Graphs and Transform open a [dialog](#dialogs) that writes a command. Each command has a page of its own, linked in the tables below, with every option.

### File

- **Open data...**, **Save data**, **Save data as...** and **Export data...** (see [Opening files](#opening-files) and [Exporting data](#exporting-data)). Save data saves the dataset of the data window it is chosen in, and the active dataset elsewhere.
- **Apply Data Dictionary...** copies labels, formats and missing values from another data file ([APPLY DICTIONARY](commands/apply-dictionary.md)).
- **New > Syntax** and **New > Output**, **Open syntax...**, **Open output...** and **Recent files**.
- **Save syntax**, **Save syntax as...**, **Save output**, **Save output as...** and **Export...** (see [Saving output](#saving-output) and [Exporting all output](#exporting-all-output)).
- **Output Management > Start...** and **End...** route tables to files while syntax runs ([OMS](commands/oms.md), [OMSEND](commands/omsend.md)).
- **Close** and **Exit**.

### Edit

- **Undo last change** (Ctrl+Z): see [Syntax is the only command layer](#syntax-is-the-only-command-layer).
- **Settings...**: see [Settings](#settings).

### Data

| Menu path | What it does | Command |
|---|---|---|
| Define Multiple Response Sets | groups variables into a multiple response set | [MRSETS](commands/mrsets.md) |
| Sort Cases | sorts the cases by one or more variables | [SORT CASES](commands/sort-cases.md) |
| Sort Variables | puts the variables in order of name, label, type and other attributes | [SORT VARIABLES](commands/sort-variables.md) |
| Restructure | variables to cases, cases to variables, or cases and variables swapped | [VARSTOCASES](commands/varstocases.md), [CASESTOVARS](commands/casestovars.md), [FLIP](commands/flip.md) |
| Split File | runs every analysis per group | [SPLIT FILE](commands/split-file.md) |
| Select Cases | leaves cases out of analyses, or deletes them | [FILTER](commands/filter.md), [SELECT IF](commands/select-if.md) |
| Select Cases by Time Range | the cases the time series analyses use | [USE](commands/use.md) |
| Weight Cases | weights the cases by a variable | [WEIGHT](commands/weight.md) |
| Aggregate | statistics per group, added to the cases or as a new dataset | [AGGREGATE](commands/aggregate.md) |
| Define Dates | date variables for a time series | [DATE](commands/date.md) |
| Orthogonal Design > Generate, Display | a design for a conjoint study, and its cards | [ORTHOPLAN](commands/orthoplan.md), [PLANCARDS](commands/plancards.md) |
| Validation > Validate Data, Compare Datasets | checks values against rules, or compares two datasets | [VALIDATEDATA](commands/validatedata.md), [COMPARE DATASETS](commands/compare-datasets.md) |
| Merge Files > Add Cases, Add Variables | appends the cases of another file, or joins its variables by keys | [ADD FILES](commands/add-files.md), [MATCH FILES](commands/match-files.md) |

### Analyze

Each item opens a dialog that writes one command. The command page lists every option the dialog offers, and more.

| Menu path | What it does | Command |
|---|---|---|
| Reports > Codebook | a description of each variable, with its labels and statistics | [CODEBOOK](commands/codebook.md) |
| Reports > Case Summaries | statistics of groups, and listings of cases | [SUMMARIZE](commands/summarize.md) |
| Reports > OLAP Cubes | totals and means of groups, with differences between groups | [OLAP CUBES](commands/olap-cubes.md) |
| Power Analysis > Means, Proportions, Correlations, Regression | the power of a test, or the sample size it needs; needs no data | [POWER MEANS ONESAMPLE](commands/power-means-onesample.md) and the other POWER commands |
| Simulation > Build Simulation Plan, Run Simulation | simulates a model from distributions you choose | [SIMPLAN](commands/simplan.md), [SIMRUN](commands/simrun.md) |
| Descriptive Statistics > Frequencies | a table of counts per value, with statistics | [FREQUENCIES](commands/frequencies.md) |
| Descriptive Statistics > Descriptives | mean, standard deviation and range of scale variables | [DESCRIPTIVES](commands/descriptives.md) |
| Descriptive Statistics > Explore | descriptive statistics, percentiles and normality tests by group | [EXAMINE](commands/examine.md) |
| Descriptive Statistics > Crosstabs | a table of two or more variables, with tests | [CROSSTABS](commands/crosstabs.md) |
| Descriptive Statistics > Ratio | statistics of the ratio of two variables | [RATIO STATISTICS](commands/ratio-statistics.md) |
| Bayesian Statistics > One Sample, Related, Independent | Bayesian tests of a mean, proportion or rate | [BAYES ONESAMPLE](commands/bayes-onesample.md), [BAYES RELATED](commands/bayes-related.md), [BAYES INDEPENDENT](commands/bayes-independent.md) |
| Bayesian Statistics > Pearson Correlation, One-way ANOVA, Linear Regression, Repeated Measures, Log-Linear Regression Models | Bayesian correlation, analysis of variance, regression and tables | [BAYES CORRELATION](commands/bayes-correlation.md), [BAYES ANOVA](commands/bayes-anova.md), [BAYES REGRESSION](commands/bayes-regression.md), [BAYES REPEATED](commands/bayes-repeated.md), [BAYES LOGLINEAR](commands/bayes-loglinear.md) |
| Custom Tables | tables of counts, percentages and means, with significance tests; see [Custom tables](#custom-tables) | [CTABLES](commands/ctables.md) |
| Compare Means > Means | means and other statistics of groups | [MEANS](commands/means.md) |
| Compare Means > One-Sample, Independent-Samples and Paired-Samples T Test | t tests of means | [T-TEST](commands/t-test.md) |
| Compare Means > One-Way ANOVA | analysis of variance of one factor, with post hoc tests | [ONEWAY](commands/oneway.md) |
| Compare Means > Proportions | tests and intervals of proportions and their differences | [PROPORTIONS](commands/proportions.md) |
| General Linear Model > Univariate | analysis of variance and covariance of one dependent | [UNIANOVA](commands/unianova.md) |
| General Linear Model > Multivariate, Repeated Measures | several dependents, or repeated measures of one | [GLM](commands/glm.md) |
| General Linear Model > Variance Components | how much each random factor adds to the variance | [VARCOMP](commands/varcomp.md) |
| General Linear Model > MANOVA (classic) | multivariate analysis of variance | [MANOVA](commands/manova.md) |
| Generalized Linear Models | models for counts, proportions and other non-normal responses | [GENLIN](commands/genlin.md) |
| Mixed Models > Linear | linear models with random effects and repeated measures | [MIXED](commands/mixed.md) |
| Mixed Models > Generalized Linear | generalized linear models with random effects | [GENLINMIXED](commands/genlinmixed.md) |
| Correlate > Bivariate | Pearson, Spearman and Kendall correlations | [CORRELATIONS](commands/correlations.md), [NONPAR CORR](commands/nonpar-corr.md) |
| Correlate > Partial | correlations controlling for other variables | [PARTIAL CORR](commands/partial-corr.md) |
| Correlate > Distances | distances or similarities between cases or variables | [PROXIMITIES](commands/proximities.md) |
| Regression > Linear | linear regression, with stepwise selection and diagnostics | [REGRESSION](commands/regression.md) |
| Regression > Curve Estimation | curves such as quadratic, logarithmic and exponential | [CURVEFIT](commands/curvefit.md) |
| Regression > Quantile | regression of a median or other quantile | [QUANTILE REGRESSION](commands/quantile-regression.md) |
| Regression > Binary Logistic | logistic regression of a yes or no outcome | [LOGISTIC REGRESSION](commands/logistic-regression.md) |
| Regression > Optimal Scaling (CATREG) | regression of categorical variables | [CATREG](commands/catreg.md) |
| Regression > 2-Stage Least Squares | regression with instrumental variables | [2SLS](commands/2sls.md) |
| Regression > Multinomial Logistic | logistic regression of an outcome with several categories | [NOMREG](commands/nomreg.md) |
| Regression > Ordinal | regression of an ordered outcome | [PLUM](commands/plum.md) |
| Regression > Probit | probit and logit models of response rates | [PROBIT](commands/probit.md) |
| Regression > Weight Estimation | weighted least squares | [WLS](commands/wls.md) |
| Regression > Partial Least Squares | regression on components of many predictors | [PLS](commands/pls.md) |
| Loglinear > General, Logit | loglinear and logit models of tables | [GENLOG](commands/genlog.md) |
| Loglinear > Model Selection | hierarchical loglinear models | [HILOGLINEAR](commands/hiloglinear.md) |
| Classify > K-Means Cluster | groups cases into a given number of clusters | [QUICK CLUSTER](commands/quick-cluster.md) |
| Classify > Hierarchical Cluster | clusters of cases or variables, with a dendrogram | [CLUSTER](commands/cluster.md) |
| Classify > TwoStep Cluster | clusters of large files, with categorical and scale variables | [TWOSTEP CLUSTER](commands/twostep-cluster.md) |
| Classify > Attribute Importance (AIM) | which variables set clusters or groups apart | [AIM](commands/aim.md) |
| Classify > Feature Selection (SELECTPRED) | the predictors most related to a target | [SELECTPRED](commands/selectpred.md) |
| Classify > Identify Unusual Cases | cases that differ from their peers | [DETECTANOMALY](commands/detectanomaly.md) |
| Classify > Discriminant | discriminant analysis, with classification | [DISCRIMINANT](commands/discriminant.md) |
| Classify > Tree | classification and regression trees | [TREE](commands/tree.md) |
| Classify > ROC Analysis | ROC curves with comparisons of tests | [ROC ANALYSIS](commands/roc-analysis.md) |
| Classify > Response Rate | gains and response rates by score groups | [RESPONSE RATE](commands/response-rate.md) |
| Classify > Nearest Neighbor | k nearest neighbor classification | [KNN](commands/knn.md) |
| Classify > Naive Bayes | naive Bayes classification | [NAIVEBAYES](commands/naivebayes.md) |
| Neural Networks > Multilayer Perceptron, Radial Basis Function | neural network models | [MLP](commands/mlp.md), [RBF](commands/rbf.md) |
| Dimension Reduction > Factor | factor analysis and principal components | [FACTOR](commands/factor.md) |
| Dimension Reduction > Correspondence Analysis | correspondence analysis of a table | [CORRESPONDENCE](commands/correspondence.md) |
| Dimension Reduction > Multiple Correspondence | correspondence analysis of several categorical variables | [MULTIPLE CORRESPONDENCE](commands/multiple-correspondence.md) |
| Dimension Reduction > Categorical Principal Components | principal components by optimal scaling | [CATPCA](commands/catpca.md) |
| Dimension Reduction > Older Optimal Scaling Commands | homogeneity analysis, nonlinear principal components and canonical correlation, and the older correspondence analysis | [HOMALS](commands/homals.md), [PRINCALS](commands/princals.md), [OVERALS](commands/overals.md), [ANACOR](commands/anacor.md) |
| Scale > Reliability Analysis | Cronbach's alpha and other reliability models | [RELIABILITY](commands/reliability.md) |
| Scale > Multidimensional Scaling (PROXSCAL), (ALSCAL) | maps of objects from their distances | [PROXSCAL](commands/proxscal.md), [ALSCAL](commands/alscal.md) |
| Scale > Multidimensional Unfolding (PREFSCAL) | maps of preferences | [PREFSCAL](commands/prefscal.md) |
| Scale > Weighted Kappa, Fleiss Multirater Kappa | agreement between raters | [WEIGHTED KAPPA](commands/weighted-kappa.md), [FLEISS MULTIRATER KAPPA](commands/fleiss-multirater-kappa.md) |
| Scale > Conjoint | conjoint analysis of preference ratings | [CONJOINT](commands/conjoint.md) |
| Nonparametric Tests > One Sample, Independent Samples, Related Samples | nonparametric tests chosen for the data | [NPTESTS](commands/nptests.md) |
| Nonparametric Tests > Legacy Dialogs | chi-square, binomial, runs, Mann-Whitney, Wilcoxon and other tests | [NPAR TESTS](commands/npar-tests.md) |
| Survival > Life Tables | life tables | [SURVIVAL](commands/survival.md) |
| Survival > Kaplan-Meier | Kaplan-Meier survival curves | [KM](commands/km.md) |
| Survival > Cox Regression | Cox proportional hazards regression | [COXREG](commands/coxreg.md) |
| Forecasting > Exponential Smoothing, ARIMA, Autoregression | models of time series | [EXSMOOTH](commands/exsmooth.md), [ARIMA](commands/arima.md), [AREG](commands/areg.md) |
| Forecasting > Create Models, Apply Models | find the best model of each series, and apply saved models | [TSMODEL](commands/tsmodel.md), [TSAPPLY](commands/tsapply.md) |
| Forecasting > Autocorrelations, Cross-Correlations | autocorrelations of a series, and correlations between series | [ACF](commands/acf.md), [PACF](commands/pacf.md), [CCF](commands/ccf.md) |
| Forecasting > Seasonal Decomposition, Spectral Analysis | seasonal factors, and periodic components | [SEASON](commands/season.md), [SPECTRA](commands/spectra.md) |
| Forecasting > Sequence Charts | a series plotted over time | [TSPLOT](commands/tsplot.md) |
| Forecasting > Forecast Period, Options | the cases to forecast, and settings of the time series commands | [PREDICT](commands/predict.md), [TSET](commands/tset.md) |
| Quality Control > Control Charts | control charts of a process | [SPCHART](commands/spchart.md) |
| Multiple Response > Frequencies, Crosstabs | tables of multiple response sets | [MULT RESPONSE](commands/mult-response.md) |
| Missing Value Analysis | patterns of missing values, with estimates | [MVA](commands/mva.md) |
| Multiple Imputation > Analyze Patterns, Impute Missing Data Values | patterns of missing values, and imputed copies of the data | [MULTIPLE IMPUTATION](commands/multiple-imputation.md) |
| Complex Samples > Sampling Plan, Select a Sample | design a sample and draw it | [CSPLAN](commands/csplan.md), [CSSELECT](commands/csselect.md) |
| Complex Samples > Analysis Plan | describe how a sample was drawn | [CSPLAN](commands/csplan.md) |
| Complex Samples > Descriptives, Frequencies, Crosstabs | estimates and tables that take the sample design into account | [CSDESCRIPTIVES](commands/csdescriptives.md), [CSTABULATE](commands/cstabulate.md) |
| Complex Samples > General Linear Model, Logistic Regression, Ordinal Regression, Cox Regression | models that take the sample design into account | [CSGLM](commands/csglm.md), [CSLOGISTIC](commands/cslogistic.md), [CSORDINAL](commands/csordinal.md), [CSCOXREG](commands/cscoxreg.md) |

### Graphs

| Menu path | What it does | Command |
|---|---|---|
| Chart Builder | bar, line, area and pie charts, histograms, scatterplots, error bars, Pareto and high-low charts | [GRAPH](commands/graph.md) |
| P-P Plots, Q-Q Plots | variables against a test distribution | [PPLOT](commands/pplot.md) |
| ROC Curve | ROC curves of test variables | [ROC](commands/roc.md) |
| Case Plots | series plotted case by case, with the cases down the page | [CASEPLOT](commands/caseplot.md) |

### Transform

| Menu path | What it does | Command |
|---|---|---|
| Compute Variable | a new variable from an expression, for all cases or only where a condition holds | [COMPUTE](commands/compute.md), [IF](commands/if.md) |
| Recode | new codes for old ones, in the same or a new variable | [RECODE](commands/recode.md) |
| Count Values within Cases | how often given values occur across variables | [COUNT](commands/count.md) |
| Rank Cases | ranks, percentile groups and normal scores | [RANK](commands/rank.md) |
| Optimal Binning | groups a scale variable by its relation to a target | [OPTIMAL BINNING](commands/optimal-binning.md) |
| Prepare Data for Modeling (ADP) | cleans and transforms variables for modeling | [ADP](commands/adp.md) |
| Automatic Recode | consecutive codes for the values of a variable, labeled with the old values | [AUTORECODE](commands/autorecode.md) |
| Create Time Series | new series from existing ones: differences, lags, moving averages and more | [CREATE](commands/create.md) |

### Window

See [The Window menu](#the-window-menu).

### Help

- **Documentation** opens this documentation online.
- **About Statscraft** shows the version, the license (MIT), links to the source code and the documentation, and the licenses of the color themes and fonts.
- **Check for updates**: see [Updates](#updates).

## Dialogs

Every dialog that builds syntax ends the same way. That holds for the menus' dialogs and for the Variables tab's editors of type and format, value labels and missing values.

- The syntax it builds is shown, or why there is none yet.
- **OK** runs it, **Paste** puts it in the designated syntax window, **Cancel** closes the dialog. The menus' dialogs also have **Reset**.
- OK and Paste stay off until the syntax is complete. Escape cancels.
- Options left at their defaults add nothing to the syntax.

## The status bar and the log

The status bar:

- shows what is running, and how many runs wait after it;
- says how many warnings a run gave (the warnings appear under their command in the log); a click there shows the log, and from a data window it brings the designated output window to the front;
- shows errors, whatever the tab;
- names, at its right, the split, filter and weight in force, such as `Split by region (layered)`, `Filter: filter_$` and `Weight: weight`. A click on one opens Split File, Select Cases or Weight Cases.

The log, in every syntax window and output window:

- lists every command with what it changed;
- has **Copy syntax**, which gives a script that repeats the session with `statscraft run`;
- is turned off by Edit > Settings > Grids and output > Log under Syntax and Output. A run with warnings shows the log even when it is off.

Data windows have no log, so the grid gets the whole height.

## Syntax is the only command layer

Every change and every result in the window comes from syntax run by the engine, exactly as `statscraft run` runs it.

- Opening a file runs `GET FILE` and `DATASET NAME` with the next free name (DataSet1, DataSet2 and so on), so the data open before stays open in the session.
- Editing a cell runs `IF ($CASENUM = n) var = value.`
- Editing a label runs `VARIABLE LABELS`, and so on.
- Macros defined by one run (DEFINE, or INCLUDE of a macro library) stay defined for the later runs while the window is open, with the macro settings of SET.
- Edit > Undo last change (Ctrl+Z outside a text field) puts back the dataset from before the last run that changed it. It goes one step back. The log marks that run as undone, and Copy syntax leaves it out. The command line and an agent do the same with `statscraft undo` and the MCP tool `undo`.
- The window stays responsive while a run goes on. The status bar shows what is running.
- One run goes at a time. Runs from every window and requests from the command line wait their turn, in the order they came.

## Settings

Edit > Settings holds every setting of the window in one dialog, with its sections in a list on the left. A change applies at once and is saved. **Close** or Escape closes the dialog.

- **Editor**
  - **Syntax colors**: the colors and background of the syntax editor. Statscraft Light (the default) and Statscraft Dark are in the Statscraft colors. The ready-made editor themes are Catppuccin Latte and Mocha, One Dark, Solarized Light and Dark, and GitHub Light and Dark.
  - **Wrap long lines** (on by default) wraps them at the window edge.
- **Grids and output**
  - **Paper of the grids and the output**: the Data and Variables grids and the output tables. **Always light** (the default) keeps them dark text on a light sheet, as a spreadsheet or a document page does, while menus, panels and dialogs follow the system's dark or light theme. **Follow the system theme** draws them in the system theme too.
  - **Log under Syntax and Output** (on by default).
  - **Show in the output**: Command headings (off by default), Titles, Tables, Notes under tables and Charts (on by default). See [What the output shows](#what-the-output-shows).
  - **Variables in tables** (Labels, Names, or Names and labels) and **Values in tables** (Labels, Values, or Values and labels). These belong to the session, not the window. A choice runs `SET TVARS` or `SET TNUMBERS` like any other command, shows in the log and stays for the later runs. A SET in syntax changes the dialog too. The window remembers the choice and sets it again when it starts, so you set it once.
  - **Output for multiply imputed data**: Original data, Each imputation, Pooled and Diagnostics, for procedures run on data split by `Imputation_`. A change runs `SET MIOUTPUT=[...]`; at least one part stays checked. Like the table labels it belongs to the session, and the window sets it again when it starts.
- **Updates**
  - **Check for updates automatically**: see [Updates](#updates).
- **AI agents**
  - Adds the Statscraft MCP server to the AI apps on this computer, or takes it out again, with a click per app, and says what to restart. For Claude Desktop, **Install extension** downloads and opens the Statscraft extension of this version. **Check** shows each app's settings files and whether the entry is in them now. See [Connect your AI agent](connect.md#from-the-window-edit--settings--ai-agents).

- **Plugins**
  - The installed plugins, programs that color and comment the cells of custom tables, in the order they run. See [Plugins](plugins.md).

## What the window remembers

Between sessions the window remembers the size and position of each kind of window, the folder of the last file dialog, the ten recent files (File > Recent files), the column widths of the Variables tab and every choice in Edit > Settings. A new window opens where the last one of its kind was, a little lower and to the right of those already open.

## Closing and unsaved work

The program ends when the last window closes, or with File > Exit, the Windows installer of an update, or Restart now. It first checks for unsaved work in every window.

- Unsaved work is a dataset changed since it was last opened or saved, syntax whose text differs from its file, new syntax that is not empty, and an output document with tables added or deleted since it was opened or saved.
- Data counts as saved after a run that ends with `GET`, `GET DATA` or `SAVE`, whether from a menu, a syntax window or the command line.
- With nothing unsaved, the program ends at once.
- Otherwise one question lists what is unsaved, with **Save**, **Don't save** and **Cancel**.
  - Save saves the syntax of every window, every changed output document and every changed dataset, each to its file, or with a file dialog when there is none yet. The program ends once all are written. A canceled file dialog or a failed save keeps it open.
  - Cancel, Escape or a click beside the question keeps it open.

Closing one window while others stay open asks only about its own work: the syntax of a syntax window, the data of a data window, the output of an output window.

## Updates

When a new version is out, a bar under the menu says so, with a link to the release notes, **Update now** and **Later**. **Help > Check for updates** checks at once. [Install and update](install.md#updates) says what Update now does for each kind of installation, and how to turn the daily check off.

## Driving the window from the command line or an AI agent

While the window is open, it is a live session named `gui`:

```
statscraft run -s gui -e "GET FILE='survey.sav'."         the window opens the file
statscraft run -s gui --json job.sps                      runs job.sps in the window
statscraft check -s gui -e "COMPUTE x = age / 10."       validates against the window's data
statscraft describe -s gui --json                        dictionary of the window's dataset
```

- The window runs the syntax exactly as its own Run button does. The tables appear in the designated output window, the commands in the log, and the data windows show the result.
- The command line prints what it prints for a file session, with `"session": "gui"`. `saved` is true when the run changed the window's dataset. Nothing is written to disk unless the syntax says SAVE.
- Relative paths in the syntax start from the folder where `statscraft` was started, unless a CD is in force in the window's session; a CD stays until the next CD. A run leaves that folder to the window's own runs.

So an AI agent with a shell can work in the open window. Start `statscraft-gui`, then tell the agent to use `statscraft run -s gui --json` for every step and `statscraft help COMMAND` for the syntax. The person watching sees each table appear, and can keep working in the window between the agent's runs.

Only programs that run as you on the same computer can reach the window. See [AI agents](ai-agents.md#safety-of-the-live-session) for what this means for safety.

## Fonts

The window brings its own fonts, both under the SIL Open Font License:

- Schibsted Grotesk for menus, dialogs, grids and output, with digits of equal width so numbers line up in columns;
- JetBrains Mono for syntax.

Characters they lack, such as Cyrillic in Schibsted Grotesk, come from system fonts: Segoe UI and Consolas on Windows, Arial and Courier New on macOS, DejaVu, Noto or Liberation on Linux. Set `STATSCRAFT_FONT` or `STATSCRAFT_MONO_FONT` to a .ttf file to use another font first.
