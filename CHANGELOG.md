# Changelog

## Unreleased

- NOMREG: multinomial logistic regression of a nominal dependent variable (numeric or string) on factors and covariates, with BASE and ORDER for the reference category, MODEL with interactions and nested effects, FULLFACTORIAL, INTERCEPT, CRITERIA, SCALE, SUBPOP, weights and MISSING. Tables: Case Processing Summary, Model Fitting Information (with AIC and BIC), Goodness-of-Fit, Pseudo R-Square, Likelihood Ratio Tests, Parameter Estimates with the interval of Exp(B), Classification, and the covariance and correlation matrices. SAVE adds the category probabilities, the predicted category and its probability, and the probability of the observed category. Stepwise methods, TEST and OUTFILE are not supported yet. See [NOMREG](docs/commands/nomreg.md).
- PLUM: ordinal regression with the logit, probit, complementary log-log, negative log-log and Cauchit links, factors and covariates, LOCATION and SCALE effects, weights and missing values. Tables: Case Processing Summary, Model Fitting Information, Goodness-of-Fit with a warning about empty cells, Pseudo R-Square, Parameter Estimates, Test of Parallel Lines, Cell Information, the covariance and correlation of the estimates and the iteration history. SAVE adds the category probabilities, the predicted category and the probabilities of the predicted and the observed category. Not supported yet: TEST. See [PLUM](docs/commands/plum.md).

## 0.9.0 (2026-11-12)

