# Differences and limits

Statscraft runs the established statistics command syntax. Its behavior follows the published reference documentation of that syntax. GNU PSPP, another free program for the same language, is used only to compare numbers.

This page lists, by topic and command:

- where Statscraft follows the documentation and GNU PSPP does not (the entry names what GNU PSPP does);
- syntax Statscraft accepts beyond the documentation, marked **Beyond the documentation**. Results on documented syntax do not change;
- the choices Statscraft made where the documentation says nothing;
- what does not work yet, at the end under [Not supported yet](#not-supported-yet).

## Weights

- **Fractional weights** are used as they are, not rounded, in counts, percentages and statistics. Only significance tests, confidence intervals, standard errors and T-TEST effect sizes round them, as documented (GNU PSPP differs).
- **Zero, negative and missing weights** leave the case out of FREQUENCIES, unweighted CTABLES counts and the MEANS minimum and maximum. CORRELATIONS counts a negative weight as 0 (GNU PSPP differs).
- UNIANOVA and GLM round a frequency weight to a whole number, as documented; ANOVA uses weights as they are. See [UNIANOVA and GLM](#unianova-and-glm) and [ANOVA](#anova).

## Expressions and functions

- **Expressions** (GNU PSPP differs): `0 ** missing`, `missing ** 0` and `0 / 0` are missing; MISSING() sees user-missing strings; month 0 in a date is invalid; RND and TRUNC allow for rounding error relative to the size of the value.
- A logical operand other than 0, 1 or missing is an error. `0 ** 0` is missing.
- A function name may be shortened to three or more letters that start only one function; a shared start such as STR (STRING, STRUNC) is an error.
- Harmonic and geometric means need every value positive; otherwise the cell is empty.
- **$CASENUM** counts every case read in the data pass, also those a later SELECT IF drops (GNU PSPP differs).

### Expression functions

- In CDF, PDF, NCDF, NPDF and SIG, a value of x that the distribution does not take gives system-missing (below 0 for CHISQ, outside min to max for UNIFORM, not a whole number in range for the discrete ones), as the manual says. GNU PSPP gives 1 for CDF.BINOM(-1, ...), 0 for CDF.LNORMAL of a negative value, and reads CDF.BINOM of a fraction.
- TIME.HMS takes one to three arguments. Minutes or seconds of 60 or more under a larger unit that is not zero, or a fraction before the last argument that is not zero, give system-missing.
- A .n suffix larger than the number of arguments, as in MEAN.4(a, b), gives system-missing.
- CHAR.LENGTH leaves out trailing blanks. STRUNC drops a character it would cut. LPAD and RPAD with a missing or invalid length give the null string. MBLEN.BYTE counts the bytes of a UTF-8 character, and NPDF.BETA is the derivative of NCDF.BETA (GNU PSPP 2.0.1 gives 1 and other values).
- The CHAR. functions, NTRIM, NORMALIZE, the HALFNRM, IGAUSS, SMOD and SRANGE distributions and the noncentral chi-square, F and t are supported.
- SUBSTR, INDEX, RINDEX, LPAD and RPAD count characters like their CHAR. forms, so they never split a character (GNU PSPP counts bytes). LENGTH, STRUNC and MBLEN.BYTE count bytes.
- The range of x of a distribution is closed: CDF.UNIFORM(max, min, max) is 1 and CDF.PARETO(threshold, threshold, shape) is 0.
- XDATE.HOUR, MINUTE, SECOND, TDAY and TIME of a negative time interval are its parts with its sign, so -1:30:00 has hour -1 and minute -30 (GNU PSPP gives hour -2).
- DATEDIFF counts a month as complete when the day of the month and the time of day are reached. DATEDIFF and DATESUM also take the units in the singular and in any case.
- An empty padding string, a REPLACE count or STRUNC length that is not a whole number of at least 0, and a trimming argument that is not one character give the null string. REPLACE of an empty string changes nothing.
- MBLEN.BYTE at a byte inside a character gives the bytes of that character.
- MIN and MAX of strings compare as strings do (the shorter one padded with blanks) and take no .n.
- LNGAMMA is exactly 0 at 1 and 2.

### Random numbers

- SET RNG=MC is the documented congruential generator, and SET RNG=MT the Mersenne Twister started from MTINDEX as documented.
- UNIFORM, NORMAL and the RV functions draw one value per case they are evaluated for, in case order, and only for the cases a DO IF branch or IF condition chooses. COMPUTE draws for all cases of one command before the next command, except in a pass that runs case by case.
- The distributions use their documented algorithms where the text gives them (gamma: Ahrens and Dieter, Fishman) and else the standard ones (normal by its inverse, binomial, hypergeometric, negative binomial and Poisson by counting), so the values differ from other programs with the same seed.
- A parameter out of range gives system-missing and draws nothing.
- SEED=RANDOM and MTINDEX=RANDOM take the time of day in milliseconds and keep that number as the setting.
- SAMPLE uses the same generator.
- A new session starts from the default seeds; a command-line session goes on where its last run stopped.

## Syntax files and commands

- **Beyond the documentation:** a quoted string may run on over line breaks, blank lines included, until its closing quote; the breaks stay in the text. Survey tools write labels that way. An error after such a string says where it started, since a missing quote also makes a string run on.
- Syntax files: one that is valid UTF-8 is read as UTF-8, any other in the system's legacy code page (on Windows its ANSI code page; elsewhere the locale's character set, else windows-1252) unless `--encoding` names one.
- INSERT and INCLUDE paths are relative to the folder of the file that holds them, or to the folder of the last CD. **Beyond the documentation:** INCLUDE accepts the file name without FILE=.
- BEGIN PROGRAM blocks are skipped with one warning per block; the run goes on.
- A warning that every SPLIT FILE group gives (EXAMINE plots not drawn, for one) is shown once, without a group name.
- END DATA, END REPEAT and END MATRIX standing alone are errors at their line, saying which command they belong to.
- FINISH ends the run as the end of the syntax does: nothing after it is read. A window or command-line session stays open, with its dataset. With a line range only a FINISH inside the range counts. Macros that the lines after FINISH define are still defined, since macros are expanded before the commands are read.
- HOST is read and checked, then refused: a syntax file that runs operating system commands could do anything the user can.

### Macros

- A body that ends with a period ends the calling command only when the call is the last thing in it. In the middle of a command (a title macro inside CTABLES) that period is dropped.
- Macro expressions compare as numbers when both sides are numbers, and else as text with case.
- A macro variable that was never set stands for its own name.
- Expansion lines carry the line number of the call, and errors in them name the macro.
- A session (a command-line session or the window) keeps the macros and the macro settings of SET from run to run. A failed run or a check keeps none, and a run of a line range keeps only the macros defined on its lines, though one defined elsewhere in the text still serves it.

## Settings

### SET

- **SET BLANKS** applies to every numeric variable, as documented, so a blank date field gets the BLANKS value too (GNU PSPP 2.0.1 makes it system-missing). Only a completely blank field takes it: a lone period stays system-missing (GNU PSPP gives it the BLANKS value).
- SET BLANKS gives its value to a completely blank numeric field of DATA LIST (fixed columns, and an empty value between delimiters or commas, inside an input program too), of GET DATA /TYPE=TXT and of NUMBER. A value missing at the end of a LIST line stays system-missing.
- **SET EPOCH=AUTOMATIC** brings back the window from 69 years before the current year (GNU PSPP 2.0.1 then refuses every two-digit year).
- SET EPOCH places a two-digit year in DATA LIST, GET DATA, NUMBER and the DATE. functions. YRMODA keeps the 1900s for years 0 to 99, as its own description says.
- SET FORMAT applies to new numeric variables of the commands after it that do not read a file.
- Settings that only change how output looks are accepted with a warning. SET CCA to CCE are followed.
- SET FUZZBITS is the third argument of RND and TRUNC when it is left out.
- Each setting applies to the commands after it.

### SET LOCALE

- Statscraft always reads and writes Unicode, so a locale changes only the decimal mark (SET DECIMAL), as the manual says a later LOCALE does. The code page part of a name (`.1257`) changes nothing.
- The mark comes from the language (a code such as et or an English name such as Estonian), with a few countries that differ (de_CH, es_MX and others write a point).
- Statscraft starts with DECIMAL=DOT whatever the system's locale, so a syntax reads the same on every computer, and SET LOCALE=OSLOCALE brings DOT back.
- A locale whose mark is not known keeps DECIMAL, with a warning.
- SET UNICODE=OFF has no effect and SHOW UNICODE stays ON.

### SHOW

- SHOW takes every keyword the manual lists.
- A setting Statscraft does not follow shows what Statscraft uses, with a note: no effect (speed, memory and messages) or output does not follow it.
- SCOMPRESSION is ON (SAVE compresses), TMSRECORDING NO, PLUGINS and EXTPATHS none, LICENSE MIT, ENVIRONMENT the operating system and processor, and $VARS lists the system variables Statscraft has.

### PRESERVE and RESTORE

- **Beyond the documentation:** PRESERVE and RESTORE store and bring back every setting (the custom currencies and the macro settings among them), so a SET subcommand name after them (`PRESERVE LOCALE.`, as survey tools write) is accepted without a warning; other words are ignored with one.
- `RESTORE LOCALE` warns only when settings other than the ones it names (and DECIMAL, which a locale sets) changed after PRESERVE, since only then could a reading that restores the locale alone give other results.

### FILE HANDLE

- A handle stands for its path wherever a file is named, quoted or not, and as the first part of a path ('handle/file.sav'). An open dataset of the same name comes first.
- A relative NAME starts from the folder of the last CD at the time FILE HANDLE runs.
- /ENCODING applies to DATA LIST, GET DATA, INSERT and INCLUDE when they give no encoding of their own. RECFORM and LRECL are ignored with a warning.
- SHOW HANDLES lists the handles.

## Reading and writing data

### DATA LIST

- The measurement level of the variables read follows the documented rules for unknown levels (the values decide, not the format), with SET SCALEMIN as the number of different values that makes a variable scale (GNU PSPP has no SCALEMIN).
- A blank line of LIST data is a case of missing values. A lone period is system-missing.
- With given delimiters, a string keeps the blanks before it.
- Two-digit years fall in the 100 years from SET EPOCH (AUTOMATIC: from 69 years back).

### Input programs

- After END CASE inside a loop, LEAVE and scratch variables keep their values (GNU PSPP 2.0.1 resets them).
- A numeric LEAVE variable starts at 0 and keeps its value from case to case (GNU PSPP leaves it system-missing and resets it).
- NUMERIC gives a new scratch variable 0, as the rules for scratch variables say, so the example that skips the first records works as described (GNU PSPP starts it system-missing).
- A DATA LIST without variables passes over a record (GNU PSPP refuses it).
- REPEATING DATA, FILE TYPE and RECORD TYPE are supported.
- INPUT PROGRAM and FILE TYPE run when their END command comes, or at BEGIN DATA when they read inline data, so PRINT output from inside shows there.
- Transformations between a DATA LIST or input program and its BEGIN DATA run after the data are read.
- A run of an input program that reads no record and reaches no END FILE is an error, since it would repeat forever.
- $CASENUM inside counts the case being built.
- After END FILE or the end of the data the case being built is dropped. A DATA LIST that finds no more records after its END variable turned 1 ends the program.
- SKIP on a DATA LIST inside applies once, at the first record of its data.
- Scratch variables of the program are gone at its end.
- REREAD reads again the first record of the last case its data gave.
- A DATA LIST inside FILE TYPE reads the one record its RECORD TYPE matched.

### REPEATING DATA

- A record is at least 80 columns long, or as long as the longest record of its data, and the end column of STARTS and CONTINUED defaults to that length.
- ID reads its columns in F (no decimals) or A, as the variable is numeric or a string.
- A missing end column on STARTS gives no case and a warning; a missing LENGTH gives one case and a warning.

### FILE TYPE

- A record or case identifier without a name is not kept.
- In a grouped file, undefined and skipped records do not end a case.
- In a nested file, the records before the first one of the highest level are skipped without a warning. A record left without records below it at the end of the file gives no case (DUPLICATE=CASE makes one only when another record of its type follows). A case identifier that differs from the one of the highest-level record gives a warning.

### LEAVE

- Outside an input program, the variables must not have existed when the data were last read. The transformations since then run again, case by case, with the variables kept from case to case.

### GET DATA

- **Beyond the documentation:** GET DATA reads a workbook by its content, whatever the extension or the TYPE says.
- /TYPE=TXT: without DELIMITERS a tab or a comma separates values; a qualified value may hold line breaks; DATETIME values read as a date and a time of day in the usual orders; a format without width takes the width of the longest value (strings) or F8.2.
- /TYPE=XLSX, XLSM and XLS:
  - CELLRANGE=FULL reads from the first to the last cell with a value. Empty rows, and empty columns without a name, are left out.
  - Without DATATYPEMIN, a column is numeric only when all its values are numbers (TRUE and FALSE count as 1 and 0), a date when all are dates or times, else a string.
  - Numbers get F with 0 decimals when all are whole, else 2, at least 8 wide. Dates get DATE11, times and durations TIME8, a mix or dates with a time DATETIME20.
  - In a string column, numbers read as short as they read back, dates as yyyy-mm-dd and times as hh:mm:ss. Error cells are empty.
  - READNAMES makes a valid name: other characters become _, a V goes before a name that does not start with a letter, an _ after a reserved word, _2 and _3 after a repeated name. A column without a name is V and its number in the range.
  - A string is as wide as its longest value. ASSUMEDSTRWIDTH caps it for every type (default 255 for XLS only), with a warning for each value cut.

### GET TRANSLATE

- **Beyond the documentation:** the subcommands come in any order, and RENAME, which the overview names, is accepted. GET TRANSLATE /TYPE=XLS reads any workbook GET DATA reads (Excel 95 to 2003 and later), not only files from before Excel 5.
- A tab-delimited file decides each column's type by its first data value, as the manual says. An empty first value makes it numeric, as an empty spreadsheet cell takes the default format, and later text in it is system-missing with a warning.
- Repeated field names get 2, 3 after the first; a name that does not start with a letter gets a V in front.
- A dBASE file adds D_R only when some record is marked deleted, after the fields. Its code page comes from the file's language driver when Statscraft knows it.
- Without FIELDNAMES, a workbook column is named by its letter (a reserved word such as BY gets $).
- RANGE takes A1..K14 and 'A1:K14' (quoted, for the colon), not range names.

### SAVE TRANSLATE

- **Beyond the documentation:** TYPE=XLSX stands for TYPE=XLS /VERSION=12, and TYPE=XLS without VERSION is accepted when the file name ends in .xlsx.
- Lines end with CR LF (RFC 4180). A CSV value is qualified when it holds the delimiter, the qualifier or a line break.
- The manual gives two rules for the default delimiter. Statscraft makes it a semicolon whenever the decimal mark in use is a comma (TEXTOPTIONS DECIMAL, else SET DECIMAL), so a number never needs quotes.
- FORMAT=PLAIN writes a date with a time of day as mm/dd/yyyy hh:mm:ss, and a time with its fraction of a second when it has one. FORMAT=VARIABLE widens a number that does not fit its format instead of writing asterisks.
- A workbook keeps the display format of each number (dates and times as Excel dates, as in the table export). With MISSING=RECODE a numeric user-missing value is #NULL! like system-missing, a string one empty.
- Without EXCELOPTIONS the sheet is Sheet1; further sheets for more cases add (2), (3).

### EXPORT and IMPORT

- **Beyond the documentation:** the subcommands come in any order.
- The file format comes from public descriptions of it and from files examined as black boxes.
- Text is written as UTF-8 (control characters as 0); text that is not UTF-8 is read as windows-1252.
- Every number is written exactly, in the fewest base-30 digits that give it back, and read with exact arithmetic, so Statscraft gives back the same values it wrote (GNU PSPP keeps 11 digits and loses the last bits of some values, such as 1e20).
- Strings are padded to their width, as GNU PSPP misreads shorter ones. Strings over 255 bytes are cut, with a warning.
- Names become upper case and at most 8 bytes. A shortened name gets a warning, and repeated short names get _2, _3 as the manual shows.
- The format has no file label, measurement levels, display settings, attributes or multiple response sets, so those are not kept. IMPORT gives levels as GET DATA does.
- IMPORT reads the records in any order (GNU PSPP writes the weight before the variable count and cannot read its own file back then).
- TYPE=COMM and TAPE are accepted and change nothing. MAP shows the names on both sides.

### MATRIX DATA

- **Beyond the documentation:** a slash before the first subcommand is accepted.
- The rows of a triangle are mirrored, so every matrix in the file is square, as the procedures read it.
- Records keep the order of the data, a matrix where its first row was. The N record of /N comes first in its split.
- VARNAME_ is wider than A8 when a continuous variable has a longer name, so the procedures find every variable.
- The result is split by the split variables (SPLIT FILE LAYERED), so a procedure reads each set of materials on its own.
- Without SPLIT, data left after the records CONTENTS describes are an error, as are a matrix with too few rows and a value that is not a number, each naming its line. Values left on a LIST record after its row are ignored with a warning, as the manual says for an upper triangle in lower format.

### Dates, times and custom currencies

- A two-digit year form (DATE9, ADATE8) shows the last two digits of any year, as the manual says (GNU PSPP shows asterisks outside its century window).
- Seconds that do not fit are cut off, never rounded up (GNU PSPP shows 59.999 seconds as 60.00).
- A date before 1582-10-14 and a WKDAY or MONTH out of range show as asterisks.
- In SET CCA to CCE, a ' makes the next period or comma text (GNU PSPP keeps the '), and a positive number keeps no room for the negative suffix.
- FREQUENCIES, DESCRIPTIVES, MEANS and EXAMINE show the values and the statistics that lie on the variable's scale (mean, median, mode, minimum, maximum, percentiles) of date, time and custom currency variables in their format. CTABLES does so for every variable, as the manual's default.
- A standard deviation, standard error or range of dates shows as days and time (DTIME). The sum and variance of dates, the variance of times, and the statistics of WKDAY and MONTH variables beyond their values show as plain numbers, where the default format would make a date of them.
- Category and group labels of date, time and custom currency variables show in their format in every procedure (CROSSTABS, MEANS, ONEWAY, T-TEST, EXAMINE, CTABLES and the split groups).
- The Data tab of the window shows dates, times and custom currencies in the variable's format, following SET CCA to CCE, and edits dates as yyyy-mm-dd.
- The Excel export writes date, time and custom currency cells as numbers with an Excel number format, so Excel can compute with them: dates and times as Excel day numbers (month names as Jan, not JAN), custom currencies with their prefixes and suffixes, grouped and with the decimal mark of the reader's Excel. What no Excel format shows stays text: JDATE, QYR, WKYR, DTIME, WKDAY, MONTH, negative times and dates before March 1900.

