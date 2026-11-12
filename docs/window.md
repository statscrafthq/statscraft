# The window

```
statscraft-gui survey.sav job.sps     open a data file and a syntax file
```

`statscraft-gui` is a desktop window over the same engine as the command line. The main window has three tabs: **Data**, **Variables** and **Output**. Syntax opens in [syntax windows](#syntax-windows) of its own. All windows work on one session. Every change and every result in them comes from syntax (see [Syntax is the only command layer](#syntax-is-the-only-command-layer)).

## Opening files

- Give a data file, a syntax file or both on the command line, as above.
- File > Open data opens a .sav or .zsav file at once.
- An Excel workbook (.xlsx, .xlsm, .xls) or a text file (.csv, .txt, .tab, .dat) opens the Open Data dialog first. It writes `GET DATA`:
  - for a workbook: the sheet, whether the first row holds the variable names, and a range such as A1:F200 (empty reads the whole sheet);
  - for a text file: the delimiter (comma, semicolon or tab; the one the first line uses most is chosen), and whether the first line holds the names, with that line shown. Every variable is read as `AUTO`.
- After an Open Data run, File > Save data asks for a new .sav file.
- File > Recent files lists the ten files opened or saved last.
- Files dropped on the window from the file manager open too. A .sav or .zsav file opens as data, a workbook or text file in the Open Data dialog, and a .sps file in a syntax window. One data file and one syntax file open at a time; the status bar names the files left out.
- On macOS, a .sav, .zsav or .sps file opened from Finder (a double-click or Open With) is meant to open the same way. This is not yet tested on a Mac.

## Exporting data

File > Export data writes `SAVE TRANSLATE`. The file name chosen sets the type: an Excel workbook (.xlsx), a CSV file (.csv) or tab-delimited text (.txt). The dialog offers:

- a first row of variable names, variable labels or none;
- value labels instead of values;
- user-missing values as empty;
- the variables to export (none chosen: all, in the order of the dataset).

An existing file is replaced once the file dialog has asked.

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

- File > New syntax opens a new window, named Syntax1, Syntax2 and so on until it is saved. File > Open syntax, Recent files and a dropped .sps file open the file in a new window; a file that is open already brings its window to the front.
- Save syntax and Save syntax as save the window they are chosen in.
- File > Close or the window's close button closes it. When its text is unsaved, a question offers **Save**, **Don't save** and **Cancel**.
- The toolbar has **Run all**, **Run selection** and the **Active** list. The Active list shows the datasets of the session, with the active one chosen. Choosing another runs `DATASET ACTIVATE name.`, which shows in the log like any run.
- Every window runs on the same session and the same active dataset. A run started while another goes on waits its turn, and the status bar says how many runs wait.
- **Paste** in a dialog or the output puts the syntax at the end of the designated syntax window: the one opened last, or the one chosen with Window > Designate window. With no syntax window open, a new one opens.

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

Every window has the same menu bar. The Window menu lists the open windows; a click brings one to the front. The designated syntax window is marked `+`. **Designate window** makes the syntax window it is chosen in the designated one.

The program ends when the last window closes. The main window can close while syntax windows stay open; opening data or a new result opens it again.

## The Output tab

Every result table, beside a navigator that groups the tables by run and command. A click on a run in the navigator folds it away.

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
- **Delete** removes them.
- **Export selection** saves only them, as a web page or an Excel workbook (one sheet per table, or all tables on one sheet).
- **Clear output** removes every table.

### Exporting all output

- File > Export output as HTML saves the whole output as one web page.
- File > Export output as Excel saves it as a workbook, with **One sheet per table** or **All tables on one sheet**, laid out as `--xlsx` does.
- From syntax, OUTPUT EXPORT writes the tables of an output document to a workbook, web page, text or CSV file. OUTPUT NEW, NAME, ACTIVATE and CLOSE group the tables into documents. The Output tab shows all documents together, and the window keeps them from run to run.

## Menus

### File

Open data, Save data, Save data as, Export data; New syntax, Open syntax, Recent files, Save syntax, Save syntax as; Export output as HTML, Export output as Excel; Close, Exit.

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
[removed]
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
  - Descriptive statistics, effect sizes, observed power, parameter estimates, homogeneity tests and the significance level.
- **Repeated Measures** (`GLM` with `WSFACTOR`):
  - Within-subjects factors, each with a name and its number of levels, and one variable for each combination of levels.
  - Between-subjects factors and covariates, and the type of sums of squares.
  - Descriptive statistics, effect sizes, observed power and the significance level.
  - One measure only, since `MEASURE` is not supported yet.

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
- Crosstabulations of groups and variables in rows, columns and layers. Each variable has its range, starting from its lowest and highest labeled value.
- Row, column and total percentages, based on cases or on responses.
- Leaving out cases missing on any variable of a dichotomy or category group.

### Transform

- **Compute Variable**, **Recode** and **Count Values within Cases**.
- **Rank Cases**: every RANK function, with new names or the default ones; ranks within groups; from the smallest or the largest value; ties; the proportion formula; user-missing values.
- **Automatic Recode** (`AUTORECODE`): into new variables, from the largest value, one scheme for all, blank strings as missing, and a table of the values.

### Help

- **Documentation** opens the documentation online.
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
- says how many warnings a run gave (the warnings appear under their command in the log); a click there shows the log, in the Output tab when you are in the Data or Variables tab;
- shows errors, whatever the tab;
- names, at its right, the split, filter and weight in force, such as `Split by region (layered)`, `Filter: filter_$` and `Weight: weight`. A click on one opens Split File, Select Cases or Weight Cases.

The log, in every syntax window and under the Output tab:

- lists every command with what it changed;
- has **Copy syntax**, which gives a script that repeats the session with `statscraft run`;
- is turned off by Edit > Settings > Grids and output > Log under Syntax and Output. A run with warnings shows the log even when it is off.

The Data and Variables tabs have no log, so the grid gets the whole height.

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

## What the window remembers

The window keeps these in `statscraft/window.txt` in the user's config folder (`%APPDATA%` on Windows, `$XDG_CONFIG_HOME` or `~/.config` elsewhere):

- its size and position;
- the folder of the last file dialog;
- the ten files opened or saved last (File > Recent files);
- the settings of Edit > Settings;
- the column widths of the Variables tab.

## Closing and unsaved work

The program ends when the last window closes, or with File > Exit, the Windows installer of an update, or Restart now. It first checks for unsaved work in every window.

- Unsaved work is data changed since it was last opened or saved, syntax whose text differs from its file, and new syntax that is not empty.
- Data counts as saved after a run that ends with `GET`, `GET DATA` or `SAVE`, whether from a menu, a syntax window or the command line.
- With nothing unsaved, the program ends at once.
- Otherwise one question lists what is unsaved, with **Save**, **Don't save** and **Cancel**.
  - Save saves the syntax of every window and the data, each to its file, or with a file dialog when there is none yet. The program ends once all are written. A canceled file dialog or a failed save keeps it open.
  - Cancel, Escape or a click beside the question keeps it open.

Closing one syntax window while others stay open asks only about its own syntax. Closing the main window then loses nothing: the data stays in the session.

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

- The window runs the syntax exactly as its own Run button does. The tables appear in the Output tab, the commands in the log, and the data grid shows the result.
- The command line prints what it prints for a file session, with `"session": "gui"`. `saved` is true when the run changed the window's dataset. Nothing is written to disk unless the syntax says SAVE.
- Relative paths in the syntax are read from the folder where `statscraft` was started. The window stays in that folder.

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
