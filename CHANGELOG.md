# Changelog

## Unreleased

- `statscraft mcp`: an MCP server over standard input and output, so an AI agent app can describe data, look up commands, check and run syntax through tools. Calls go to the open window when there is one, else to a session in the user data folder. See [the MCP server](docs/mcp.md).

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