### Datasets

- DATASET ACTIVATE warns when the dataset it replaces has no name, since that one is closed.
- A command-line session keeps its named datasets between runs.
- MATCH FILES and ADD FILES (FILE, TABLE), AGGREGATE OUTFILE and APPLY DICTIONARY FROM take a dataset name, quoted or not, and an open dataset comes before a file of the same name.
- AGGREGATE OUTFILE writes into a dataset that DATASET DECLARE or DATASET NAME defined, and refuses the active dataset's name (OUTFILE=* is that).
- GET FILE and SAVE OUTFILE take files only, as their syntax charts show; DATASET ACTIVATE and DATASET COPY do that for datasets.

## Transformations

### RECODE and COUNT

- **RECODE ... INTO an existing variable** keeps the old value of a case that no rule matches (GNU PSPP differs). A new target variable gets system-missing.
- RECODE COPY into a narrower string cuts the value. A reversed range such as `10 THRU 1` is turned around, with a warning.
- **Beyond the documentation:** COUNT and RECODE accept MISSING for string variables, and COUNT is allowed inside DO IF.

### LOOP

- When the index passes its last value, the index variable holds the counter (one step past the last value), and a zero-trip loop leaves the first value in it (GNU PSPP differs).

### Data passes, N OF CASES and TEMPORARY

