# Differences and limits

StatsCraft runs syntax compatible with the reference program syntax. Its behaviour follows the published documentation of the reference program. GNU PSPP, another free program for the same language, is used only to compare numbers. This page lists where StatsCraft differs from either on purpose, the choices it made where the documentation says nothing, and what does not work yet.

## Where StatsCraft follows the documentation and GNU PSPP does not

- **RECODE ... INTO an existing variable** keeps the old value of a case that no rule matches. A new target variable gets system-missing.
- **Fractional weights** are used as they are, not rounded, in counts, percentages and statistics. Only significance tests, confidence intervals, standard errors and T-TEST effect sizes round them, as documented.
- **Zero, negative and missing weights** leave the case out of FREQUENCIES, unweighted CTABLES counts and the MEANS minimum and maximum. CORRELATIONS counts a negative weight as 0.
- **DESCRIPTIVES**: "Valid N (listwise)" always counts the cases valid on every listed variable. String variables are skipped with a warning. Saved z-scores are named Z plus the variable name, then ZSC001 to ZSC099, STDZ01 to STDZ09, ZZZZ01 to ZZZZ09 and ZQZQ01 to ZQZQ09.
- **CROSSTABS**: expected counts in the margins are counts; FORMAT=DVALUE reverses the rows only; `/CELLS` and `/STATISTICS` without a list are accepted.
- **CTABLES**: an empty count is 0 and EMPTY decides how it shows; the median and percentiles use the averaged empirical distribution; a variable without a measurement level is classed by all the documented rules. With CLABELS ROWLABELS=OPPOSITE the moved categories sit inside the column categories and outside the summary labels.
- **Expressions**: `0 ** missing`, `missing ** 0` and `0 / 0` are missing; MISSING() sees user-missing strings; month 0 in a date is invalid; RND and TRUNC allow for rounding error relative to the size of the value.
- **ADD FILES** keeps the weight when every file is weighted by the same variable. **MATCH FILES** fails when a TABLE file holds a key twice.
- **ONEWAY**: Games-Howell uses separate variances.
- **FREQUENCIES**: percentiles and the median use (W + 1) p as documented (PSPP's default uses another definition, and with weights that are not whole numbers it leaves the formula); NTILES gives only the cut points between the groups; MISSING=INCLUDE counts user-missing values in the statistics and lists them as valid; ORDER=VARIABLE gives a Statistics table per variable; the mode is the value with the largest weight; the minimum, maximum and range do not depend on the order of the table; GROUPED is supported, and DVALUE, AFREQ and DFREQ are ignored with percentiles, as is the median with AFREQ and DFREQ.
- **LOOP**: when the index passes its last value, the index variable holds the counter (one step past the last value), and a zero-trip loop leaves the first value in it.
- **$CASENUM** counts every case read in the data pass, also those a later SELECT IF drops.
- **N OF CASES** limits the active dataset at the next data pass after GET too, after SELECT IF and SAMPLE.
- **SORT CASES** turns SPLIT FILE off, with a warning.
- **AUTORECODE /BLANK=MISSING** gives a blank string a user-missing code after the valid ones.
- **DATA LIST**: the measurement level of the variables read follows the documented rules for unknown levels (the values decide, not the format).
- **MEANS, CORRELATIONS, EXAMINE**: undefined skewness and kurtosis are left empty; the CORRELATIONS listwise descriptives, the EXAMINE hinges with small weights, percentiles of small groups, CINTERVAL, listwise totals and the default descriptives table follow the documentation.

## Syntax StatsCraft accepts beyond the documentation

These are accepted where the documented syntax refuses them. Results on documented syntax do not change.

- ADD FILES widens a string variable to its widest width across the files.
- COUNT and RECODE accept MISSING for string variables.
- CTABLES accepts MIN and MAX for MINIMUM and MAXIMUM, and a format before the label in a summary list.
- COUNT is allowed inside DO IF.
- A quoted string may run on over line breaks, blank lines included, until its closing quote; the breaks stay in the text. Survey tools write labels that way. An error after such a string says where it started, since a missing quote also makes a string run on.
- PRESERVE and RESTORE accept words after them (`PRESERVE LOCALE.`) and ignore them with a warning. INCLUDE accepts the file name without FILE=. DISPLAY accepts `FILE LABEL`.
- CTABLES /TITLES takes several quoted lines after TITLE, CAPTION and CORNER, as its subcommand diagram shows.

## Choices where the documentation is silent

- A logical operand other than 0, 1 or missing is an error. `0 ** 0` is missing.
- RECODE COPY into a narrower string cuts the value. A reversed range such as `10 THRU 1` is turned around, with a warning.
- Harmonic and geometric means need every value positive; otherwise the cell is empty.
- MEANS shows one report per dependent variable. Its ANOVA uses the first control variable only.
- A CORRELATIONS coefficient marked by NOSIG shows 3 decimals and stars, explained in a caption.
- Levene's test is based on the mean.
- CROSSTABS shows Fisher's exact test for every 2 x 2 table.
- FREQUENCIES shows the Statistics table only when statistics or percentiles are asked for or a frequency table is left out. LIMIT counts the missing values too. Values with equal frequencies stay in ascending order. Grouped percentiles: with a width or boundaries a percentile is interpolated within the first group whose cumulative weight reaches it; a value on a boundary belongs to the group above, except the last boundary; values outside the boundaries form open groups, where percentiles are undefined.
- EXAMINE: Shapiro-Wilk uses Royston's algorithm for 3 to 5000 cases; the Lilliefors significance comes from a published table and shows ">0.200" beyond it; M-estimators start from the median, and a cell is empty when the absolute deviation is 0; percentiles below the first or above the last value take the minimum or maximum.
- CTABLES significance tests: column letters run across the whole table (A, B, ... then AA), so no two column groups share a letter; the Total of an outer variable is a subtable of its own and is tested; a period marks every column left out of a comparison; ADJUST defaults to BONFERRONI and MEANSVARIANCE to ALLCATS; significant means p below alpha (Benjamini-Hochberg: at most alpha); a repeated COMPARETEST of the same type replaces the earlier one; the means test needs MEAN, UMEAN alone does not qualify.
- CTABLES layers give one table per layer category or combination, named in the title. A subtotal stays shown under EMPTY=EXCLUDE. KEY=function sorts categories without a value last and keeps value order for ties. Computed categories are left out of totals and tests.
- SPLIT FILE LAYERED merges the tables of the groups into one, the split variables as the outermost row labels; a column that only some groups have is empty in the others. SEPARATE names the group in each title. AUTORECODE recodes every case, also those a filter leaves out, and its new variables are nominal.
- A data pass in which a case reads a value an earlier case left (a scratch variable it has not assigned, or LAG) runs case by case from the first such command to the end of the pass; a command that is not a transformation in between ends that part. TEMPORARY lasts until the end of a run at most.
- DATA LIST: a blank line of LIST data is a case of missing values; a lone period is system-missing; with given delimiters a string keeps the blanks before it; two-digit years fall in the 100 years from 69 years back. SAMPLE uses the same random seed in every session.
- Multiple response sets: in a category set a value that is user-missing in any of its variables is a missing category, and an empty string counts as a value.
- Macros: a body that ends with a period ends the calling command only when the call is the last thing in it; in the middle of a command (a title macro inside CTABLES) that period is dropped. Macro expressions compare as numbers when both sides are numbers and else as text with case. A macro variable that was never set stands for its own name. Expansion lines carry the line number of the call, and errors in them name the macro.
- Syntax files: one that is valid UTF-8 is read as UTF-8, any other in the system's legacy code page (on Windows its ANSI code page; elsewhere the locale's character set, else windows-1252) unless `--encoding` names one. INSERT and INCLUDE paths are relative to the folder of the file that holds them, or to the folder of the last CD.
- BEGIN PROGRAM blocks are skipped with a warning; the run goes on.
- DATASET ACTIVATE warns when the dataset it replaces has no name, since that one is closed. A command-line session keeps its named datasets between runs.
- SET FORMAT applies to new numeric variables of the commands after it that do not read a file. Settings that only change how output looks are accepted with a warning.
- DOCUMENT and ADD DOCUMENT add `(Entered YYYY-MM-DD)` after their lines. ALTER TYPE from string to number reads F, COMMA and DOT values; a type change drops value labels and missing values. AGGREGATE /PRESORTED is accepted; groups are formed by value either way.
- GET DATA /TYPE=TXT: without DELIMITERS a tab or a comma separates values; a qualified value may hold line breaks; DATETIME values read as a date and a time of day in the usual orders; a format without width takes the width of the longest value (strings) or F8.2.