- Agents inside WSL: on Windows, Edit > Settings > AI agents and `statscraft connect` also find Codex (including the ChatGPT app's Codex in WSL mode), Gemini CLI, Antigravity and Claude Code installed in a WSL distribution, listed as for example Codex (WSL: Ubuntu) or `codex-wsl:Ubuntu`. The entry starts the Windows program through its `/mnt/c` path, so the agent sees the open window, and tells the server the distribution. The server then takes Linux paths in tool arguments, gives the trail file and the workbook in both forms, and `window_status` says the platform and the distribution; file names inside syntax stay Windows paths. Listing the apps no longer holds up the window while WSL starts. See [Agents inside WSL](docs/connect.md#agents-inside-wsl).
- Connecting AI apps moved to Edit > Settings > AI agents; Help > Connect an AI agent opens it there. For Claude Desktop, **Install extension** downloads the extension (.mcpb) of this version for your system, checks it against SHA256SUMS and opens it, so Claude Desktop offers to install it; this does not depend on Claude Desktop's settings file, which some versions rewrite. **Connect via config file** stays as the second way and now says where it wrote. **Check** on each app reads its settings files again and shows whether the Statscraft entry is still there. See [Connect your AI agent](docs/connect.md).
- A change of Claude Desktop's settings file while Claude Desktop runs now comes with a warning, in the window and in `statscraft connect`: quit it completely and connect again.
- Codex is listed as Codex, also the Codex view of the ChatGPT app, with a note that ordinary ChatGPT chats cannot use tools on this computer.
- Every step leaves syntax. Each run that works and is sent to the open window, by an agent or with `statscraft run -s gui`, is added to a syntax window named Agent under a comment line with the time, so the person can read, change, rerun and save it. A file session adds the same to `syntax.sps` in its folder, with the date. `trail` in the JSON says where; failed runs and checks leave nothing. See [the MCP server](docs/mcp.md#every-step-leaves-syntax).
- The MCP server works in the person's syntax windows: `list_syntax_windows`, `read_syntax`, `edit_syntax` (the change shows at once and Ctrl+Z in the editor takes it back), `new_syntax_window`, `open_syntax_file` and `save_syntax`, which never replaces another file unless told to. `run_syntax` with `window` and `lines` runs from a syntax window as its own Run does. These need a window of this release; an older window answers that it needs an update. See [syntax windows](docs/mcp.md#syntax-windows).
- Saved output: the output window can be saved as a Statscraft output file (.scout) and opened again, with each command's syntax, warnings, tables and charts. File > Save output, Save output as and Open output in the window; OUTPUT SAVE and OUTPUT OPEN in syntax, so `OUTPUT OPEN` followed by `OUTPUT EXPORT` converts a saved file to a workbook, document or web page without the window. Deleting tables in an output window now deletes them from its document too, and closing a window with unsaved output asks first. The file is plain JSON that agents read directly; .scout files open by double-click after installing. See [output files](docs/command-line.md#output-files).
- Charts drawn: the output window shows each chart as a picture, sized to the window, on light or dark paper, in the brand's petrol and amber. Bar charts in every layout, pie, line, area and scatter charts, the histogram with its normal curve, boxplots, Q-Q, P-P, spread-versus-level and the other point plots, the dendrogram, the icicle plot and the tree diagram are drawn by Statscraft itself; a stem-and-leaf display stays text. A right click offers Show data (the chart's numbers as a table), Save chart as PNG and Save chart as SVG; on Windows, Copy also puts the picture beside the editable chart. `statscraft run --charts DIR` writes every chart of a run as an SVG file, or a PNG file with `--chart-format png`. See [the window](docs/window.md#charts) and [the command line](docs/command-line.md#chart-pictures).
- Charts as data: FREQUENCIES BARCHART, PIECHART and HISTOGRAM (with a normal curve) and CROSSTABS BARCHART now make charts. `--json` gives each chart's kind, labels and numbers under `commands[].charts`, `--vega-lite` adds a Vega-Lite specification, and the text output shows the chart's numbers as a table. See [the command line](docs/command-line.md#charts).
- Charts in the office programs: a bar, pie, line or area chart becomes a real, editable office chart with its numbers in a workbook inside it, in the theme of where it lands. In the window a right click on a chart offers Open in PowerPoint, Open in Excel and Open in Word, and on Windows Copy puts it on the clipboard so that PowerPoint and Word paste it as their own chart. `--xlsx` and OUTPUT EXPORT /XLSX put each chart beside its data table, and OUTPUT EXPORT now writes Word documents (/DOC) and presentations (/PPT) with the tables and charts. See [the window](docs/window.md#charts-in-the-office-programs).
- Statistical plots as data: EXAMINE boxplots, stem-and-leaf plots, histograms, normal and detrended Q-Q plots and spread-versus-level plots (with the Test of Homogeneity of Variance); the ONEWAY means plot; the FACTOR scree and loading plots; REGRESSION residual histograms, normal P-P plots, scatterplots and partial regression plots; the CLUSTER dendrogram and icicle plots; the TREE diagram; the LOGISTIC REGRESSION classification plot. Each has its data table, its JSON shape under `plot` and a Vega-Lite specification. EXAMINE now makes its default stem-and-leaf plot and boxplot, and CLUSTER its default icicle plot, as the manual says. See [the command line](docs/command-line.md#charts).
- More plots as data: UNIANOVA and GLM PLOT (profile plots of the estimated marginal means as line or bar charts, spread-versus-level plots, the observed by predicted by standardized residual plot); DISCRIMINANT PLOT (the cases on the first two functions with the group centroids, all groups or one plot per group, the Casewise Statistics table, and the territorial map as one polygon per group); TREE PLOT GAIN, RESPONSE and INDEX (line charts over percentile groups). The profile and gain charts open in the office programs as editable charts.
- ONEWAY post hoc tests with many groups are fast: Tukey, GT2 and Gabriel worked out the same critical value again for every pair of groups, so 60 groups took three minutes; now it takes seconds.
- The Linux packages run on Ubuntu 24.04 again: they are built there, not on the newest runner.

## 0.8.1 (2026-11-12)

- Help > Connect an AI agent in the window: connects Statscraft to the AI apps on this computer, or removes it, with clicks instead of `statscraft connect`, and says what to restart. See [Connect your AI agent](docs/connect.md).
- The Data View keeps rows and columns evenly spaced in files with a million cases.

## 0.8.0 (2026-11-12)

- `statscraft mcp`: an MCP server over standard input and output, so an AI agent app can describe data, look up commands, check and run syntax through tools. Calls go to the open window when there is one, else to a session in the user data folder. See [the MCP server](docs/mcp.md).
- `statscraft connect` adds the Statscraft MCP server to the AI agents installed on your computer: Claude Desktop, Claude Code, ChatGPT desktop and Codex, Cursor, VS Code with Copilot, Gemini CLI and Antigravity. It changes only its own entry, keeps a copy of each file before the first change, and says what to restart; `--list` shows the state, `--remove` undoes it. See [Connect your AI agent](docs/connect.md).
- MCP bundles for Claude Desktop: each release has a `.mcpb` for Windows, macOS and Linux that installs the Statscraft server with a double click, no terminal needed.
- A Claude Code plugin in this repository: `/plugin marketplace add statscrafthq/statscraft`, then `/plugin install statscraft@statscraft`. It adds the MCP server and the skill.
- The agent skill `statscraft-analysis` teaches an agent the working loop. It comes with the plugin, as a .zip with each release for claude.ai, and installs in Codex from its GitHub folder. See [Connect your AI agent](docs/connect.md#teach-your-agent-the-skill).
- Each dataset has its own data window and each output document its own output window. A click in a data window makes its dataset the active one (DATASET ACTIVATE); new tables go to the designated output window (Window > Designate window); closing a window asks about unsaved data. Checked on a virtual display only so far, not yet on a real Windows or macOS desktop.
- REGRESSION, FACTOR, MEANS, RECODE, COUNT and DESCRIPTIVES are faster on large data, with the same results.
- Older releases are no longer offered for download; update to this one.

## 0.7.0 (2026-11-12)

- Tables show variable and value labels, names or both, following SET TVARS, TNUMBERS, OVARS and ONUMBERS (labels by default, as the manual says). Edit > Settings has the two choices and keeps them.
- Every dialog that writes syntax ends the same way: the syntax, OK, Paste and Cancel; Escape closes it. Hint lines and instruction tooltips are gone, and so is the View menu (the Data tab has its own value labels switch).

## 0.6.0 (2026-11-12)

- The MATRIX ... END MATRIX language: COMPUTE, PRINT, DO IF, LOOP, READ, WRITE, GET, SAVE, MGET, MSAVE, CALL EIGEN, SVD and SETDIAG, and the whole function library.
- Transformations know 185 of the manual's functions: the distribution families (CDF, IDF, PDF, NCDF, SIG), every date and time function (DATEDIFF, DATESUM, XDATE, CTIME and the rest), the CHAR string functions, REPLACE, NORMALIZE, MEDIAN, VALUELABEL and more. The editor completes them.
- SET BLANKS, EPOCH, FUZZBITS and SCALEMIN now take effect, and sessions keep them.

## 0.5.0 (2026-11-12)

- New procedures: FACTOR (every extraction and rotation, KMO and Bartlett, saved scores), QUICK CLUSTER, CLUSTER (hierarchical, all measures and methods), DISCRIMINANT (direct and stepwise, classification, cross-validation), LOGISTIC REGRESSION (binary, contrasts, stepwise, Hosmer-Lemeshow), UNIANOVA and GLM (sums of squares types 1 to 4, EM means, post hoc, repeated measures) and ANOVA.
- Input programs: INPUT PROGRAM, END CASE, END FILE, REPEATING DATA, FILE TYPE and RECORD TYPE, REREAD and LEAVE.
- MATRIX DATA, REPORT, FINISH, and OUTPUT NEW, NAME, ACTIVATE, CLOSE, DISPLAY and EXPORT (Excel, HTML, text and CSV).
- SHOW knows every keyword; SET LOCALE sets the decimal mark; PRESERVE and RESTORE with a setting name run without a warning.
- Window: dialogs for Factor, K-Means Cluster, Hierarchical Cluster, Discriminant, Binary Logistic, General Linear Model Univariate and Repeated Measures.
- New packages: a portable Windows ZIP without installer, and an app for macOS (Apple silicon and Intel, not signed yet: allow it once in System Settings, Privacy and Security). Both update from the window.

## 0.4.0 (2026-11-11)

- Closing the window asks about unsaved data and syntax (Save, Don't save, Cancel); so do New syntax, Open syntax and Restart now. Nothing unsaved closes without asking.
- One Settings dialog (Edit > Settings) for the editor, grids and output, and updates; the quick menus no longer hold settings.
- Grids: select with the mouse by clicking or dragging over cells, headers and row numbers; Delete clears the selected cells; system-missing shows as a gray dot; numbers stay right-aligned when their labels are too long for the cell; no shadows anywhere.
- A shorter right-click menu on columns and Variables tab rows: sort, Frequencies, Descriptives, rename, insert, copy, delete.
- Analyze dialogs for Linear Regression, Reliability Analysis, the Nonparametric Tests and Partial Correlations.
- New commands: TITLE, SUBTITLE, ECHO, NEW FILE, CLEAR TRANSFORMATIONS, ERASE, SYSFILE INFO, PERMISSIONS, DATE, PRINT, PRINT EJECT, PRINT SPACE, WRITE, PRINT FORMATS, WRITE FORMATS and XSAVE. HOST is refused for safety.

## 0.3.0 (2026-11-11)

- New procedures: REGRESSION (linear, with stepwise methods, statistics and saved variables), RELIABILITY (alpha, split-half, Guttman, parallel models, intraclass correlations), NPAR TESTS (every one-sample, independent and related samples test, exact where the manual says), PARTIAL CORR, MULT RESPONSE and TREE (CHAID and exhaustive CHAID).
- Restructuring and merging: VARSTOCASES, CASESTOVARS, FLIP, UPDATE, SORT VARIABLES, STAR JOIN; ADD FILES and MATCH FILES with IN, FIRST, LAST and MAP.
- Spreadsheets and text: GET DATA reads Excel workbooks; SAVE TRANSLATE writes CSV, tab-delimited text and XLSX. In the window, File > Open data and File > Export data, and dropped workbooks and text files open.
- Dates, times and custom currencies show in their formats in every table, the Data tab and Excel export (as real Excel dates and numbers).
- FILE HANDLE; SET SEED, RNG and MTINDEX with the random number functions; PRESERVE and RESTORE for every setting.
- Command-line sessions keep settings, PRESERVE levels, the CD folder, file handles and the random numbers between runs; describe shows them.
- Faster CTABLES on large stacked tables; shortened function names such as SYS; CROSSTABS /COUNT and /FORMAT.
- Window: dialogs for Multiple Response, Tree, Restructure and Sort Variables.

## 0.2.0 (2026-11-11)

- New app icon and the Statscraft name everywhere: window, installers, file types, documentation and the product string of written .sav files. Installing 0.2.0 over 0.1.x keeps the same install and file associations.
- Window: right-click menus in the Data tab (column headers, cells, row numbers) and the Variables tab: sort, quick Frequencies and Descriptives, prefilled dialogs, measure, rename, insert, delete, filter to a value, keep only those cases, copy.
- Window: the log shows only under Syntax and Output, so the grids use the whole height; cells follow each variable's alignment; cases left out by FILTER have their row number struck through.
- INCLUDE and INSERT without FILE= read the file before the next lines, so the macros of a macro library can be called right after it.
- CTABLES: the NEGPAREN, NEQUAL, PAREN, PCTPAREN, DOT, N and E summary formats; )DATE, )TIME and )TABLE in titles; a function repeated in one summary list gives two columns.
- Window: the Statscraft colors (petrol) for selection, focus, links and OK buttons; Schibsted Grotesk and JetBrains Mono bundled, with digits of equal width so numbers line up; Statscraft Light and Dark syntax colors, Light the default.

## 0.1.1 (2026-11-10)

- FREQUENCIES supports every subcommand (statistics with standard errors, formats, percentiles, ntiles, grouped data, order); chart requests give a warning instead of an error.
- RANK with every function, tie rule and fraction method.
- Encrypted database connections (TLS with certificate checks) and much faster storing and reading of large datasets in PostgreSQL, MySQL, MariaDB, Dolt and SQLite.
- Update checks run quietly in the background; the window no longer shows a notice about them on the first start.
- Window: the Variables tab, with dialogs for type and format, value labels and missing values editors (with pasting many codes at once), row selection with Delete, spreadsheet keys, Ctrl+F to find a variable, Ctrl+C as tab-separated text; Rank, Automatic Recode and Split File dialogs; Select Cases filters by default; split, filter and weight shown in the status bar.
- Syntax editor: themes (Catppuccin, One Dark, Solarized, GitHub), macros in completion, fills its tab. Output: no automatic selection, a margin around tables. Square corners throughout; Help links open in the browser.
- Macros defined in a session last across its runs; macro arguments expand at the call as the manual says; dataset names in MATCH FILES, ADD FILES and AGGREGATE.

## 0.1.0 (2026-11-10, early release)

The first version for people who want to try Statscraft and tell us what breaks. Expect rough edges; please report them on the Issues page of the GitHub repository.

### What it does

- Reads and writes .sav and .zsav files, including labels, missing values, multiple response sets, dates and old encodings.
- Macros (DEFINE with every argument kind, !LET, !IF, !DO and the string functions), INCLUDE and INSERT, syntax files in old encodings, SET, DATASET and the dictionary commands, GET DATA for text files.
- Runs the established statistics command syntax: transformations (COMPUTE, IF, RECODE, COUNT, DO IF, SELECT IF, DO REPEAT, LOOP, VECTOR, AGGREGATE, AUTORECODE, file merging, SPLIT FILE, FILTER, TEMPORARY and more) and procedures (FREQUENCIES, DESCRIPTIVES, CROSSTABS, CTABLES, MEANS, T-TEST, ONEWAY, CORRELATIONS, NONPAR CORR, EXAMINE).
- Custom tables: stacking, nesting, layers, totals and subtotals, percentages, multiple response sets, significance tests with column letters, effective base.
- Desktop window: data and variable views, syntax editor, output view, dialogs that write syntax, Excel and HTML export.
- Command line built for scripts and AI agents: sessions, running parts of a file by line range, JSON output with precise error positions, and a live link to the open window.
- OpenStatSpec: store datasets in SQLite, PostgreSQL, MySQL, MariaDB or Dolt and apply transformation plans in place.

### Known limits

- The Windows installer is not signed yet, so Windows shows an "unknown publisher" warning; see [Getting started](docs/getting-started.md).
- No charts yet. Some commands and subcommands are not supported; see [Differences](docs/differences.md) and `statscraft help`.