- **N OF CASES** limits the active dataset at the next data pass after GET too, after SELECT IF and SAMPLE (GNU PSPP differs).
- A data pass in which a case reads a value an earlier case left (a scratch variable it has not assigned, or LAG) runs case by case from the first such command to the end of the pass. A command that is not a transformation in between ends that part.
- TEMPORARY lasts until the end of a run at most.
- CLEAR TRANSFORMATIONS returns the dataset, N OF CASES, TEMPORARY and the vectors to how the last data pass (or the end of the last run) left them. Commands that touch no data, such as SET and TITLE, stay.

### RANK

- A case with a missing, zero or negative weight gets no rank (GNU PSPP ranks a case of weight 0).
- Missing values of BY variables form groups of their own.
- Labels add "by" and the BY variables.
- A variable named twice is ranked once, with a warning.

### AUTORECODE

- /BLANK=MISSING gives a blank string a user-missing code after the valid ones (GNU PSPP differs).
- AUTORECODE recodes every case, also those a filter leaves out, and its new variables are nominal.

### DATE

- DATE_ reads cycle, month (JAN to DEC) or quarter, then year for long periods, and from the highest part to the lowest for short ones.
- Each part is as wide as its largest possible value (four digits without a period), and MINUTE shows as M.
- The new variables get an F format as wide as their largest value.

### Dictionary commands

- DOCUMENT and ADD DOCUMENT add `(Entered YYYY-MM-DD)` after their lines.
- ALTER TYPE from string to number reads F, COMMA and DOT values. A type change drops value labels and missing values.
- **Beyond the documentation:** DISPLAY accepts `FILE LABEL`.

## Files, merging and restructuring

### ADD FILES, MATCH FILES and UPDATE