## Not supported yet

- **Commands.** The list of commands of the language that StatsCraft recognises but does not run is at the end of [the command index](commands/README.md#not-supported-yet). That list is generated from the program, so it is always current. A script that uses one stops at that line, and nothing is changed.
- **Subcommands.** Every command page lists what it accepts. Notably: FREQUENCIES draws no charts (BARCHART, PIECHART and HISTOGRAM are accepted with a warning); ONEWAY has no POLYNOMIAL, RANGES, EFFECTS or WALLER; NONPAR CORR has no MATRIX or CI; EXAMINE computes the normality tests but draws no plots (STEMLEAF, BOXPLOT and HISTOGRAM are accepted with a warning, SPREADLEVEL is refused); macro arguments with !NOEXPAND are expanded anyway; USE takes case numbers only with /PERMANENT; APPLY DICTIONARY has no NEWVARS; GET DATA reads text only (no XLS, XLSX or ODBC); SET BLANKS, EPOCH, SEED and similar are kept but not followed yet.
- **Transformations.** RANK, LEAVE, PRINT, WRITE, END CASE and input programs are not supported; DATA LIST reads only F, N, E, COMMA, DOT, DOLLAR, PCT, A, DATE, ADATE, EDATE and SDATE and UTF-8 files; commands such as VALUE LABELS cannot stand inside LOOP; AUTORECODE has no templates.
- **Charts.** No command draws charts yet.
- **Data files.** StatsCraft reads and writes .sav and .zsav files. Portable (.por) files and spreadsheets cannot be read yet; text files are read with DATA LIST. An old .sav file that does not state its code page is read as Western European (windows-1252); there is no way to choose another encoding yet.
- **Output.** Tables come out as Markdown, JSON, HTML (from the window) and Excel workbooks.
- **Installer.** The Windows installer is not signed yet, and there are no packages for macOS or for ARM processors.
