# The window

```
statscraft-gui survey.sav job.sps     open a data file and a syntax file
```

`statscraft-gui` is a desktop window over the same engine as the command line. It has three kinds of window, all on one session:

- a [data window](#data-windows) for each open dataset, with the **Data** and **Variables** tabs;
- a [syntax window](#syntax-windows) for each syntax file;
- an [output window](#output-windows) for each output document.

The session has one active dataset, and every command, from any window, works on it. Every change and every result in the windows comes from syntax (see [Syntax is the only command layer](#syntax-is-the-only-command-layer)).

With no data given, the program starts with one empty data window, a place to open files from.

## Opening files

- Give a data file, a syntax file or both on the command line, as above.
- File > Open data opens a .sav or .zsav file at once, in a data window of its own. The empty data window takes the first file.
- An Excel workbook (.xlsx, .xlsm, .xls) or a text file (.csv, .txt, .tab, .dat) opens the Open Data dialog first. It writes `GET DATA`, with `DATASET NAME` after it as for a .sav file:
  - for a workbook: the sheet, whether the first row holds the variable names, and a range such as A1:F200 (empty reads the whole sheet);
  - for a text file: the delimiter (comma, semicolon or tab; the one the first line uses most is chosen), and whether the first line holds the names, with that line shown. Every variable is read as `AUTO`.
- After an Open Data run, File > Save data asks for a new .sav file.
- File > Open output opens a Statscraft output file (.scout) in an output window of its own, by running `OUTPUT OPEN FILE='...'.`. See [saving output](#saving-output).
- File > Recent files lists the ten files opened or saved last.
- Files dropped on the window from the file manager open too. A .sav or .zsav file opens as data, a workbook or text file in the Open Data dialog, a .sps file in a syntax window and each .scout file in an output window. One data file and one syntax file open at a time; the status bar names the files left out.
- The installer on Windows, and the packages for macOS and Linux, let a double-click on a .sav, .zsav, .sps or .scout file open it in Statscraft. On macOS this is not yet tested on a Mac.

## Exporting data

File > Export data writes `SAVE TRANSLATE`. The file name chosen sets the type: an Excel workbook (.xlsx), a CSV file (.csv) or tab-delimited text (.txt). The dialog offers:

- a first row of variable names, variable labels or none;
- value labels instead of values;
- user-missing values as empty;
- the variables to export (none chosen: all, in the order of the dataset).

An existing file is replaced once the file dialog has asked.

## Data windows

Each dataset of the session that holds data has a data window. The title names the file and the dataset, such as `survey.sav [DataSet1]`, and the window of the active dataset says `(active)`.

- Opening a data file opens its window. A file opened from the window gets the next free dataset name (DataSet1, DataSet2 and so on), so the data open before stays open.
- A dataset made by syntax gets a window too: `DATASET COPY`, `DATASET DECLARE` once a command has written data to it, and a `GET` typed in syntax without `DATASET NAME`. A dataset that `DATASET CLOSE` closes loses its window, and `DATASET NAME` keeps the window under the new name.
- A click anywhere in a data window, its menus included, makes its dataset active. That runs `DATASET ACTIVATE name.`, which shows in the log like any run. A click in the window of the active dataset runs nothing.
- Dialogs opened from a data window work on its dataset, which is active by then.
- Each window keeps its own tab, selection and find bar. Grid edits, right-click menus and Delete work in every data window.
- The data a typed `GET` reads without `DATASET NAME` has no name. As the manual describes, such a dataset closes when another one becomes active (a click in another data window, the Active list, opening a file). When it holds unsaved changes, a question comes first: **Save** saves it and goes on, **Don't save** goes on without it, **Cancel** keeps everything as it is.
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
  - Insert variable before: a numeric variable `new_1` (`NUMERIC` and `MATCH FILES /KEEP`, with `WEIGHT BY` again when a weight is on).
  - Copy, Delete.
- **On a cell**:
  - Copy, Edit, Clear value.
  - Only cases where the variable has this value: a filter, for example `COMPUTE filter_$ = (age = 49).`, then `VARIABLE LABELS` and `FILTER BY`. It compares the stored value, not the rounded one shown. A user-missing value gives `VALUE(x) = 99`, an empty cell `SYSMIS(x)`. A variable `filter_$` that holds other data is never overwritten; the next free name, such as `filter_$_2`, is used.
  - Stop filtering.
  - Keep only cases where...: opens Select Cases to delete the other cases.
  - Sort ascending or descending by the variable.
- **On a row number**: Copy, or Delete case (`COMPUTE #row = #row + 1.` and `SELECT IF #row <> 12.`).
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

### How tables look

- Each table has its multi-level header. Its caption and notes follow it.
- Columns are as wide as their numbers, which align right.
- Long row labels and column headers wrap between words.
- A row label repeated under the same outer labels is shown once, beside all its rows.
- Each table is measured once and only the tables in view are drawn, so hundreds of tables scroll smoothly.

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
- **Export selection** saves only them, as a web page or an Excel workbook (one sheet per table, or all tables on one sheet).
- **Clear output** deletes every table, the same way.

### Charts

A chart shows in the output as a picture, as wide as the window up to about 760 pixels: flat bars, lines, areas, dots and slices in the brand's petrol and amber, with muted labels and thin rule lines, on light or dark paper as the output shows its tables. Bar charts (upright or lying, side by side, stacked or stacked to 100%), pie, line, area and scatter charts and the histogram with its normal curve are drawn, and so are the statistical plots: boxplots with their outliers and extremes marked by case, normal Q-Q and P-P plots, detrended Q-Q plots, the Q-Q and P-P plots of PPLOT against any of its distributions with their detrended forms, ROC curves (one line per test variable, with a quiet diagonal), spread versus level and the other point plots with their reference lines in amber, the dendrogram, the icicle plot, the tree diagram and the territorial map of DISCRIMINANT (each group's territory tinted in its color, with the centroids marked and named). Error bars, such as those of a profile plot with ERRORBAR, are drawn as thin bars with caps: in ink on bars, in the line's color on lines. A stem-and-leaf display, a plot in three dimensions and any kind of chart that has no drawing yet show as their data table.

A right click on a chart offers, besides Copy, Copy as text and Delete:

- **Show data**: the chart's numbers as a table in its place; **Show chart** brings the picture back.
- **Save chart as PNG...** and **Save chart as SVG...**: the picture as a file, 640 pixels wide (a PNG has twice the pixels), on the paper the output shows.

### Charts in the office programs

A chart's office form is a real chart of the office programs, with its numbers in a workbook inside it, so Edit Data opens them, and with no colors of its own, so it takes the theme of the presentation or document it goes into. Bar, pie, line and area charts have one, with their error bars when they have them (the half lengths sit in the workbook beside the values). A statistical plot, such as a histogram, a boxplot or a dendrogram, has none: it goes in as a picture, and in a workbook the picture stands beside its data table.

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

- File > Export output as HTML saves all the tables of the output window as one web page, each drawn chart as a PNG picture inside the page. Chosen in a data or syntax window, it exports the designated output window.
- File > Export output as PDF saves all the tables and charts of the output window as one PDF file on A4 pages, as OUTPUT EXPORT /PDF writes it: tables as text, charts as vector drawings, and bookmarks for the commands and their items. Chosen in a data or syntax window, it exports the designated output window.
- File > Export output as Excel saves them as a workbook, with **One sheet per table** or **All tables on one sheet**, laid out as `--xlsx` does: a chart as its data table with the editable chart, or the picture of a statistical plot, beside it.
- From syntax, OUTPUT EXPORT writes the tables and charts of an output document to a workbook, a Word document, a presentation, a web page, text or a CSV file.

## Menus

### File

Open data, Save data, Save data as, Export data; New (Syntax, Output), Open syntax, Open output, Recent files, Save syntax, Save syntax as; Save output, Save output as, Export output as HTML, Export output as PDF, Export output as Excel; Close, Exit. Save data saves the dataset of the data window it is chosen in, and the active dataset elsewhere.

### Window

See [The Window menu](#the-window-menu).

### Edit

- **Undo last change** (Ctrl+Z): see [Syntax is the only command layer](#syntax-is-the-only-command-layer).
- **Settings**: see [Settings](#settings).

### Data

- **Define Multiple Response Sets**: dichotomy or category sets, the counted value, labels, deleting sets.
- **Sort Cases**.
- **Sort Variables**: `SORT VARIABLES` by name, type, format, label, value labels, missing values, measure, role, columns or alignment, ascending or descending.
- **Restructure**:
  - variables to cases: `VARSTOCASES` with one or more new variables, each made from a list of variables, an index variable, a case number variable and the variables to copy;
  - cases to variables: `CASESTOVARS` with identifier and index variables, the new variables in the order of the variable or of the index, sorting the cases first;
  - transposing cases and variables: `FLIP` of chosen or all variables, the new variables named by the values of a variable.
- **Split File**: groups compared in the same tables or in tables of their own, sorting the cases first, or off.
- **Select Cases**:
  - a filter that leaves the other cases out of analyses: `COMPUTE filter_$ = (condition).` and `FILTER BY filter_$.`;
  - SELECT IF, which deletes them;
  - all cases again, with `FILTER OFF.` and `USE ALL.`.
- **Weight Cases**.
- **Aggregate**: into every case, into a new dataset of one case per group, or into a file.
- **Merge Files > Add Cases** and **Merge Files > Add Variables**: with keys, or with a lookup table.

### Analyze

**Descriptive Statistics**

- **Frequencies**: statistics, the order of the rows, a limit on the values of a table, quartiles, cut points for equal groups and any percentiles.
- **Descriptives**.
- **Explore**: descriptives, M-estimators, extreme values, percentiles and tests of normality, by factor.
- **Crosstabs**.

**Custom Tables**

- Rows, columns and layers of variables and multiple response sets.
- Means, percentages and totals.
- Significance tests, their letters shown in the table itself or in a table of their own.
- Hiding small counts, and an effective base weight with the effective count.
- Per variable: the order of the categories (sorted by value, label, count or a mean), the total with its label and position, subtotals over chosen categories that can hide them, and missing and other categories.

**Compare Means**

- **Means**: every cell statistic, including the grouped median and percentages within a control variable.
- **One-Sample T Test**, **Independent-Samples T Test** and **Paired-Samples T Test**.
- **One-Way ANOVA**: every post hoc test the engine has, and Dunnett's test against the first or last group, on either side.

**General Linear Model**

- **Univariate** (`UNIANOVA`): one dependent variable by fixed factors, with covariates.
  - The model: the full factorial, or a custom one of main effects and interactions picked from the factors and covariates. The type of sums of squares and the intercept.
  - Post hoc tests of chosen factors: the same tests as One-Way ANOVA.
  - Estimated marginal means overall and for each chosen factor, with main effects compared unadjusted or by Bonferroni or Sidak.
  - Saving predicted values, their standard errors, residuals, Cook's distance and leverage.
  - Contrasts of each factor (deviation, simple, difference, Helmert, repeated or polynomial; deviation and simple against the last or the first category).
  - Plots: spread versus level, residuals against observed and predicted values, and profile plots of the estimated marginal means, each with a factor on the horizontal axis and maybe one for separate lines and one for separate charts, drawn as lines or bars with error bars and a reference line at the grand mean if you like.
  - Descriptive statistics, effect sizes, observed power, parameter estimates, homogeneity tests, the lack of fit test and the significance level.
- **Multivariate** (`GLM` with several dependent variables): the Univariate dialog with a list of at least two dependent variables, all numeric and none of them a factor or covariate.
  - Everything Univariate offers; post hoc tests, estimated marginal means, plots and saved variables come for each dependent. Saved variables take their default names. Comparing main effects gives the univariate tests of each dependent only, since their multivariate tests are not supported yet (the command warns).
  - Homogeneity tests add Box's M test of the covariance matrices to Levene's test of each dependent. Further tables: the residual SSCP, covariance and correlation matrices with Bartlett's test of sphericity, the hypothesis and error SSCP of each effect, and the coefficients of the M matrix (offered only once the M matrix has a column).
  - M matrix: columns, each with an optional label and a coefficient for each dependent (an empty cell is 0, fractions such as `1/2` are allowed). It transforms the dependents in the test of the intercept, so the intercept must be in the model, and each column needs a coefficient other than 0.
- **Repeated Measures** (`GLM` with `WSFACTOR`):
  - Within-subjects factors, each with a name and its number of levels, and one variable for each combination of levels.
  - Between-subjects factors and covariates, and the type of sums of squares.
  - Descriptive statistics, effect sizes, observed power and the significance level.
  - One measure only, since `MEASURE` is not supported yet.

**Mixed Models**

- **Linear** (`MIXED`): one dependent variable with factors and covariates.
  - Subject variables, and repeated variables that tell the measurements of one subject apart, with the covariance type of the repeated measures. The repeated measures use every subject variable.
  - Covariance types: variance components (random effects only), scaled identity, diagonal, unstructured, compound symmetry, first-order autoregressive, and the further types `AD1`, `ARH1`, `ARMA11`, `CSH`, `CSR`, `FA1`, `FAH1`, `HF`, `TP`, `TPH` and `UNR`, each listed by its label with its keyword, such as "Heterogeneous Toeplitz (TPH)". `ARMA11` and `FAH1` need at least three random parameters or repeated levels; with fewer the command stops with an error.
  - Fixed effects: the main effects of the factors and covariates, or custom terms; the intercept; Type III (default) or Type I sums of squares.
  - One set of random effects: the intercept and custom terms, the subject variables they vary over, the covariance type, and the predicted random effects of every subject (`SOLUTION`). More sets of random effects are in the syntax.
  - Restricted or full maximum likelihood, the confidence level, the maximum iterations and step-halving, Satterthwaite, residual or Kenward-Roger degrees of freedom (Kenward-Roger only with restricted maximum likelihood: with maximum likelihood it is greyed out and the syntax uses Satterthwaite), and user-missing values as valid.
  - Estimates of the fixed effects, tests of the covariance parameters, the covariance of the random effects (with random effects), the covariance of the residuals (with repeated variables), the covariance and correlation of the fixed estimates, the case processing summary, descriptive statistics, the iteration history (every n-th iteration) and the coefficients of every test.
  - Estimated marginal means: any number of tables, each the overall mean or a crossing of factors. A table can compare the levels of one of its factors, every pair or each level with the first or the last, unadjusted or by Bonferroni or Sidak. Covariates stay at their means unless you give a value.
  - Custom tests: a label and rows typed one per line, each row an effect of the fixed part (or `INTERCEPT`, or `ALL`) followed by its coefficients, such as `gender 1 -1 0`. Fractions such as `1/3` are allowed. With random effects, a row may go on after `|` with coefficients of random effects: a random term, `INTERCEPT` when the random intercept is in, or `ALL`, or plain values for every random parameter, as in `gender 1 -1 0 | INTERCEPT 1`. The part before `|` may be empty or `ALL` with no coefficients. `ALL` and effect names never mix in one test; plain values after `|` fit either. The dialog checks that each effect is in the model or among the random effects; whether the number of coefficients fits is checked when the command runs.
  - Saving the fixed predicted values with their standard errors and degrees of freedom, the predicted values with theirs, and the residuals.

**Generalized Linear Models**

- **Generalized Linear Models** (`GENLIN`): one response on factors and covariates.
  - The response: one dependent variable, or a count of events out of trials, where the trials are a variable or one number for every case. A binary dependent with the binomial distribution takes a reference category (last, first or a value); an ordinal one takes the order of its categories.
  - The distribution: normal, binomial, gamma, inverse Gaussian, negative binomial (k given or estimated), Poisson or ordinal multinomial. The link list shows only the links that go with it, and choosing a distribution sets its usual link and scale. Power and odds power links take an exponent.
  - The model: main effects, the full factorial of the factors with the covariates, or custom terms; the intercept (always in for the ordinal multinomial distribution, whose thresholds are always fitted), an offset variable and a scale weight variable.
  - Estimation: hybrid (Fisher scoring for a number of iterations, then Newton-Raphson), Fisher scoring or Newton-Raphson; the scale parameter by maximum likelihood (normal, gamma and inverse Gaussian only), deviance, Pearson chi-square or a fixed value; a model-based or robust covariance; the confidence level, the maximum iterations and step-halving, the parameter convergence and the full or kernel log-likelihood.
  - Statistics: Type III, Type I or both tests of effects, by Wald or likelihood ratio; the tables of the output, with exponentiated estimates if you like.
  - Estimated marginal means as in Linear Mixed Models, with seven ways to compare levels and five adjustments, on the scale of the response or of the linear predictor. Not offered for the ordinal multinomial distribution.
  - Saving the linear predictor, the predicted mean with its interval, the predicted category, leverage, five kinds of residuals and Cook's distance. The ordinal multinomial distribution saves the first six only, and the predicted category needs a binary or ordinal dependent.
  - User-missing values of the factors as valid.
  - Repeated measures (`REPEATED`, generalized estimating equations): the subject variables and the within-subject variables that order each subject's measurements. Both are categories, so they may be factors of the model or variables outside it, but not the response, the trials variable, a covariate, the offset or the scale weight, and no variable is on both lists. Then sorting the cases (on by default; without it each run of equal subject values in file order is a subject with its records in file order, while the within-subject variables still decide which cases are complete), the working correlation (independent, AR(1), exchangeable, m-dependent with its m, unstructured, or fixed values typed below the diagonal row by row, each from 0 to 1), the adjustment of the correlation by the number of parameters, a robust or model-based covariance, the maximum iterations, how often the matrix is updated, the parameter convergence (absolute or relative) and the Hessian convergence. Only settings that differ from the defaults are written. The dialog checks the roles, m and the count and range of the fixed values; whether the fixed matrix has a row for each measurement is checked when the command runs.
  - With repeated measures the covariance choice of the Estimation section is greyed out, since the one under repeated measures counts, and its maximum iterations and parameter convergence settle the starting model only.
  - With repeated measures the Statistics section adds the working correlation matrix, the case processing summary also gives the correlated data summary, and the summary table has the tests of effects only, without the omnibus test. The ordinal multinomial distribution, likelihood ratio tests and saving leverage, Cook's distance and the deviance, standardized and likelihood residuals are greyed out, since the estimating equations do not give them; a choice made before stays removable.

**Correlate**

- **Bivariate**: Pearson by CORRELATIONS, Spearman and Kendall by NONPAR CORR, and both commands when both kinds are chosen.
- **Partial** (`PARTIAL CORR`): the variables controlling for others; a one- or two-tailed test; the coefficients alone, marked with stars; means and standard deviations; the zero-order correlations; cases left out of every correlation or only where they are missing.

**Regression**

- **Linear** (`REGRESSION`): one dependent variable on blocks of independent variables.
  - Each block with its method: Enter, Stepwise, Remove, Backward or Forward.
  - Estimates, confidence intervals at any level, model fit, R squared change, descriptives, part and partial correlations, and collinearity diagnostics.
  - Saving predicted values, residuals, distances and influence statistics.
  - The stepping criteria as probabilities or values of F, the constant, and listwise, pairwise or mean substitution of missing values.
- **Binary Logistic** (`LOGISTIC REGRESSION`): one dependent variable with two values on blocks of covariates.
  - Each block entered, or stepped forward or backward by the conditional, likelihood-ratio or Wald statistic.
  - Categorical covariates, each with its contrast and the first or last category as reference.
  - Saving predicted probabilities, group membership, residuals and influence statistics.
  - The Hosmer-Lemeshow test, confidence intervals for exp(B), the iteration history, the classification cutoff, the stepwise entry and removal probabilities, the maximum iterations and the constant.
- **Multinomial Logistic** (`NOMREG`): a nominal dependent variable, numeric or string, on factors and covariates.
  - The reference category: the last, the first or a value you type; the categories in ascending or descending order.
  - The model: main effects, the full factorial, or custom terms built as in Univariate; the intercept. A custom model can add stepwise terms with a method (forward entry, backward elimination, forward stepwise or backward stepwise). Forward entry and forward stepwise start from the model terms, or from the intercept alone when the model is empty; backward elimination and backward stepwise start with every stepwise term entered.
  - Stepping, for stepwise terms: the hierarchy rule, the least effects (backward methods), the most effects (forward methods), and the probabilities and tests for entry and for removal, each offered only for the methods that use it.
  - The tables: case processing, model fitting with or without the information criteria, pseudo R-square, goodness of fit, likelihood ratio tests, parameter estimates, classification, the covariance and correlation of the estimates, and the step summary. With every table off the command shows none.
  - Saving the probability of each category, the predicted category, its probability and the probability of the observed category.
  - The confidence level, the maximum iterations and step-halving, and user-missing values as valid.
- **Ordinal** (`PLUM`): an ordinal dependent variable on factors and covariates.
  - The link: logit, probit, complementary log-log, negative log-log or Cauchit.
  - The location part: the main effects, or custom terms. The scale part: none, or custom terms.
  - The tables: goodness of fit, the summary, parameter estimates, the test of parallel lines, cell information, the covariance and correlation of the estimates, the iteration history and the kernel of the log-likelihood.
  - The same saved variables as Multinomial Logistic; the confidence level, the maximum iterations and step-halving, the value added to empty cells, and user-missing values as valid.
  - Custom tests typed as in Linear Mixed Models, where `INTERCEPT` stands for the thresholds. An effect that is in both the location and the scale part needs `ALL`, and then every row of that test uses `ALL`.

**Classify**

- **K-Means Cluster** (`QUICK CLUSTER`): a number of clusters, by iterating and classifying or by classifying only.
  - The maximum iterations and the convergence criterion.
  - Saving the cluster membership and the distance from the cluster center.
  - The initial centers, the ANOVA table, and the cluster of each case labeled by a variable.
  - Cases left out listwise or pairwise.
- **Hierarchical Cluster** (`CLUSTER`): the cases by the variables, labeled by a variable.
  - The method: between-groups or within-groups linkage, nearest or furthest neighbor, centroid or median clustering, or Ward's method.
  - The measure for interval data: squared Euclidean, Euclidean, cosine, Pearson correlation, Chebychev, block, Minkowski with its power, or customized with a power and a root.
  - The measure for counts: chi-squared or phi-squared.
  - The measure for binary data, with the values of presence and absence: squared Euclidean, Euclidean, simple matching, Jaccard, Dice, Russell and Rao, Rogers and Tanimoto, Hamann, Ochiai, phi 4-point correlation or Yule's Q. The other binary measures are in the syntax.
  - The agglomeration schedule, the proximity matrix, and the cluster membership of one solution or a range of them.
  - Saving the membership of one solution or a range.
- **Discriminant** (`DISCRIMINANT`): a grouping variable with its range of values.
  - Independents entered together, or stepwise by Wilks' lambda, the Mahalanobis distance, the smallest F ratio, the unexplained variance or Rao's V, with values of F or their probabilities to enter and remove.
  - Means, univariate ANOVAs, Box's M, Fisher's and unstandardized function coefficients, the within-groups correlation and covariance, and the separate-groups and total covariance matrices.
  - Prior probabilities equal or from the group sizes; classification by the within-groups or separate-groups covariance; the summary table and leave-one-out classification.
  - Saving the predicted group, the discriminant scores and the probabilities of group membership.
  - A selection variable with its value.
- **Tree** (`TREE`) by CHAID or exhaustive CHAID.
  - A nominal or ordinal dependent, and predictors each with its own level or the measure of the dictionary.
  - The maximum depth, and the fewest cases in a parent and in a child node.
  - Target categories for the gain tables.
  - Saving the terminal node, the predicted category and the predicted probabilities.

**Dimension Reduction**

- **Factor** (`FACTOR`):
  - Univariate descriptives, the initial solution, the correlation coefficients with their significance, the determinant, KMO and Bartlett's test, and the inverse, reproduced and anti-image matrices.
  - Extraction by principal components, unweighted or generalized least squares, maximum likelihood, principal axis, alpha or image factoring; of the factors with eigenvalues over a value, or a fixed number of them; with the maximum iterations.
  - Rotation by varimax, direct oblimin with its delta, quartimax, equamax or promax with its kappa, or none.
  - Factor scores saved by the regression, Bartlett or Anderson-Rubin method, and the score coefficient matrix.
  - Listwise, pairwise or mean substitution of missing values, and loadings sorted by size with small ones left blank.
- **Correspondence Analysis** (`CORRESPONDENCE`): a row variable by a column variable, each numeric with the smallest and largest category value to count. A newly set variable starts from its lowest and highest labeled whole value that is not user-missing.
  - Each category of the range, shown with its value label, is active, supplementary or in a numbered equal group whose members share one score. Groups need two categories or more, and each side needs at least two active categories, a group counting as one. When you leave a changed range field, the roles of the categories now outside the range are forgotten.
  - The model: the number of dimensions, the chi-square distance or a Euclidean distance with one of its five standardizations, and symmetrical, principal, row principal, column principal or a custom normalization from -1 to 1. When the table allows fewer dimensions than you ask for, for example because a side has few active or non-empty categories, the command computes fewer.
  - Statistics: the table, the row and column profiles, the overviews and the confidence of the row and column points, and permuted tables up to a dimension.
  - Plots: the biplot (not with principal normalization), the row and column points, the transformation plots of the rows and the columns, a longest label and a range of dimensions instead of every pair.
  - Writing the scores or variances to a file (`OUTFILE`) and table data (`TABLE=ALL`) are in the syntax.

**Scale**

- **Reliability Analysis** (`RELIABILITY`): one scale of all the items.
  - The model: Alpha, Split-half, Guttman, Parallel or Strict parallel. A scale label.
  - Descriptives for each item, the scale, and the scale if an item is deleted.
  - Inter-item correlations and covariances, and their summaries.
  - An ANOVA table with the F test, Friedman's or Cochran's chi-square, Hotelling's T-squared and Tukey's test of additivity.
  - The intraclass correlation with its model, type, confidence level and test value.

**Nonparametric Tests** (`NPAR TESTS`), with the dialogs analysts know. Each has descriptives, quartiles, and cases left out test by test or listwise.

- **Chi-square**: the categories from the data or a range, and equal or given expected values.
- **Binomial**: a test proportion, and the two values of the data or a cut point.
- **Runs**: at the median, mode, mean or a custom cut point.
- **1-Sample K-S**: against the normal, uniform, Poisson or exponential distribution.
- **2 Independent Samples**: Mann-Whitney, Kolmogorov-Smirnov, Moses and Wald-Wolfowitz.
- **K Independent Samples**: Kruskal-Wallis, the median test and Jonckheere-Terpstra.
- **2 Related Samples**: Wilcoxon, sign, McNemar and marginal homogeneity, on pairs.
- **K Related Samples**: Friedman, Kendall's W and Cochran's Q.

**Multiple Response** (`MULT RESPONSE`): **Frequencies** and **Crosstabs**.

- Groups defined in the dialog, as dichotomies with a counted value or as categories with a range. They are kept from one of the two dialogs to the other while the window is open.
- Frequency tables of every group.
- Crosstabulations of groups and variables in rows, columns and layers. Each variable has its range, starting from its lowest and highest labeled whole value that is not user-missing.
- Row, column and total percentages, based on cases or on responses.
- Leaving out cases missing on any variable of a dichotomy or category group.

**Complex Samples**

- **Analysis Plan** (`CSPLAN ANALYSIS`): writes a plan file, a small JSON file, with the sample weight and one to three stages. Each stage has an optional label, strata and clusters (every stage but the last needs clusters), and is drawn with replacement or with equal probability without replacement. A stage drawn with replacement ends the design, so no stage can follow it. Without replacement needs the population size or the inclusion probability, as one value or read from a variable. Design effects compare with simple random sampling with or without replacement. Matrices of population sizes are in the syntax.
- **Descriptives** (`CSDESCRIPTIVES`), **Frequencies** and **Crosstabs** (`CSTABULATE`, one-way and two-way tables) each pick a plan file.
  - Descriptives: means and sums of the measures, and one ratio of numerators to denominators.
  - Frequencies and Crosstabs: population sizes and table percents; in Crosstabs also row and column percents.
  - The standard error, coefficient of variation, design effect and its square root, the unweighted count and a confidence interval at any level. Descriptives adds the population size, Frequencies cumulative values, Crosstabs the expected table percents and the residuals (with table percents).
  - Tests: equal shares in Frequencies; independence, the odds ratio, relative risk and risk difference in Crosstabs.
  - Subpopulations, shown together or in a table for each variable; cases valid on every variable only; user-missing values of categories as valid.
- **General Linear Model** (`CSGLM`) and **Logistic Regression** (`CSLOGISTIC`) each pick a plan file and fit one dependent on factors and covariates. The linear model needs a numeric dependent; the logistic one takes a numeric or string dependent with its reference category: the highest (the default), the lowest or a value. Covariates are numeric.
  - The model: main effects, the full factorial of the factors with the covariates, or custom terms; the intercept included, left out or alone, and shown in the tables or not.
  - Statistics of the estimates (estimates, standard error, t test, confidence interval, design effect and its square root, and in Logistic Regression the exponentiated estimates), the test statistic (Wald F or chi-square, adjusted or not) and the adjustment of p values for several tests.
  - Custom tests, typed one row per line as in the other model dialogs: INTERCEPT, when the model has it, comes first in a row (in a test of ALL rows it takes a row of its own), and a row can end with `= value` to test against that value instead of 0.
  - General Linear Model: tables of estimated marginal means, each of crossed factors or the overall mean, with a comparison of one factor's levels by a contrast, and the values the covariates are held at.
  - Logistic Regression: odds ratios, each of a factor against its lowest or highest level or a value, or of a covariate by one or more changes, with the levels or values the other factors and covariates are held at. The lowest and highest levels are choices of their own, so a typed value is always a category, quoted for string and formatted variables. An odds ratio of a variable taken out of Factors or Covariates is dropped, and a variable moved between the two starts again from its default level. Also the iteration limits and convergence criteria.
  - The confidence level, the degrees of freedom of the design, a subpopulation (a variable outside the model and its value), user-missing values of factors as valid, the tables to show (Logistic Regression adds the classification table and the iteration history every n-th step), and saving the predicted values and residuals or the predicted probabilities and category, with names of your own.
  - The dialog checks roles, string variables where numbers are needed and names of saved variables that are taken (also the numbered names a root of predicted probabilities makes, and whether they fit) before the command runs, and refuses means, held covariate values, odds ratios and held values of a factor or covariate that the model leaves out (with the intercept alone, or custom terms without it). A missing plan file and plan variables in the model are found when it runs.

### Graphs

- **Chart Builder** (`GRAPH`): one chart, picked from a list: simple, clustered or stacked bars; simple or multiple lines; an area chart; a pie; a histogram; simple, grouped, overlay or matrix scatterplots; error bars; a Pareto chart; or a high-low chart. Only the roles the chosen chart needs are shown.
  - Bars, lines, areas, pies, Pareto and high-low charts show a function by the categories of a variable: the number or percentage of cases (also cumulative), or the mean, median, mode, sum, minimum, maximum, standard deviation, variance, grouped median, a percentile, or the share or number of values below, above or within a range, of one or more variables. Pareto charts show counts or sums; high-low charts summarize two or three variables (high, low and close).
  - Clustered and stacked bars, multiple lines, areas and Pareto charts take a second category variable for their series, or instead summarize several variables, one series each. Simple bars and lines and pies summarize one variable.
  - A histogram can carry a normal curve. Scatterplots take their axes, a variable that groups the points and one whose values label them; an overlay pairs every X variable with every Y variable, or the lists in order.
  - Error bars show the mean of each variable, by category and cluster, with a confidence interval at any level from 50 to 99.9, or a number of standard errors or standard deviations.
  - A title, subtitle and footnote; panel variables, which draw a chart for each combination of their categories; and how missing values are left out, whether they get categories of their own, and whether user-missing values count as valid.
  - There is no preview: OK draws the chart in the output.
- **P-P Plots** and **Q-Q Plots** (`PPLOT`): variables against a test distribution (normal, exponential, Weibull, Pareto, lognormal, beta, gamma, logistic, Laplace, uniform, half-normal, chi-square or Student t), its parameters estimated from the data or given (chi-square and Student t always need their degrees of freedom). The proportion formula (Blom, Rankit, Tukey, Van der Waerden), the rank of tied values, the plot, the detrended plot or both, and the natural log, standardizing, differencing and seasonal differencing with its period.
- **ROC Curve** (`ROC`): test variables against the value of a state variable that marks the positive state. The curve with or without the diagonal reference line, the standard error and confidence interval of the area with the chart of the model quality, and a table of the cutoffs with their sensitivity and specificity. Whether larger or smaller test values point to a positive test, whether a value at the cutoff counts as positive, the confidence level, a nonparametric or bi-negative exponential standard error, and user-missing values as valid.

### Transform

- **Compute Variable**, **Recode** and **Count Values within Cases**.
- **Rank Cases**: every RANK function, with new names or the default ones; ranks within groups; from the smallest or the largest value; ties; the proportion formula; user-missing values.
- **Automatic Recode** (`AUTORECODE`): into new variables, from the largest value, one scheme for all, blank strings as missing, and a table of the values.

### Help

- **Documentation** opens the documentation online.
- **Connect an AI agent...** opens Edit > Settings > AI agents.
- **About Statscraft** shows the version, the license (MIT), links to the source code and the documentation, where the color themes come from with their licenses, and the licenses of the fonts.
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
- Edit > Undo last change (Ctrl+Z outside a text field) puts back the dataset from before the last run that changed it. It goes one step back. The log marks that run as undone, and Copy syntax leaves it out.
- Opening a file and every run happen off the drawing thread. The status bar shows what is running, and the window stays responsive.
- One run goes at a time. Runs from every window and requests from the command line wait their turn, in the order they came.

## Settings

Edit > Settings holds every setting of the window in one dialog, with its sections in a list on the left. A change applies at once and is saved. **Close** or Escape closes the dialog.

- **Editor**
  - **Syntax colors**: the colors and background of the syntax editor. Statscraft Light (the default) and Statscraft Dark are in the Statscraft colors. The ready-made editor themes are Catppuccin Latte and Mocha, One Dark, Solarized Light and Dark, and GitHub Light and Dark.
  - **Wrap long lines** (on by default) wraps them at the window edge.
- **Grids and output**
  - **Paper of the grids and the output**: the Data and Variables grids and the output tables. **Always light** (the default) keeps them dark text on a light sheet, as a spreadsheet or a document page does, while menus, panels and dialogs follow the system's dark or light theme. **Follow the system theme** draws them in the system theme too.
  - **Log under Syntax and Output** (on by default).
  - **Variables in tables** (Labels, Names, or Names and labels) and **Values in tables** (Labels, Values, or Values and labels). These belong to the session, not the window. A choice runs `SET TVARS` or `SET TNUMBERS` like any other command, shows in the log and stays for the later runs. A SET in syntax changes the dialog too. The window also keeps the choice (lines such as `tvars names` and `tnumbers both` in `window.txt`) and runs the same SET when it starts, so you set it once.
- **Updates**
  - **Check for updates automatically**: see [Updates](#updates).
- **AI agents**
  - Adds the Statscraft MCP server to the AI apps on this computer, or takes it out again, with a click per app, and says what to restart. For Claude Desktop, **Install extension** downloads and opens the Statscraft extension of this version. **Check** shows each app's settings files and whether the entry is in them now. See [Connect your AI agent](connect.md#from-the-window-edit--settings--ai-agents).

## What the window remembers

The window keeps these in `statscraft/window.txt` in the user's config folder (`%APPDATA%` on Windows, `$XDG_CONFIG_HOME` or `~/.config` elsewhere):

- the size and position of the last data window, syntax window and output window, which a new window of that kind opens with (a little lower and to the right of the windows of its kind already open);
- the folder of the last file dialog;
- the ten files opened or saved last (File > Recent files);
- the settings of Edit > Settings;
- the column widths of the Variables tab.

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

The window asks GitHub at most once a day whether a newer release is published. It does so in the background and never holds up the start.

- Checks are on by default and say nothing unless a newer version is out.
- Edit > Settings > Updates turns them on and off (the line `updates on` or `updates off` in `window.txt`).
- Help > Check for updates checks at once, and also says when there is nothing new.

When a newer release is out, a small bar under the menu says so, with a link to the release notes, **Update now** and **Later**. Later hides the bar until the next check. Update now downloads the package for this installation and checks it against the release's `SHA256SUMS`. Then it depends on how Statscraft was installed:

- **Windows installer**: the window closes, the new installer runs silently for the same per-user installation (same folder, file types and Start menu entry), and the installer opens the window again. Unsaved work is asked about first, as for any close.
- **Tarball**: `statscraft` and `statscraft-gui` in its `bin` folder are replaced (each written next to the old one, then renamed over it). The bar offers **Restart now**, which closes the window (asking about unsaved work first) and starts the new one, and **Later**, which keeps this window. Cancel in the unsaved-work question keeps this window and starts none.
- **Portable Windows .zip** (the folder with `LICENSE.txt` next to the programs): `statscraft.exe` and `statscraft-gui.exe` in that folder are replaced the same way, with **Restart now**. Windows cannot replace a running program, so the old one is renamed aside first and deleted the next time Statscraft starts.
- **Statscraft.app on macOS**: there is no Update now. The bar has a **Download** link to the new macOS package; replace Statscraft.app with the one in it. The updater does not change files inside the app, which a signed app would not allow.
- **.deb**: the package is saved in `~/Downloads`, and the bar shows the one command that installs it, `sudo apt install ~/Downloads/statscraft_X_amd64.deb`, with a **Copy command** button. The window never runs sudo.
- **Built from source or copied elsewhere**: there is no Update now. The bar says to install the new version the way this copy was installed.

The check is one request to the GitHub API with the User-Agent `statscraft/VERSION`. Nothing about you or your data is sent. `statscraft update` does the same from the command line.

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

How it connects:

- On start, the window listens on a random TCP port of 127.0.0.1.
- It writes the address and a random token to `statscraft/gui.json` in `$XDG_RUNTIME_DIR` on Linux or `%LOCALAPPDATA%` on Windows (`~/.config` otherwise), readable only by the user.
- A request without the token is refused. The command line sends the token only to a local address.
- When no window is open, `-s gui` fails with "no Statscraft window is running". With several windows open, the one started last answers.

See [AI agents](ai-agents.md#safety-of-the-live-session) for what this means for safety.

## Fonts

The window brings its own fonts, both under the SIL Open Font License:

- Schibsted Grotesk for menus, dialogs, grids and output, with digits of equal width so numbers line up in columns;
- JetBrains Mono for syntax.

Characters they lack, such as Cyrillic in Schibsted Grotesk, come from system fonts: Segoe UI and Consolas on Windows, Arial and Courier New on macOS, DejaVu, Noto or Liberation on Linux. Set `STATSCRAFT_FONT` or `STATSCRAFT_MONO_FONT` to a .ttf file to use another font first.