- **ADD FILES** keeps the weight when every file is weighted by the same variable; **MATCH FILES** and **UPDATE** keep it when every weighted file uses the same variable (GNU PSPP drops it in all three). When a weighted file leaves the result unweighted, a warning says so.
- **MATCH FILES** fails when a TABLE file holds a key twice.
- **Beyond the documentation:** ADD FILES widens a string variable to its widest width across the files.
- IN, FIRST and LAST are F1.0 and go at the end: IN in file order, then FIRST, then LAST.
- MAP shows the variables of the result at that point with their names in each file (GNU PSPP accepts MAP and shows nothing).
- The subcommands keep the manual's order (FILE, TABLE, RENAME and IN before BY, FIRST, LAST, MAP, KEEP and DROP), and BY comes once.
- MATCH FILES needs BY for IN, and both commands need BY for FIRST and LAST, as the manual says (GNU PSPP accepts them without).
- UPDATE has no FIRST or LAST, as its syntax chart shows.
- The result has the documents of every file, each file once.

### STAR JOIN

- A system-missing key matches nothing, as NULL in SQL. User-missing values match as values.
- Keys not named on SELECT come first in the result, then the SELECT fields, then the IN variables. SELECT * keeps every field in its file's order.
- The weight, filter and split of FROM stay.
- PASSPROTECT with a password for an output file is refused, since Statscraft writes no encrypted files, and encrypted files cannot be read.
- STAR JOIN after TEMPORARY is refused, as MATCH FILES is.

### AGGREGATE

- /PRESORTED is accepted; groups are formed by value either way.

### SORT CASES and SORT VARIABLES

- **SORT CASES** turns SPLIT FILE off, with a warning (GNU PSPP differs).
- **SORT VARIABLES BY NAME** compares the part before a number at the end first and the number after it, so the order never goes round in a circle (GNU PSPP differs).
- SORT VARIABLES: D reverses the whole order, ties too.
- FORMAT orders by the type codes of the data file (A, COMMA, DOLLAR, F, ... DATE ...), then width and decimals.
- VALUES and MISSING put equal definitions together. ATTRIBUTE compares all values of the attribute, without regard to case, as LABEL does.
- A variable without a column width counts as 8 wide, a string as its width up to 32.

### FLIP

- FLIP makes user-missing values system-missing, writes NEWNAMES values in upper case with K_ before a number and _A, _B after a repeated name, numbers default names from VAR001 and keeps whole variable names in CASE_LBL (GNU PSPP differs).
- A NEWNAMES value that does not start with a letter gets K_ too; a missing or blank one keeps the default name.
- CASE_LBL is as wide as the longest name, at least 8.
- Every case is transposed whatever FILTER says, and a table lists the new names.

### VARSTOCASES

- VARSTOCASES and CASESTOVARS are supported, as documented (GNU PSPP differs).
- The new file holds ID, the copied variables, COUNT, INDEX and the MAKE variables, in that order.
- ID and INDEX are nominal (F8.0); COUNT is scale.
- An index of names holds them as the dictionary spells them.
- A MAKE variable may take the name of one of the FROM variables.

### CASESTOVARS

- With several index variables, a name is the root and every index value, each after the separator. With one string index and several variables spread, the value follows the root and separator.
- Indicator names are the root and the index values without separator.
- A new variable's label is the source's label (else its name) and the index values, as in "Systolic, month = Jan".
- A blank string counts like system-missing when deciding whether a variable varies, and in ID and split variables.
- A row group that comes back after another one is an error (the data are not sorted).
- A fixed variable takes the first value of its group that is not missing.
- Order: split, ID and fixed variables as in the file, the variable groups, the indicators, COUNT.

## Procedures

### Several procedures

- MEANS, CORRELATIONS and EXAMINE leave undefined skewness and kurtosis empty. The CORRELATIONS listwise descriptives, the EXAMINE hinges with small weights, percentiles of small groups, CINTERVAL, listwise totals and the default descriptives table follow the documentation (GNU PSPP differs).
- Levene's test is based on the mean.

### FREQUENCIES

- Percentiles and the median use (W + 1) p as documented. GNU PSPP's default uses another definition, and with weights that are not whole numbers it leaves the formula.
- NTILES gives only the cut points between the groups.
- MISSING=INCLUDE counts user-missing values in the statistics and lists them as valid.
- ORDER=VARIABLE gives a Statistics table per variable.
- The mode is the value with the largest weight.
- The minimum, maximum and range do not depend on the order of the table.
- GROUPED is supported. DVALUE, AFREQ and DFREQ are ignored with percentiles, as is the median with AFREQ and DFREQ.
- The Statistics table shows only when statistics or percentiles are asked for, or a frequency table is left out.
- LIMIT counts the missing values too.
- Values with equal frequencies stay in ascending order.
- Grouped percentiles: with a width or boundaries, a percentile is interpolated within the first group whose cumulative weight reaches it. A value on a boundary belongs to the group above, except the last boundary. Values outside the boundaries form open groups, where percentiles are undefined.

### DESCRIPTIVES

- "Valid N (listwise)" always counts the cases valid on every listed variable (GNU PSPP differs).
- String variables are skipped with a warning.
- Saved z-scores are named Z plus the variable name, then ZSC001 to ZSC099, STDZ01 to STDZ09, ZZZZ01 to ZZZZ09 and ZQZQ01 to ZQZQ09.

### CROSSTABS

- Expected counts in the margins are counts; FORMAT=DVALUE reverses the rows only; `/CELLS` and `/STATISTICS` without a list are accepted (GNU PSPP differs).
- Fisher's exact test shows for every 2 x 2 table.
- /COUNT with a keyword left out takes CELL or ROUND, as the dialog writes `/COUNT ROUND CELL`. Rounding takes halves away from zero.

### MEANS

- MEANS shows one report per dependent variable. Its ANOVA uses the first control variable only.

### CORRELATIONS

- A coefficient marked by NOSIG shows 3 decimals and stars, explained in a caption.

### PARTIAL CORR

- The significance is two-tailed by default, as the Command Syntax Reference says (the Algorithms manual says one-tailed).
- The partial of a variable with itself shows 1 with df 0.
- CONDENSED marks coefficients at 0.05 or less with * and at 0.01 or less with **, and shows them with three decimals.
- SERIAL leaves the lower half of a square matrix empty.
- With CORR, the zero-order block also shows the control variables.

### ONEWAY

- Games-Howell uses separate variances (GNU PSPP differs).

### EXAMINE

- Shapiro-Wilk uses Royston's algorithm for 3 to 5000 cases.
- The Lilliefors significance comes from a published table and shows ">0.200" beyond it.
- M-estimators start from the median, and a cell is empty when the absolute deviation is 0.
- Percentiles below the first or above the last value take the minimum or maximum.

### NPAR TESTS

Where GNU PSPP differs:

- The sign and McNemar tests use the binomial distribution up to 25 changes and the normal or chi-square approximation above (GNU PSPP is always exact).
- The two-sample median test has the documented continuity correction. The median of the median test is the middle case or the mean of the two middle cases.
- K-S with normal parameters taken from the data gives the Lilliefors significance, and the Smirnov significance uses the documented three terms.
- The Poisson K-S compares the distributions at each value and at the whole number below it.
- The Jonckheere-Terpstra statistic counts the pairs in the order of the groups.
- Weights repeat the cases in the ranks of Mann-Whitney and Wilcoxon and in Cochran's Q.
- Mann-Whitney adds the exact significance for small samples.

Where the documentation is silent:

- EXPECTED applies to the CHISQUARE before it.
- The binomial test with a proportion other than 0.5 is one-tailed in the observed direction, as the Command Syntax Reference describes (the Algorithms formula sums over the smaller category). Without values, the first value in the file is the first binomial category.
- K-W and J-T take every value between the two given ones that occurs, in the order from the first to the second.
- Two-sample K-S subtracts the distribution of the smaller group from that of the larger, as the Algorithms manual says (the first group when both are as large).
- Wald-Wolfowitz with ties across the groups shows the fewest and the most runs. Up to 30 cases the significance is exact, above it the normal approximation of the runs test.
- The exponential K-S needs values of at least 0.
- Marginal homogeneity uses the values as scores and turns into the McNemar test with two values.
- Wilcoxon reports Z from the smaller rank sum, which says which ranks it is based on.
- METHOD=EXACT and METHOD=MC give exact significances for the binomial, sign, McNemar, Wilcoxon (given the tied ranks), Mann-Whitney (given the tied ranks) and runs tests, as exact results take the place of simulated ones. The other tests stay asymptotic with a warning.
- TIMER, CIN, SAMPLES, KS_SIM and SAMPLE are accepted and change nothing.
- A weight below 0.5 drops the case.

### REGRESSION

- METHOD takes variable lists and STEPWISE, FORWARD, BACKWARD and REMOVE (GNU PSPP differs).
- With ORIGIN, the adjusted R square and the confidence intervals use C - p degrees of freedom, as the ANOVA does.
- Tolerance is 1 - R square of the variable on the other predictors.
- A user-missing value of the dependent or a predictor gives no predicted value or residual.
- Saved variables are named PRE_1, RES_1 and so on.
- Subcommands may come in any order; the last STATISTICS, CRITERIA and ORIGIN apply.
- PAIRWISE takes each variable's mean and variance from its own valid cases, and the smallest count of a pair as N.
- With ORIGIN, the Descriptive Statistics show the real means, with standard deviations and correlations about 0.
- STEPWISE may remove any variable of the equation, also one an earlier METHOD put there, as its description reads. BACKWARD removes only its own. Equal F values go to the variable named first, and MAXSTEPS limits each stepwise method.
- ENTER leaves out a variable that would break the tolerance, with a warning.
- Condition indices are the root of the ratio of the eigenvalues, as Belsley, Kuh and Welsch define them (the reference the manual gives; its printed formula has no root).
- The variance of the constant divides the residual mean square by the sum of case weight times REGWGT, as weighted least squares needs (the manual prints the sum of case weights, the same number without REGWGT).
- Cases outside the analysis (missing on a variable that is not in the equation, or on the dependent) get predicted values, their standard errors, intervals, leverage and Mahalanobis distance, and with a valid dependent the residuals by the manual's formulas for such cases. Cook's distance, DFBETA, DFFIT and COVRATIO go only to the cases of the analysis with a case weight of at least 1.
- Saved variables are F11.5 scale variables. A default name takes the smallest free number (PRE_1, then PRE_2). DFBETA(name) and SDBETA(name) give name0 for the constant and name1 and up for the variables in order (from 1 without a constant). MCIN and ICIN take no name. A name that exists is an error.
- Durbin-Watson shows on the last model and is empty with fractional weights.
- XTX shows the swept correlation matrix of the last model.
- The casewise and outlier tables cover the cases of the analysis.
- LABEL changes nothing.

### LOGISTIC REGRESSION

- METHOD takes FSTEP and BSTEP with COND, LR and WALD. Interactions, every contrast type, CASEWISE and SAVE are supported (GNU PSPP differs).
- The terms keep the order of the VARIABLES list (GNU PSPP puts the covariates that are not categorical first).
- Subcommands may come in any order; the last PRINT, CRITERIA, CASEWISE and SAVE apply.
- PRINT without DEFAULT still shows the default tables. SUMMARY shows the last step of each block.
- Newton-Raphson starts from all coefficients 0. BCON compares the absolute change of each coefficient.
- An entering term's score is computed on its columns that are not redundant (EPS).
- A step that would return to a model already fitted in the method ends it, as the manual's stepwise rules say; at most 200 steps.
- Hosmer-Lemeshow groups: the cases sorted by predicted probability, cases with equal predictions together, a group closing once the cumulative weight reaches its tenth of the total.
- Cook's distance is z^2 h / (1 - h) as the Algorithms manual prints it. Leverage, Cook's distance and DFBETA leave the case weight out, as the manual's formulas do.
- The constant has no interval for Exp(B).
- PGROUP of a string dependent is saved as a string.
- Model if Term Removed shows for LR and COND.
- String covariates are categorical. A categorical variable needs two categories among the cases used.
- The Categorical Variables Codings show weighted frequencies.

### UNIANOVA and GLM

- A frequency weight is rounded to a whole number, as documented (GNU PSPP uses fractional weights as they are).
- An interaction with empty cells has as many degrees of freedom as its parameters that are not redundant (GNU PSPP counts every cell, filled or not).
- The Tests of Between-Subjects Effects show the Intercept.
- Covariates, Type IV sums of squares, EMMEANS, POSTHOC, CONTRAST, LMATRIX, SAVE and WSFACTOR are supported.
- The columns of an effect are the level combinations that occur in the data, so an L matrix lists one coefficient per occurring combination.
- Type IV spreads each coefficient of the tested effect equally over the cells of a containing effect at those levels of its other factors where every level with a coefficient has a cell, and sets the rest to 0.
- Estimated marginal means weight the cells of an effect equally over the levels not fixed by TABLES (with empty cells: over the cells that occur). A mean that is not estimable is left empty with a note.
- COMPARE adjusts for the k(k - 1)/2 pairs of the compared factor.
- CONTRAST compares the marginal means of the factor's levels.
- Levene's test uses the absolute residuals of the model in the cells of all factors; with a full factorial and no covariates, also the median, the median with adjusted df and the 5% trimmed mean.
- POSTHOC uses the observed means with frequency weights and the model's error mean square. T2, T3, GH and C need a model with one factor.
- With WSFACTOR, the Tests of Between-Subjects Effects analyze the sum of the measures over the root of their number, the averaged tests use orthonormal polynomial contrasts whatever the contrast type, and the contrast type shapes the Tests of Within-Subjects Contrasts.
- Saved variables are F11.5 scale variables named PRE_1, RES_1 and so on.
- A nested effect such as a(b) shows as DESIGN writes it, and parameter names such as [a=1] keep names and values.

### ANOVA

- **Beyond the documentation:** the first analysis list may come without VARIABLES=, as the later lists of the manual's examples are written.
- UNIQUE uses the Type III hypotheses of the general linear model (the same sums of squares as the manual's sum-to-zero coding when every cell has cases). The other methods take reductions in the error sum of squares, which do not depend on the coding.
- An order of interactions that is not of full rank (empty cells) is dropped with the orders above it, for every method, with a warning.
- With COVARIATES=WITH, both combined rows show the covariates and main effects together, as the manual's table of sums of squares gives them.
- Eta and the betas of MCA sum over every level of the factor (the manual's printed sums start at the second level; Andrews et al. sum over all).
- The MCA columns adjusted for the factors are shown when there are no covariates, with HIERARCHICAL, or with EXPERIMENTAL and COVARIATES=AFTER.
- REG adds a Raw Coefficient column to the covariate rows.
- Cell Means cover each factor and combination of factors up to the highest order of the model.
- Weights are used as they are (UNIANOVA rounds them, as its documentation says).

### RELIABILITY

- The weight variable is used. SPLIT(n) puts n items in the second part, and SPLIT alone splits in halves, the odd item going to the first part (GNU PSPP differs).
- The standardized alpha is always shown with ALPHA.
- The summaries of inter-item covariances and correlations run over every ordered pair of items, the variance dividing by k(k - 1) - 1, as the formula reads.
[removed]
- FRIEDMAN and COCHRAN use the values as they are, so the items must be ranks within each case, or 0 and 1.
- With ICC MODEL(MIXED), the average measures are computed as if there were no interaction, with a caption saying so.
- Cases with a zero or missing weight are not counted in the Case Processing Summary.

### FACTOR

- Standard deviations and covariances divide by W - 1. LISTWISE leaves out a case missing on any variable of VARIABLES, also one that is not on ANALYSIS. EXTRACTION without ROTATION leaves the factors unrotated. The default power of PROMAX is 4. Rotated factors are ordered by their sums of squared loadings (GNU PSPP differs).
- **Beyond the documentation:** /CRITERIA takes DELTA(n) and KAPPA(n), the delta of OBLIMIN and the power of PROMAX when ROTATION gives none, as older versions of the syntax had them.
- Unrotated and oblique factors are reflected so that each column of loadings sums to a positive number, as the manual does for orthogonal rotations. Oblique factors keep the order of the extraction.
- FORMAT=SORT groups the variables by the factor of their largest absolute loading, each group in descending order of it.
- With PAIRWISE, each correlation's significance uses its pair's count, and Bartlett's test and the ML and GLS chi-square use the smallest pair count.
- ML, GLS and ULS take their second derivatives as central differences of the exact first derivatives.
- A method or rotation that does not converge within ITERATE shows its last estimates with a warning.
- Orthogonal rotations stop when the simplicity criterion changes by at most RCONVERGE and no pair of factors turns by more than RCONVERGE in a pass, since the criterion alone can leave the loadings unsettled in the third decimal. OBLIMIN stops when its criterion changes by less than RCONVERGE times its starting value.
- Alpha factoring scales by H^-1/2, the only reading under which the manual's communality update holds.
- Bartlett and Anderson-Rubin scores take U^2 as the unique variances, one minus the communalities.
- For oblique rotations, the Rotation Sums of Squared Loadings are those of the structure matrix.
- Image factoring keeps only the factors whose eigenvalues are above 1.
- With one factor the rotation is skipped with a warning.

### QUICK CLUSTER

- With LISTWISE, a case missing on a clustering variable gets no cluster, and saved values stay with their own case. The weight variable is used in the means (GNU PSPP differs).
- Clusters are numbered by the position of their initial center. A case equally near two centers goes to the lower-numbered one.
- A cluster that loses all its cases keeps its center during the iterations and gets an empty final center, with a warning.
- KMEANS(UPDATE) starts every pass counting each center as one case.
- PAIRWISE measures the distance over the variables a case has, without rescaling.
- Cluster Membership lists the clustered cases with their case numbers in the file.

### CLUSTER

- The cases are numbered in the order they are clustered, and the tables label them with their case numbers in the file.
- When two pairs are equally similar (to 1e-10 of their size), the pair with the lowest cluster numbers joins first.
- The clusters of a solution are numbered in the order of their first case.
- CORRELATION is Pearson's correlation of the two cases' values.
- A proximity that the measure leaves undefined (such as JACCARD of two cases with no presence) stops the command with an error naming the cases.
- The Proximity Matrix leaves its diagonal empty.
- With CENTROID, MEDIAN and WARD on a similarity measure, the most similar pair joins and the update formulas stay those of the manual.
- Without PLOT no chart is asked for, so there is no warning. With VICICLE, HICICLE or DENDROGRAM the command warns that charts are not drawn yet.
- Saved memberships are F8.0 nominal variables labeled with the method and the number of clusters.

### DISCRIMINANT

- A group value that is not a whole number counts as out of range.
- Each function is reflected so that its standardized coefficients sum to a positive number.
- The Structure Matrix orders the variables by the function of their largest absolute correlation, within it by that correlation, and names the variables not entered.
- Classification needs the values of the entered variables only. MEANSUB puts the means of the analysis cases in place of missing ones.
- Classification counts use the case weights. Cases with a zero weight are classified but count for nothing.
- PRIORS takes one value per group with cases, or one per value of the GROUPS range.
- Box's M covers all variables of the analysis block and leaves out groups with a singular covariance matrix. Its F follows Box (1949), b = t2 / (1 - e1 + 2/t2) when e2 < e1 squared (the manual prints a minus before 2/t2).
- Cross-validation leaves out each case with its weight and keeps the entered variables and the priors.
- In a stepwise analysis, the removal of level 1 variables starts once no variable of a higher level can enter. A lone variable is removed too when its F falls below FOUT.
- FPAIR shows the F of the last step.
- The stepwise tables are Variables Entered/Removed (with the criterion after each step), Wilks' Lambda with Rao's F, and the variables in and not in the analysis at every step.

### TREE

- A predictor category smaller than MINCHILDSIZE joins its most similar allowed group during merging (smallest first), so a predictor that cannot give two large enough children is not chosen and the next best one is tried.
- Ties in merging and in the choice of predictor go to the first pair or predictor in order. The floating missing category stays apart when both choices are equally significant. Exhaustive CHAID keeps the earliest of equally significant sets.
- The exhaustive CHAID multiplier of a nominal predictor is I(I^2 - 1)/6, the number of pairs it could merge summed over the steps, as the manual's text defines it (its printed formula divides by 2, which gives 3 for two categories).
- FORCE splits the root whatever the significance.
- Nodes are numbered level by level. Children follow the order of their smallest category, the missing category last.
- Adjusted significances above 1 show as 1.
- SAVE fills the cases in the analysis and leaves the others missing. A name given that already exists is an error, and default names get _1, _2 when taken.
- USEVALUES categories that do not occur get a warning and no row.
- The gain tables of the TARGET categories show without /GAIN too, unless CATEGORYTABLE=NO, as the subcommand's defaults read.
- The case processing summary counts weighted cases.

## Custom tables and multiple response

### CTABLES

- An empty count is 0 and EMPTY decides how it shows; the median and percentiles use the averaged empirical distribution; a variable without a measurement level is classed by all the documented rules (GNU PSPP differs).
- With CLABELS ROWLABELS=OPPOSITE, the moved categories sit inside the column categories and outside the summary labels.
- **Beyond the documentation:** MIN and MAX stand for MINIMUM and MAXIMUM, and a format may come before the label in a summary list. /TITLES takes several quoted lines after TITLE, CAPTION and CORNER, as its subcommand diagram shows.
- Layers give one table per layer category or combination, named in the title.
- A subtotal stays shown under EMPTY=EXCLUDE.
- KEY=function sorts categories without a value last and keeps value order for ties.
- Computed categories are left out of totals and tests.
- NEGPAREN, NEQUAL, PAREN and PCTPAREN show the number in the width of the format and add their marks outside it.
- )DATE and )TIME in titles are the local date and time as YYYY-MM-DD and HH:MM:SS.
- A function given twice in one summary list is two columns.
- /VLABELS wins over SET TVARS and TNUMBERS, and KEY=LABEL sorts by the value labels whatever the table shows.

Significance tests:

- Column letters run across the whole table (A, B, ... then AA), so no two column groups share a letter.
- The Total of an outer variable is a subtable of its own and is tested.
- A period marks every column left out of a comparison.
- ADJUST defaults to BONFERRONI and MEANSVARIANCE to ALLCATS.
- Significant means p below alpha (Benjamini-Hochberg: at most alpha).
- A repeated COMPARETEST of the same type replaces the earlier one.
- The means test needs MEAN; UMEAN alone does not qualify.

### MULT RESPONSE

- **Beyond the documentation:** MULT RESPONSE has none of the documented size limits (20 groups, 100 variables, 5 dimensions, 200 rows and columns). A slash between two groups of GROUPS is accepted, as an example in the manual writes it.
- A value a case gives twice in a category group counts twice (the frequencies are summed over the variables, as described).
- Fractional weights count as they are. A case with a missing, zero or negative weight is left out, also from the missing cases.
- With BASE=CASES, the row and column totals of a crosstabulation are the weighted responses of that category among the cases valid in the table, and the grand total the valid cases. With BASE=RESPONSES and in PAIRED tables, they are sums of the cells.
- PAIRED pairs as many variables as the shortest category group of the table has. A pair with a missing or out-of-range value is skipped.
- Control variables give one block of rows per category in one table.
- CONDENSE shows values instead of labels in one column, not the printed three-column layout. DOUBLE and ONEPAGE are ignored with a warning.
- Group names show as written.
- /FORMAT=NOLABELS wins over SET TVARS and TNUMBERS.

### Multiple response sets

- In a category set, a value that is user-missing in any of its variables is a missing category, and an empty string counts as a value.

## Output

### Names and labels in tables

- SET TVARS and TNUMBERS steer the variables and values in the headers, row labels and cells of every procedure's tables. OVARS and ONUMBERS steer them in the table titles, which make the outline (the navigator of the window, the headings of exported output).
- BOTH puts the name before the label (`q1 Satisfaction`) and the value before its label (`1 A great deal`), as the Custom Tables manual shows values.
- Tables that give the name or value and the label in columns of their own (FREQUENCIES Value and Label, DESCRIPTIVES Variable and Label, the Between-Subjects Factors of UNIANOVA and GLM) keep both columns with LABELS and BOTH, so the value is never lost, and leave out the label column with NAMES or VALUES.
- SPLIT FILE LAYERED shows the groups as tables do, SEPARATE as titles do.
- CTABLES /VLABELS and MULT RESPONSE /FORMAT=NOLABELS win over the settings. CTABLES KEY=LABEL sorts by the value labels whatever the table shows.
- A nested GLM effect such as a(b) shows as DESIGN writes it, and parameter names such as [a=1] keep names and values.
- These keep names and labels as they are: warnings and errors, the dictionary and data listings (DISPLAY, SYSFILE INFO, LIST, PRINT), REPORT (which has label options of its own), the transformations (RANK, AUTORECODE, AGGREGATE) and the labels given to new variables.

### SPLIT FILE

- LAYERED merges the tables of the groups into one, the split variables as the outermost row labels. A column that only some groups have is empty in the others.
- SEPARATE names the group in each title.

### Output documents and OUTPUT EXPORT

- **Beyond the documentation:** OUTPUT EXPORT takes /CSV DOCUMENTFILE='file' [NOTESCAPTIONS=YES | NO], a Statscraft addition: each table as comma-separated values after its title, a blank line between tables.
- Documents are named Document1, Document2 and so on.
- The window shows the tables of every document in its one Output tab. The documents decide what OUTPUT EXPORT writes, including tables deleted from the view.
- The window session keeps its documents from run to run; a command-line run starts without any.
- OUTPUT ACTIVATE of a name no document has, and OUTPUT CLOSE of one, give a warning, so the run goes on.
- OUTPUT EXPORT /XLSX puts the tables one below the other on one sheet (SHEET, or Tables), a blank row between. MODIFYSHEET leaves one blank column (LASTCOLUMN) or row (LASTROW) before the new tables.
- CREATESHEET and MODIFYSHEET add only to a workbook that OUTPUT EXPORT wrote in the same session and that has not changed since, because the whole workbook is written again and what Statscraft did not write (formulas, charts, formatting) would be lost. Other workbooks are refused with an error.
- ENCODING=LOCALE writes UTF-8.
- In TEXT, the columns are as wide as their widest cell (COLUMNWIDTH=n wraps at n characters) and numbers align right.

### PRINT and WRITE

- Columns after a string variable give it an A format (GNU PSPP takes F and refuses). An asterisk may follow a format list. WRITE FORMATS keeps a write format apart from the print format, also in saved data files.
- PRINT, PRINT EJECT, PRINT SPACE, WRITE and XSAVE run when the data are next read, and also in the pass at the end of a run. Their output shows with the command that read the data.
- On the screen, each PRINT EJECT starts a new table.
- PRINT to a file starts each line with a blank, or 1 after PRINT EJECT, and puts the period of a system-missing value where the decimal point would be, as GNU PSPP does.
- A file is written anew at every data pass. Without ENCODING, a file is UTF-8 without a byte order mark.
- PRINT SPACE with a negative or missing count prints one line with a warning, as GNU PSPP does.
- WRITE needs the full width for a string variable, as its description says.

### TITLE, SUBTITLE and ECHO

- They show their text as a one-cell table where they stand. A blank TITLE or SUBTITLE shows nothing.

### REPORT

- **Beyond the documentation:** a slash before the first subcommand is accepted.
- The weight counts as in the other procedures (VALIDN is the sum of the weights).
- MEDIAN, MODE, FREQUENCY and PERCENT cut each value to a whole number and count those in the range. The median is the middle value, the mean of the two middle values when the weights split evenly between them. The mode is the most frequent value, the lowest of a tie. PERCENT is per cent of all valid values.
- FREQUENCY and PERCENT give one line per value of the range, titled with the value, then a line with the total under the summary title.
- A function without valid values is missing.
- TOTAL repeats the first SUMMARY of the first BREAK for all cases at the end, under a line Grand Total.
- A report without break columns puts its summary titles in a column of their own.
- The report is one page: LENGTH, PAGE and PREVIEW change nothing, and )PAGE is the PAGE1 number.
- The shown table leaves out the blank lines. OUTFILE writes the text with them and shows no table. The first REPORT of a session that names a file starts it anew; later ones append.
- A numeric part of STRING that does not fit its width in the variable's format is shown without decimals.

## MATRIX

Where GNU PSPP differs:

- GET with MISSING=OMIT or a value also treats system-missing values, as documented (GNU PSPP stops).
- EOF is 1 after a READ that took the last record of the file (GNU PSPP waits for a READ past it).
- CDF.NEGBIN and PDF.NEGBIN count the trials up to the r-th success, from r (GNU PSPP counts the failures).
- The bivariate normal CDF integrates its density (GNU PSPP 2.0.1 gives values above 1), and PDF.LAPLACE is the density of CDF.LAPLACE (GNU PSPP's is twice that).
- Unquoted RLABELS and CLABELS are in upper case.
- T, the function names and EIGEN, SETDIAG and SVD cannot name a matrix.
- WRITE starts in the column that FIELD names.
- CALL SVD gives Q the dimensions of the matrix and U and V square, as documented (GNU PSPP gives the thin decomposition).
- The half-normal, inverse Gaussian, studentized range and maximum modulus distributions and the noncentral functions are supported.
- COMPUTE of a scalar into a part of a matrix fills that part (GNU PSPP refuses).
- RND rounds a half away from zero, as RND does in COMPUTE (GNU PSPP rounds it to even).
- MAGIC gives the Siamese square for odd sizes, the complement square for multiples of 4 and the LUX square for other even sizes (GNU PSPP gives other squares for even sizes).
- GET /NAMES gives a row vector, one name per column (GNU PSPP gives a column).

Beyond the documentation:

- A program may hold any number of SAVE statements (the manual allows 100).
- LOOP takes a comma in place of TO, as the manual's BREAK example writes it.
- `&`, `|` and `~` stand for AND, OR and NOT.

Where the documentation is silent:

- PRINT gives a table with row labels (row1, row2, ... unless RLABELS or RNAMES give them) and column labels (col1, col2, ...), as the manual describes for tables. SPACE does not change a table.
- Without FORMAT, whole numbers below 10^9 show without decimals and other numbers with ten significant digits in one format. When the largest value is 10^9 or more, or all are below 10^-3, the values are divided by a power of 10 that the caption names.
- Infinities and undefined values show as +Inf, -Inf and NaN. Division by zero follows floating point, while LN, LG10, SQRT, ARSIN, PROBIT, INV and the like check their arguments.
- An error stops the program at its statement, with its line and column, and nothing is saved or written. SAVE, MSAVE and WRITE write their files at END MATRIX.
- DESIGN orders the columns of a variable by value.
- Eigenvectors and singular vectors have length 1 and their largest element positive.
- GET of the active dataset reads every case, also those FILTER leaves out.
- A LOOP index keeps the value of its last iteration. A loop that MXLOOPS stops gives a warning. BY 0 is an error.
- UNIFORM and NORMAL with two scalars give a matrix of draws, with one argument a draw per element. They and the RV functions draw from the generator SET RNG chooses, in the order COMPUTE draws case by case.
- A distribution or random function with a parameter out of range is an error.
- READ takes a blank fixed field as missing (NaN) and fills what the file does not reach with missing values, with a warning.
- RELEASE of a name that does not exist gives a warning.

## Not supported yet

- **Commands.** The commands of the language that Statscraft recognizes but does not run are listed at the end of [the command index](commands/README.md#not-supported-yet). That list is generated from the program, so it is always current. A script that uses one stops at that line, and nothing is changed.
- **Subcommands.** Every command page lists what it accepts. Notably:
  - FREQUENCIES draws no charts (BARCHART, PIECHART and HISTOGRAM are accepted with a warning).
  - ONEWAY has no POLYNOMIAL, RANGES, EFFECTS or WALLER.
  - NONPAR CORR has no MATRIX or CI. PARTIAL CORR has no MATRIX.
  - NPAR TESTS has no exact significance for K-S, the median, Kruskal-Wallis, Jonckheere-Terpstra, Friedman, Kendall, Cochran and marginal homogeneity tests.
  - EXAMINE computes the normality tests but draws no plots (STEMLEAF, BOXPLOT and HISTOGRAM are accepted with a warning, SPREADLEVEL is refused).
  - REGRESSION has no METHOD=TEST, SELECT, MATRIX or OUTFILE and draws no plots (HISTOGRAM, NORMPROB, SCATTERPLOT, PARTIALPLOT and TEMPLATE are accepted with a warning).
  - RELIABILITY has no MODEL=OMEGA or MATRIX.
  - LOGISTIC REGRESSION has no SELECT or OUTFILE and draws no CLASSPLOT (accepted with a warning).
  - UNIANOVA and GLM have no multivariate model (GLM with several dependents without WSFACTOR), RANDOM, TEST, ROBUST, OUTFILE, MMATRIX, MEASURE, POSTHOC VS, CONTRAST SPECIAL or the heteroskedasticity tests, and draw no plots (PLOT is accepted with a warning). With WSFACTOR they have no EMMEANS, POSTHOC, CONTRAST, LMATRIX or SAVE.
  - FACTOR has no METHOD=COVARIANCE, MATRIX=OUT or MATRIX=IN(FAC=...) and draws no plots (PLOT is accepted with a warning).
  - CLUSTER has no MATRIX=OUT and draws no icicle plot or dendrogram (PLOT is accepted with a warning).
  - DISCRIMINANT has no ROTATE=COEFF or STRUCTURE, MATRIX or OUTFILE. PLOT (charts, the territorial map and the casewise listing) is accepted with a warning.
  - USE takes case numbers only with /PERMANENT.
  - ADD FILES, MATCH FILES and UPDATE take no PASSWORD.
  - GET DATA reads no ODBC or OLEDB sources.
  - SAVE TRANSLATE writes CSV, TAB and Excel 2007 workbooks only (no SAS, Stata, dBASE, 1-2-3, SYLK, ODBC or the older binary Excel files), has no APPEND or MAP, and RENAME takes valid variable names only.
  - FILE HANDLE refuses MODE=BINARY, MULTIPUNCH, IMAGE and 360, since only text and data files are read.
- **Expression functions.** APPLYMODEL and STRAPPLYMODEL, which need MODEL HANDLE. NUMBER reads the formats DATA LIST reads (F, N, E, COMMA, DOT, DOLLAR, PCT, DATE, ADATE, EDATE and SDATE). STRING writes the number, date and time formats, but not the custom currencies CCA to CCE.
- **Transformations.** PRINT and WRITE have no binary formats (P, PK, IB, PIB, PIBHEX, RB, RBHEX, Z). DATA LIST reads only F, N, E, COMMA, DOT, DOLLAR, PCT, A, DATE, ADATE, EDATE and SDATE, and only UTF-8 files. Commands such as VALUE LABELS cannot stand inside LOOP. AUTORECODE has no templates.
- **Charts.** No command draws charts yet. TREE shows its tree diagram as a table, one row per node indented by depth, and says so in a warning.
- **TREE.** CHAID and exhaustive CHAID with a nominal or ordinal dependent. Not supported yet: CRT, QUEST, a scale dependent, split-sample and cross-validation, SPLITMERGED=YES, custom costs, profits, percentile gain tables, rules, influence variables, model files and TARGETRESPONSE.
- **Data files.** Statscraft reads and writes .sav and .zsav files, and portable (.por) files with IMPORT and EXPORT.
  - Text files are read with DATA LIST, GET DATA and GET TRANSLATE /TYPE=TAB; workbooks (.xlsx, .xlsm, .xls) with GET DATA and GET TRANSLATE; dBASE III and IV files with GET TRANSLATE. 1-2-3, Symphony, SYLK and dBASE II files are not read.
  - SAVE TRANSLATE writes CSV, tab-delimited text and .xlsx.
  - An old .sav file that does not state its code page is read as Western European (windows-1252). There is no way to choose another encoding yet.
- **Output.** Tables come out as Markdown, JSON, HTML (from the window and OUTPUT EXPORT), text and CSV (OUTPUT EXPORT) and Excel workbooks.
  - Statscraft has no output document file of its own, so OUTPUT SAVE is refused with an error that points to OUTPUT EXPORT. OUTPUT OPEN and OUTPUT MODIFY are not supported.
  - OUTPUT EXPORT writes no Word, PDF, PowerPoint or Excel 97-2004 files, and no images, since no command draws charts yet.
- **Packages.** The Windows installer and the macOS app are not signed yet. There are packages for x86_64 Windows and Linux and for macOS on Apple silicon and Intel, but none for Windows or Linux on ARM processors.
