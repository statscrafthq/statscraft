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
- An expression nests at most 64 levels of parentheses and functions and holds at most 5000 operators; deeper or longer is an error, not a stack overflow.
- **$CASENUM** counts every case read in the data pass, also those a later SELECT IF drops (GNU PSPP differs).

### Expression functions

- In CDF, PDF, NCDF, NPDF and SIG, a value of x that the distribution does not take gives system-missing (below 0 for CHISQ, outside min to max for UNIFORM, not a whole number in range for the discrete ones), as the manual says. GNU PSPP gives 1 for CDF.BINOM(-1, ...), 0 for CDF.LNORMAL of a negative value, and reads CDF.BINOM of a fraction.
- TIME.HMS takes one to three arguments. Minutes or seconds of 60 or more under a larger unit that is not zero, or a fraction before the last argument that is not zero, give system-missing.
- A .n suffix larger than the number of arguments, as in MEAN.4(a, b), gives system-missing.
- A noncentral distribution with a noncentrality past 2^53 gives the limit of the probability (NCDF.T, NCDF.F) or system-missing (the others). An F, t or beta probability whose parameters are too large for its series to converge (past about 1e10) is system-missing.
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
- A warning that every SPLIT FILE group gives (SPREADLEVEL without a factor in EXAMINE, for one) is shown once, without a group name.
- Under SPLIT FILE, the variables a procedure saves are those any group saved, defined as in the first group that saved them (a string as wide as its widest group); the cases of a group that could not be fitted or saved nothing get missing values.
- END DATA, END REPEAT and END MATRIX standing alone are errors at their line, saying which command they belong to.
- FINISH ends the run as the end of the syntax does: nothing after it is read. A window or command-line session stays open, with its dataset. With a line range only a FINISH inside the range counts. Macros that the lines after FINISH define are still defined, since macros are expanded before the commands are read.
- HOST is read and checked, then refused: a syntax file that runs operating system commands could do anything the user can.
- Runs from the window's syntax windows and from the command line share one queue: a run started while another runs waits, and the runs go in the order they were started. Each starts from the state the runs before it left. A failed run changes nothing and does not stop the runs after it.

### Macros

- A body that ends with a period ends the calling command only when the call is the last thing in it. In the middle of a command (a title macro inside CTABLES) that period is dropped.
- Macro expressions compare as numbers when both sides are numbers, and else as text with case.
- A macro variable that was never set stands for its own name.
- Expansion lines carry the line number of the call, and errors in them name the macro.
- A session (a command-line session or the window) keeps the macros and the macro settings of SET from run to run. A failed run or a check keeps none, and a run of a line range keeps only the macros defined on its lines, though one defined elsewhere in the text still serves it.
- Whatever SET MNEST says, macro calls nest at most 100 levels. One expansion makes at most 100 000 calls and 1 MiB of text, and !BLANKS gives at most 1 MiB, so a macro that calls or doubles itself ends in an error.

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
  - An XLSX sheet whose cells lie so far apart that a grid from the first to the last would hold more than 20 million cells (and more than ten times the cells it has) is refused; CELLRANGE reads a part of it.

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
- MAP works for every type. For CSV, TAB and workbooks it lists the names as RENAME, KEEP and DROP leave them.
- **TYPE=STATA, beyond the documentation:** VERSION may be left out and then means 14 (format 118), and a file name ending in .dta needs no TYPE. VERSION above 14 also writes format 118, which Stata 14 to 19 read. Stata 4 to 7 files are not written.
- A date with a time of day (DATETIME, YMDHMS) becomes a %tc value in milliseconds since 1960, so the time is kept; the manual's table gives it the date format. Format 113 (VERSION 8 and 9) has no %tc, so there it is written as days with their fraction under a date format.
- Numbers are stored as byte, int or long when every value is whole and fits, else as double, so no value changes. Missing values become Stata's `.`.
- A name that is not ASCII becomes v and its position before Stata 14. A Stata reserved word (byte, int, if and the like, str with digits) gets _ at the end; names made equal by cutting get _2, _3.
- The default encoding before Stata 14 is the system's code page; text it cannot hold is an error that suggests VERSION=14, which is always UTF-8.

### GET STATA

- Formats 113 to 119 are read (Stata 8 to 19); older files are refused.
- A %tc or %tC value (a date with a time of day) becomes DATETIME20 (DATETIME23.3 when some value has a fraction of a second), not DATE, so the time survives. Leap seconds of %tC are not counted.
- A %g display format becomes F of the same width, with 0 decimals when every value is whole, else 2; %f keeps its decimals and %fc becomes COMMA.
- Names keep their case. A name that starts with _ (such as _merge) gets V in front and one of our reserved words gets $ at the end; a name equal to an earlier one but for case gets _A, _B and so on, and the first keeps its name.
- Without ENCODING, text in files older than Stata 14 is read as UTF-8 when it is valid UTF-8, else in the system's code page.
- Value labels keep 255 bytes (the manual cuts them at 120). The file label is cut to 64 bytes, the most a .sav file holds, with a warning.
- A long string marked binary is read as text when it holds no zero byte; one with a zero byte is blank.

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
- With NODIAGONAL and one variable a matrix record has no cells to hold and is an error.

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
- A data file opened from the window gets the name DataSetn, as the manual describes, written into the log as a DATASET NAME after the command that opened it. n comes after every DataSetn name the session has given, by the window or in syntax, so a closed dataset's name is not given to another. A file that fails to open uses up no name.
- A file opened by syntax without DATASET NAME replaces an unnamed active dataset, as the manual says; the window's open names every file, so it replaces nothing.
- DATASET CLOSE of a name no dataset has is an error.
- DATASET DISPLAY lists each dataset with its cases, variables and whether it is active or only declared. It shows no creation time and no file.
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
- A step too small to change the index at its size (1e17 BY 1) ends the loop with a warning. LOOPs nest at most 100 levels.

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
- A start, period or BY past 1e9 is an error.

### Dictionary commands

- DOCUMENT and ADD DOCUMENT add `(Entered YYYY-MM-DD)` after their lines.
- ALTER TYPE from string to number reads F, COMMA and DOT values. A type change drops value labels and missing values.
- **Beyond the documentation:** DISPLAY accepts `FILE LABEL`.
- An attribute array index lies from 1 to 65535.

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
- NTILES takes 1 to 1000.
- Charts come as data and as pictures: the window draws them, as `statscraft run --charts` does ([the window](window.md#charts)). Bars and slices follow the order of the frequency table. Bar percentages are of the valid cases; slice percentages are of the whole pie, missing slice included. When FREQ and PERCENT, or NORMAL and NONORMAL, are both given, the first counts, as the manual says for the pie. An axis top in FREQ(n) or PERCENT(n) below the highest bar is ignored and left to the drawing.
- Histogram bins: Sturges' count (1 plus log2 of the weighted N, rounded up) sets a first width, moved to the nearest of 1, 2 or 5 times a power of ten, at least 1 when all values are whole. Bins start at a multiple of the width and hold their lower bound. A string variable gets no histogram, with a warning.
- Grouped percentiles: with a width or boundaries, a percentile is interpolated within the first group whose cumulative weight reaches it. A value on a boundary belongs to the group above, except the last boundary. Values outside the boundaries form open groups, where percentiles are undefined.

### DESCRIPTIVES

- "Valid N (listwise)" always counts the cases valid on every listed variable (GNU PSPP differs).
- String variables are skipped with a warning.
- Saved z-scores are named Z plus the variable name, then ZSC001 to ZSC099, STDZ01 to STDZ09, ZZZZ01 to ZZZZ09 and ZQZQ01 to ZQZQ09.

### CROSSTABS

- Expected counts in the margins are counts; FORMAT=DVALUE reverses the rows only; `/CELLS` and `/STATISTICS` without a list are accepted (GNU PSPP differs).
- Fisher's exact test shows for every 2 x 2 table.
- BARCHART: the manuals word the roles of the two variables differently; Statscraft follows the dialog's reading, a cluster per row category and a bar per column category. Layers are summed over.
- /COUNT with a keyword left out takes CELL or ROUND, as the dialog writes `/COUNT ROUND CELL`. Rounding takes halves away from zero.

### MEANS

- MEANS shows one report per dependent variable. Its ANOVA uses the first control variable only.

### SUMMARIZE

- With CELLS left out the cells are MEAN, COUNT and STDDEV, as the description of CELLS says; the syntax chart marks only COUNT as the default.
- LIMIT=n restricts the whole command, statistics included, to the first n cases (of each split group, after the filter); a caption says so. **Beyond the documentation:** LIMIT also follows VALIDLIST.
- VALIDLIST changes only which cases are listed; the statistics follow /MISSING.
- MISSING=TABLE drops a case missing on any dependent or on a control of that table, also from the analysis of variance.
- Strings given one after another in TITLE or FOOTNOTE are joined without a space.
- A variable named twice among the dependents or among the controls of one BY counts once.

### CODEBOOK

- Counts, percents and statistics are weighted; a case with a missing, zero or negative weight is left out of them.
- A numeric variable of unknown level gets the level its values suggest (as data read from text do) for its statistics; its table still says Unknown.
- Quartiles use (W + 1) p, as FREQUENCIES does.
- A nominal or ordinal variable lists its labeled values and the values found; its missing values are its discrete missing values, labeled or found values in a missing range, and System when cases are system-missing. Without COUNT or PERCENT only labeled values are listed.
- Within a VARORDER group file order holds; ALPHA sorts a set's name without its "$" and ATTRIBUTE compares values without regard to case.
- NAME and LOCATION come from the GET FILE or SAVE that last read or wrote the active dataset; a session continued in a later `statscraft run` no longer knows them.
- CODEBOOK always reads the data, also with STATISTICS NONE, so it ends TEMPORARY.

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
- Plots come as chart data. Boxplot whiskers stop at the furthest values that are neither outliers nor extremes, never inside the box; a value equal to a hinge is never marked, which matters when the box has no length. Weighted cases are marked once each.
- Stem-and-leaf: the cases past the boxplot's outlier bounds are listed as extremes. There are about 10 log10(n) lines, each 1, 2 or 5 times a power of ten wide (one, five or two lines per stem). A leaf is the next digit, cut rather than rounded, in ascending order on every line, negative lines included. A leaf stands for one case, or for 2, 5, 10 and so on when a line would hold more than 50 leaves; weighted cases give their weight in leaves, rounded.
- Normal Q-Q plots need two cases with different values; otherwise a warning says so. With weights, the rank of a value is the mean rank of its weight.
- Spread versus level without a power also shows 1 minus the slope as the power to try. The Test of Homogeneity of Variance shows Levene's tests based on the mean, the median (also with adjusted df) and the trimmed mean.

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
- A CHISQUARE range holds at most 100 000 whole numbers.

### NPTESTS

Where the documentation is silent:

- A field of unknown measurement level gets one from its values, the same rule a data pass uses. String fields are nominal, so only the categorical tests take them.
- Without test keywords, related fields of each level are tested apart: two or more continuous fields get Wilcoxon or Friedman, and the nominal fields and the ordinal fields get McNemar or Cochran when their values together are two. Ordinal fields count as categorical in that choice, as in the one-sample choice.
- FIRST and SAMPLE take the first value met in the data, case by case and field by field, and need exactly two values among the test's fields together.
- The two-group tests take the lower group value as the first group. The independent-samples tests skip nominal test fields with a warning, and so do the other tests that need a continuous or ordinal field.
- The Monte Carlo significance of K-S restarts its generator at SEED for every test, so one test does not change another's draws. It draws from the fitted family with the parameters estimated again in every sample; uniform and exponential fields with parameters from the data always use it, the normal unless SIMULATION=FALSE. Past 200 million draws in one test it gives the Lilliefors table (normal) or Smirnov's significance, with a warning.
- Moses decides on the trimmed span when at least one record is trimmed, at p <= ALPHA.
- Jonckheere-Terpstra decides on the one-sided significance in the direction ORDER gives; the detail table also shows the two-sided one. Its homogeneous subsets show the mean joint ranks.
- Wald-Wolfowitz decides on the most runs ties allow. Below 50 cases its standardized statistic moves half a run up, toward the side the one-sided test looks at.
- The related Hodges-Lehmann limits take the signed-rank mean and standard deviation of all differences, zeros included, without a tie correction; the independent ones take the Mann-Whitney values with the tie correction, as documented. Either estimate builds at most 5 million pairwise differences or averages of distinct values; past that it gives a warning instead.
- The pairwise median test of two samples uses their own median, Fisher's test up to 30 cases, and shows the chi-square only when it gives the significance. Multiple comparisons need a rejected test of three or more samples.
- Stepwise subsets test each candidate subset with the omnibus test at ALPHA and show its statistic and significance; there is no adjusted significance for subsets.
- The hypothesis summary marks each significance as exact, asymptotic, Monte Carlo or Lilliefors. Charts are not drawn; the detail tables carry their numbers.
- A SPLIT FILE variable may be named as a test field; it is constant within each group, so its tests come out empty.

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
- Charts come as data and cover the cases of the analysis. The normal P-P plot puts the cumulative share of the weight up to each distinct value against the normal probability of that value. Partial regression plots use the weights of the analysis and keep the constant (none under ORIGIN). The Residuals Statistics table shows whenever a chart is asked for. TEMPLATE is ignored with a warning.

### LOGISTIC REGRESSION

- METHOD takes FSTEP and BSTEP with COND, LR and WALD. Interactions, every contrast type, CASEWISE and SAVE are supported (GNU PSPP differs).
- The terms keep the order of the VARIABLES list (GNU PSPP puts the covariates that are not categorical first).
- CLASSPLOT counts the cases in 40 bins of 0.025 of the predicted probability, each bin holding its lower bound, with one plot per step shown.
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

### COXREG

- GNU PSPP has no COXREG; the numbers are checked against R's `survival` package.
- **Baseline.** The baseline survival is the maximum likelihood estimate given the coefficients (in R: `survfit` with `stype=1`), as documented, not the Breslow estimate `exp(-H)` that R gives by default; the cumulative hazard is -ln of that survival, except that where every case at risk fails (survival 0) it adds the Breslow step, so it stays finite. The Survival Table's baseline cumulative hazard is at all covariates 0, its other columns at the covariate means (R's `basehaz` gives the Breslow hazard at the means). The standard error of the survival is the survival times the standard error of the Breslow cumulative hazard at the same covariates.
- **Overall (score) test** is the score test at all coefficients 0 of the model's columns, also in later blocks.
- **Redundant covariates.** A coded covariate is left out when it adds no variation within the risk sets at b = 0 beyond the ones before it: a linear combination of them, or constant within every stratum or risk set.
- **Dropped cases.** A censored case before the first event of its stratum is dropped and counted on its own; it does not enter the covariate means. A case missing on a covariate that is not in the model is still dropped when the covariate is named on the command.
- **Weights** are case weights in the likelihood (a case whose weight is not positive counts under missing values in the Case Processing Summary); the covariance is the inverse information (R gives a robust covariance with fractional weights).
- **Convergence.** BCON compares each coefficient's full Newton step with its previous value (divided by at least 1e-3 when that is near 0), so a halved step does not look converged for being halved; LCON compares the change of the log likelihood with its previous value, after a full step only. A step that lowers the likelihood is halved. When ITERATE runs out the results are still shown, with a warning, rather than only the iteration history. The default criteria stop slightly before R does; give CRITERIA BCON(1E-12) LCON(0) for full convergence.
- **Saved variables.** DFBETA is the one-step approximation: the case's score residual times the covariance, the change b minus b without the case (R's `dfbeta`), times the case weight. PRESID is the Schoenfeld residual of an event, one variable per coded covariate (missing for covariates out of the model and for censored cases). DFBETA and PRESID give one variable per coded covariate, named DFB1_1, DFB2_1 and PRE1_1, PRE2_1, ... in their order, missing for covariates out of the final model, so split groups whose models keep different covariates share the names (as long as their categorical covariates have the same number of categories).
- **Time-dependent covariates.** A case the program gives a missing value while it is missing on a variable the program names (read or assigned) counts under missing values; otherwise a missing value from the program is an error. LAG, $CASENUM and values carried from case to case are not supported yet in a program. Every covariate the TIME PROGRAM creates or changes counts as time dependent; a program that changes none is not used, with a warning. CLEAR TRANSFORMATIONS and any command other than COXREG that reads the data drop a waiting program; a COXREG that stops with an error keeps it for the corrected command, unless the fault is in the program. The TIME PROGRAM runs for every case at each event time of its stratum up to its own time (in R: the counting-process form, or `tt()`). With them SAVE gives DFBETA only, and the Survival Table, PATTERN and the plots are left out with a warning.
- **Tables.** Variables not in the Equation closes with a Residual Chi Square row. Model if Term Removed shows the loss chi-square for COND and LR. Covariate Means shows with BASELINE or PLOT, with a column per PATTERN. PRINT SUMMARY shows the Omnibus Tests line of every step and the other tables for the last step of each block. A chi-square with 0 df is left empty.
- **Plots** are chart data of step curves, one line per stratum (and per category of a PATTERN BY variable), starting at time 0; LML starts at the first event.
- PATTERN sets an interaction column from its variables when all are named, else to the column mean.

### NOMREG

- The manual is followed where the usual R function for this model (`nnet::multinom`) differs: the reference category is the last one unless BASE says otherwise (`multinom` takes the first); a factor gets one parameter per level, the last redundant and shown as 0 (`multinom` drops the first level); the standard errors come from the information matrix, which here is the same observed or expected. The tests convert by releveling and by contrasts with the last level as base.
- The -2 log likelihoods include the multinomial constant of the subpopulations, so they are smaller than `multinom`'s deviance by twice that constant; KERNEL shows the deviance. Chi-squares, Cox and Snell and Nagelkerke R square do not depend on it; McFadden's R square uses the kernel, as the manual's formula does.
- PRINT given shows only the tables it names; without PRINT the Case Processing Summary and Model Fitting Information, as the manual marks the defaults.
- The likelihood ratio tests leave the intercept out, as the Algorithms manual says.
- DELTA and BIAS change the counts that are fitted and tested; the Case Processing Summary and the Classification table show the case weights as they are.
- SAVE gives values to every case whose factors and covariates are valid and whose factor levels occur among the cases used, also to cases left out for a missing dependent value or a weight of 0; ACPROB needs a dependent value that is one of the categories. ESTPROB with a root name gives root1, root2, ...; without one EST1_1, EST2_1, ....
- Convergence: the change of each parameter is absolute (PCONVERGE), and the gradient test uses the smaller of the criteria above 0. Step-halving accepts a step when the log likelihood does not fall.
- A string BASE value for a numeric dependent is read as a number, or matched against the formatted value.
- Stepwise methods: an effect whose parameters are all redundant given the rest of the model has a test on 0 df with p = 1, so it is the first to leave. The Wald test of an effect with very large estimates (near separation) depends on where the fit stopped, as for any Wald test. SCALE does not change the stepwise tests. The step tests were checked against R: nnet's multinom fits of each model on the path for the likelihood ratios, and the score and Wald statistics written out from the gradient and information of the multinomial log likelihood.
- Under RULE(SINGLE), SFACTOR and CONTAINMENT only effects on MODEL count: an interaction listed without its main effects may enter on its own.
- Not supported yet: TEST, OUTFILE, PRINT CELLPROB, HISTORY and ASSOCIATION.

### PLUM

- Location parameters are subtracted from the thresholds, so a positive estimate moves cases toward the higher categories, and the last level of a factor is the redundant one. R's MASS::polr uses the same signs but drops the first level of a factor by default; compare with the reference level set to the last.
- Without a PRINT subcommand the tables are the Case Processing Summary, Model Fitting Information, Goodness-of-Fit, Pseudo R-Square and Parameter Estimates. With PRINT, the first two and the named ones.
- Standard errors come from the observed information at the estimates (the Newton-Raphson form the manual allows); a step that the observed information does not send uphill uses the expected information instead.
- The -2 log likelihood includes the multinomial constant of the pooled cells, as the manual's KERNEL keyword implies; the chi-squares do not depend on it. The pseudo R-squares use the kernel, as the manual's formulas do.
- The test of parallel lines is a likelihood ratio test, as the Algorithms manual describes, on (J - 2) times the number of location parameters. It is left out with SCALE, with a warning.
- The Missing row of the Case Processing Summary counts the weighted cases with a positive rounded weight and a missing value.
- SAVE writes predictions for every case with valid factors and covariates, also one outside the fit through its weight or a missing dependent; ACPROB needs the dependent. ESTPROB(root) names the variables root1, root2 and so on. PREDCAT of a string dependent is a string.
- TEST: the manual asks both for one value after INTERCEPT and for one per threshold; INTERCEPT here takes one per threshold, since the thresholds are the intercepts. An effect in both LOCATION and SCALE cannot be named on TEST (an error says to use ALL). ALL and effect names never mix in one TEST: the first of them sets the form of every row. A coefficient on a redundant parameter multiplies its fixed 0; there is no estimability check. A row with no coefficient on an estimated parameter is left out with a warning. The tables are Contrast Estimates (each row's estimate, test value, standard error, Wald chi-square on 1 df, significance and interval) and Test Results (the joint Wald chi-square). The numbers were checked against R: polr's estimates with the covariance from a numerical Hessian of the log-likelihood (polr's own Hessian is on its internal threshold scale).

### GENLIN

- GNU PSPP has no GENLIN. The numbers were compared with R (`glm`, `MASS::glm.nb`, `MASS::polr`) run as a separate program on synthetic data, converting where the documented definitions differ from R's:
  - A binary dependent models its first value (ascending) unless REFERENCE says otherwise, so with 0 and 1 the signs are those of R's model of 0; REFERENCE=FIRST gives R's signs.
  - The last level of a factor is the redundant one, shown as 0; R drops the first unless told otherwise.
  - The scale of the normal, gamma and inverse Gaussian models is the maximum likelihood estimate (for the normal, the residual sum of squares over n), not R's Pearson-based dispersion; SCALE=DEVIANCE or PEARSON gives the moment estimates.
  - Standard errors come from the observed information, as Newton-Raphson after the first Fisher scoring iteration ends; R reports the expected information. They agree for the canonical links. METHOD=FISHER (scoring throughout, or FISHER(n) with n at least MAXITERATIONS) gives the expected information. With the observed information not positive definite, the expected one is used.
  - With NEGBIN(MLE) the covariance of the coefficients is the corresponding block of the inverse of the joint information of the coefficients and ln k; R's `glm.nb` holds k fixed.
  - The deviance and Pearson chi-square of a binary dependent are summed over subpopulations (cases with the same factor levels, covariate values and offset), on their number minus the parameters; the full log likelihood adds the binomial coefficients of the subpopulations. Events/trials data then give the same statistics, and N in BIC and AICC is the number of trials.
  - The full log likelihood of the ordinal model adds the multinomial constant of its subpopulations; LIKELIHOOD=KERNEL leaves it out.
  - AIC and the other criteria count the scale and k when estimated.
- With NEGBIN(MLE), k counts against the degrees of freedom of the goodness-of-fit statistics. An estimate of k below 1E-6 gives a warning: the counts are not overdispersed, and the standard error and interval of k are not reliable (a bound that is not finite is left blank).
- Events/trials without DISTRIBUTION and LINK is a binomial model with the logit link. Read literally, the documentation would give it the identity link, which leaves most data with probabilities outside 0 to 1; the canonical link is what a careful analyst expects.
- The reduced and constrained models of the likelihood ratio tests stop when the log likelihood changes less than LCONVERGE (1E-4 absolute when it is 0), as documented; the full model uses all the criteria. FISHER(n) that converges during its scoring iterations goes on with Newton-Raphson steps until those converge too.
- Estimated marginal means leave the offset out (the means of the linear predictor without it), as the documented formula does; with OFFSET=number that is the intercept's shift.
- ORDER=DATA is accepted with SPLIT FILE; each group then orders the levels as they come in that group.
- A POLYNOMIAL metric with fewer values than the factor has levels gives a warning and leaves out the comparisons of that EMMEANS; the means are still shown.
- The omnibus test of the ordinal model compares with the fitted thresholds-only model; with an offset this is not the start values the documentation names.
- Type I tests start from the model with no parameters, so the intercept has a test. Under a link that gives no valid means at a linear predictor of 0 (the inverse link of the gamma, a power link) that model cannot be fitted, and its likelihood ratio test is left blank with a note. Type I Wald tests use the rows of the forward Doolittle reduction of X'WX (W the frequency times the scale weight times the trials); Type III Wald tests use the hypotheses of UNIANOVA. Likelihood ratio tests refit the model, Type III under its hypothesis.
- The scale estimate of DEVIANCE and PEARSON divides by N minus the parameters (minus one more for NEGBIN(MLE)); the F tests use the same degrees of freedom.
- Only complete separation is checked (CHECKSEP), and it gives a warning; quasi-complete separation is not detected.
- Confidence intervals of estimated marginal means on the response scale are symmetric around the mean (delta method), as documented. The covariates of EMMEANS default to their means weighted by frequency and scale weight.
- REPEATED (generalized estimating equations) was checked against our own matrix implementation of the documented estimating equations in R; with the working correlation fixed at its final value, that implementation and the `geepack` package agree to 1E-13 in the estimates and the robust standard errors. Choices and differences:
  - The working correlation estimates follow the documented formulas: each mean product of residual pairs is divided by the Pearson scale moment, also when the scale of the model is fixed (so a correlation does not depend on the scale), and with ADJUSTCORR=YES every count in a denominator (pairs and measurements) loses the number of parameters. `geepack` uses other estimators, so its correlations and estimates differ slightly.
  - The documentation does not say how large the ridge is that makes an estimated matrix positive definite. Here it leaves 0.01 as the smallest eigenvalue, and a warning says the final matrix was moved. An AR(1) estimate above 0.999 in size is set to 0.999 (with the sign kept), which keeps the AR(1) form, with a warning that says so. A matrix whose smallest eigenvalue is 1E-6 or less counts as not positive definite. The Number of Subjects of the Correlated Data Summary counts the frequency weights. A step that gives a case an invalid mean is halved until it does not (CRITERIA MAXSTEPHALVING belongs to the plain model); a halved step does not count toward PCONVERGE, and when no step works the iterations stop with a warning.
  - With SCALE=MLE the scale of the plain model is passed on and replaced by the Pearson moment after every iteration, so MAXITERATIONS=0 keeps the plain model's estimate.
  - QIC takes H of the independent structure at its solution (the estimates of the plain model) and at the same scale as the quasi-likelihood: a scale fixed on CRITERIA, or else 1. Both terms then rest on the one quasi-likelihood; for a normal response the penalty is about twice the parameters times the variance, as in Mallows' Cp. `geepack` divides H by an estimated scale even when the scale is fixed, so its trace term differs; the quasi-likelihood and QICC (its QICu) agree.
  - The measurements of a subject are its cases used in the fit. With WITHINSUBJECT the positions are the values found on every case with valid subject and within values, so a week with no response anywhere is still a position. Without WITHINSUBJECT a case left out does not hold a position. With SORT=NO the runs are taken in the whole file, so a case left out still ends a run, unless its SUBJECT value is missing or its frequency weight is below 1. A missing WITHINSUBJECT value leaves the case out also with SORT=NO. The matrix may have at most 200 positions. A case with a frequency weight below 1 adds no position. Model Information shows the m of MDEPENDENT as used, after the cut to the dimension less 1.
  - As documented, the response, events, trials, covariates, offset and scale weight cannot be SUBJECT or WITHINSUBJECT variables (a factor can). To order by a variable that is also a covariate, use a copy of it, as in `COMPUTE time = wave - 1`.
  - User-missing values of SUBJECT and WITHINSUBJECT variables are always missing; CLASSMISSING=INCLUDE applies to factors only, as the subject variables have their own missing value rule. A FIXED matrix must have a smallest eigenvalue above 1E-6.
  - A frequency weight copies the whole subject, so it must be the same for all cases of a subject; otherwise the command stops with an error.
  - The Correlated Data Summary is laid out as levels of each subject and within-subject variable, the number of subjects, the smallest and largest number of measurements and the dimension of the matrix.
- SAVE gives predictions to every case with valid factors, covariates and offset whose factor levels were fitted; residuals, leverage and Cook's distance only to the cases of the fit. For events/trials, MEANPRED and the intervals are numbers of events. A default name already in use gets a suffix _1, _2 and so on; a name given that exists is an error.
- Not supported yet: OUTFILE, DISTRIBUTION=TWEEDIE, INITIAL, CITYPE=PROFILE, PRINT GEF, HISTORY, LAGRANGE and LMATRIX, and with REPEATED the multinomial distribution and generalized score tests (ANALYSISTYPE with LR). WORKINGCORR without REPEATED is accepted and ignored, as there is no working correlation then.

### HILOGLINEAR

Choices where the documentation says nothing:

- A case with a weight of 0 or less is not counted anywhere. Data Information counts unweighted cases: valid, out of range and missing (a case that is both missing and out of range counts as missing).
- A CWEIGHT variable gives each cell the case-weighted mean of its valid values in that cell; a cell without cases gets 1. Cases in a cell whose weight is 0 are dropped from the margins, with a warning, so a structural zero never holds data.
- The goodness-of-fit tests use the adjusted df (cells fitted as 0 left out); a caption gives the unadjusted df when the two differ. The tests of k-way effects and partial associations use the unadjusted df, and their models are fitted to the counts without DELTA.
- Partial associations are given for every term below the highest order; the highest-order term's test is the last row of the k-way table.
- Backward elimination: a term whose removal leaves the chi-square unchanged has p = 1. The Step Summary lists the generating class of each step with its likelihood-ratio chi-square, then every candidate term with the change in chi-square, its df, p and iterations. The procedure stops after MAXSTEPS removals.
- The convergence table gives the largest gap between the observed and fitted margins of the generating class.
- RESID plots the standardized residuals against the observed and against the expected counts. NORMPROB plots the adjusted residuals, from the leverages of the dummy-coded model of every term in the generating class, against normal quantiles of i / (n + 1).
- With several DESIGN subcommands each design's tables carry "Design n" in their caption.

### GENLOG

Choices where the documentation says nothing:

- A logit model's cells run with the dependent factors first and the BY factors after them, the last factor changing fastest. Percentages in the cell table are of the whole table, also in a logit model.
- A model is saturated when it has as many non-redundant columns as cells outside the structural zeros; it is then fitted to the counts plus DELTA throughout, and the tables show those counts, as for HILOGLINEAR.
- The convergence test needs both parts of the manual's rule: the fitted values change by less than CONVERGE (absolutely or relative to their size) and the root mean square of the score of the parameters other than the constant and the totals is below CONVERGE.
- Under MULTINOMIAL the constant and the BY combinations are not listed among the estimates, the covariance and correlation matrices or the design matrix.
- The analysis of dispersion takes as model df the non-redundant parameters of terms that involve a BY factor or a covariate, and as total df (N - 1)(r - 1), with N the weighted count and r the number of combinations of the dependent factors; the residual df is the difference.
- GLOR coefficients that do not sum to 0 (within each BY combination of a logit model) are reported in a warning and not tested.
- The deviance residual of an empty cell is minus the root of 2m. Residual plots and normal plots use the cells outside the structural zeros; normal quantiles are of i / (n + 1).
- SAVE gives cases in a structural zero, and cases missing on a variable of the command, the system-missing value. Saved variables have format F8.2.
- A design of more than 3000 columns is refused, since the fit works with dense matrices of that size.

### PROBIT

Choices where the documentation says nothing, and where it is not followed:

- A case weight multiplies both counts. A case with no subjects (0) adds nothing and is left out; more responses than subjects, or fewer than 0, stops the command.
- A factor value is cut to a whole number before the range check. Levels without cases get no intercept.
- Without a factor the model has one intercept, so the Pearson df is the number of cases less the slopes, the intercept and an estimated natural response rate. The manual's df formula for a factor ((levels - 1) times cases less parameters) is not used: every case is one covariate pattern.
- Estimates are maximum likelihood by Newton steps with step halving, from zero slopes and intercepts, using the observed information where it is positive definite and the expected one otherwise. STEPLIMIT is accepted but not needed by this method. Iteration stops when neither the parameters nor the log-likelihood change by more than the root of OPTOL, relative to their size (at least 1).
- A control case (some covariate 0) is left out of the fit under a logarithm and kept with LOG=NONE; either way it starts the estimated natural response rate. The start is kept between 0.000001 and 0.99. An estimate that ends at the bound 0 is reported as known, with a warning, and leaves the covariance matrix.
- A negative covariate under a logarithm leaves the case out, counted in Data Information.
- The heterogeneity factor is used when the goodness-of-fit significance is below P (always with P=1, never with P=0) and the test has df above 0. It widens the fiducial limits and the relative median potency limits; the parameter table keeps its plain standard errors and z-based 95% intervals.
- Relative median potency: the limits solve Fieller's equation for (a_k - a_j)/b exactly. The manual's printed v12 has the opposite sign of the covariance that equation needs; ours agree with the effective-level limits, whose printed formula is consistent. Each pair of levels is listed once, the lower level first; the estimate is the first level's median dose over the second's under a logarithm.
- The parallelism test fits each level on its own with the natural response rate held at the common model's value, and needs a single covariate (a warning otherwise).
- The plot shows the probit or logit of each case's observed rate (cases with none or all responding are left out) against the transformed covariate, one group per factor level.
- Cell Counts and Residuals shows the covariates as given, not transformed.

### CURVEFIT

Choices where the documentation says nothing, and where it is not followed:

- TSET is not supported, so nothing is saved without SAVE (as under TSET NEWVAR=NONE); the CIN subcommand gives the interval level and CONSTANT is the default.
- A saved number counts up per dependent and model: FIT_1, ERR_1, LCL_1 and UCL_1 for the first model, the next free number for the next. Fits and intervals go to every case with a valid independent (with the case sequence, every case); residuals need the dependent too.
- The prediction interval is back(v -/+ t s sqrt(1 + h)) with h the leverage including the constant, which is the manual's 1 + h + 1/n with a constant and its 1 + h without. The log models without a constant use 1 + h too (the manual prints 1/n for them in every case).
- A model whose linear form is undefined for a case of the analysis (a logarithm of a value not above 0, 1/0) is not fitted, with a warning; the other models still are. QUADRATIC and CUBIC are not fitted when a term's tolerance is below 0.0001.
- The Coefficients table of PRINT=ANOVA gives b0 and the multiplicative b1 as exp of the linear form's coefficient, with exp times its standard error; Beta, t and the significance are those of the linear form.
- PLOT gives one chart per dependent: the observed values and every fitted curve at the observed x, sorted by x; a line chart against the case sequence, a scatter plot against an independent. ID labels the observed points. TEMPLATE is ignored with a warning.
- APPLY is not supported yet.

### WLS

Choices where the documentation says nothing, and where it is not followed:

- TSET is not supported, so WGT_n is saved only with SAVE=WEIGHT, under the first free number. Its values cover every case with a source value above 0.
- A case whose source (or WEIGHT variable) is not above 0 is left out of every fit, with a warning, so that all powers fit the same cases.
- The Standardized Coefficients use the weighted sums of squares about the weighted means (about 0 without a constant), and show their own standard error.
- The power with the largest log-likelihood is used; the first one on a tie. PRINT=ALL adds a Coefficients table for every power, in the order given.
- APPLY is not supported yet.

### UNIANOVA and GLM

- A frequency weight is rounded to a whole number, as documented (GNU PSPP uses fractional weights as they are).
- An interaction with empty cells has as many degrees of freedom as its parameters that are not redundant (GNU PSPP counts every cell, filled or not).
- The Tests of Between-Subjects Effects show the Intercept.
- Covariates, Type IV sums of squares, EMMEANS, POSTHOC, CONTRAST, LMATRIX, MMATRIX, SAVE, WSFACTOR and the multivariate model are supported.
- Nested design terms such as a(b(c)) nest at most 64 levels, as expressions do.
- The columns of an effect are the level combinations that occur in the data, so an L matrix lists one coefficient per occurring combination.
- Type IV spreads each coefficient of the tested effect equally over the cells of a containing effect at those levels of its other factors where every level with a coefficient has a cell, and sets the rest to 0.
- Estimated marginal means weight the cells of an effect equally over the levels not fixed by TABLES (with empty cells: over the cells that occur). A mean that is not estimable is left empty with a note.
- COMPARE adjusts for the k(k - 1)/2 pairs of the compared factor.
- CONTRAST compares the marginal means of the factor's levels.
- A custom test (CONTRAST, LMATRIX) with a row that is not estimable shows the estimable rows in Contrast Results, but no joint test: the documented hypothesis is testable only when every row is estimable.
- Levene's test uses the absolute residuals of the model in the cells of all factors; with a full factorial and no covariates, also the median, the median with adjusted df and the 5% trimmed mean.
- POSTHOC uses the observed means with frequency weights and the model's error mean square. T2, T3, GH and C need a model with one factor.
- With WSFACTOR, the Tests of Between-Subjects Effects analyze the sum of the measures over the root of their number, the averaged tests use orthonormal polynomial contrasts whatever the contrast type, and the contrast type shapes the Tests of Within-Subjects Contrasts.
- Saved variables are F11.5 scale variables named PRE_1, RES_1 and so on.
- A nested effect such as a(b) shows as DESIGN writes it, and parameter names such as [a=1] keep names and values.
- Plots come as chart data. A profile plot is a line chart (TYPE=BAR: a bar chart) of the estimated marginal means as EMMEANS computes them, with WITH fixing covariates for every crossing of that PROFILE list; a mean that is not estimable has no value. MEANREFERENCE draws the grand mean (the estimated marginal mean of the whole model at the same covariate values) as a flat line. Error bars use the standard error of each estimated marginal mean, the same one the EMMEANS table shows: CI reaches the t quantile at 1 minus ALPHA/2 on the error degrees of freedom times it on each side, SE(n) reaches n times it. A mean that is not estimable has no bar, and neither has the grand mean line. YAXIS is left to the drawing.
- SPREADLEVEL plots the cells of all factors that hold more than one case (frequency weights, as in the descriptive statistics), the mean against the standard deviation and the mean against the variance, each cell labeled. RESIDUALS gives one point per case: the predicted value across, the observed value up and the standardized residual (the residual over the root of the error mean square divided by the REGWGT weight) as the third coordinate, from which the matrix of the three pairs is drawn.
- PLOT with WSFACTOR is ignored with a warning.
- GNU PSPP 2.0 has no multivariate GLM (several dependents without WSFACTOR). Its numbers were compared with R run as a separate program on synthetic data: `manova` and model comparisons of multivariate `lm` fits for the four statistics and their F approximations, and matrix arithmetic on the fitted model (sum-to-zero codings) for the Type III SSCP matrices, Box's M, Bartlett's test and the custom tests.
- In the multivariate model the four statistics use the rank of the error SSCP as the number of variables. Roy's largest root has the hypothesis df plus the error df minus the larger of the hypothesis df and that rank as its error df, as R does. Box's M uses the cells of all factors, leaves out a cell whose covariance matrix is singular, and is not given when fewer than two cells remain. Its F takes the one approximation of the GLM algorithms, with the absolute value in its denominator df; the Box's M of DISCRIMINANT follows that procedure's own algorithm, which switches formulas for small cells, so the two can differ there.
- MMATRIX or KMATRIX without LMATRIX tests the L matrix of the intercept's Type III test, whatever METHOD says. The documentation names the estimable function of the intercept without saying which form.
- In the multivariate model EMMEANS, POSTHOC, PLOT and SAVE run for each dependent in turn, with that dependent's tables. EMMEANS COMPARE gives the univariate tests only; SAVE takes no variable names.

### MIXED

- GNU PSPP has no MIXED. The numbers were compared with R's nlme package (`lme` and `gls`), run as a separate program on synthetic data. nlme reports standard deviations and correlations; the tables here give variances and covariances, as the documentation does, and a compound symmetry structure as a diagonal offset and a common covariance. The -2 log likelihoods agree.
- Degrees of freedom are Satterthwaite's by default, as documented; nlme uses its own count by grouping level. They were checked against a separate calculation in R from numerical derivatives of the likelihood.
- Standard errors of covariance parameters come from the observed Hessian of the likelihood, the same Hessian that the Satterthwaite degrees of freedom use.
- The Type III test of the Intercept tests the intercept at the average of the factor levels (covariates at 0), as UNIANOVA does, not the coefficient of the last cell.
- Information criteria: for REML, n is the number of cases less the number of fixed parameters that are not redundant, and only covariance parameters count; for ML, n is all cases and both kinds count.
- COVTYPE(VC) on REPEATED runs as DIAG with a warning. A case with a frequency weight above 1 counts that many times; with REPEATED the weight must be 1.
- The likelihood can have more than one local maximum (for example COVTYPE(ID) over effects on very different scales). The fit starts from the least squares residual variance shared out, as documented, and can stop at a different maximum than another program.
- When an iteration cannot improve the likelihood within MXSTEP halvings, or MXITER runs out, the fit stops with a warning and shows the estimates it reached.
- SAVE names are FXPRED_1, PRED_1, RESID_1, SEFIXP_1 and DFFIXP_1, or the names given. RESID is the dependent minus PRED.
- Cases that a random effect without SUBJECT links into one covariance block of more than 2000 cases are not supported yet; give RANDOM a SUBJECT.
- EMMEANS and TEST were checked against a separate R calculation: the means built from the predicted cells, their standard errors from the covariance of the fixed estimates, and the Satterthwaite degrees of freedom of each mean, difference and F test from numerical derivatives. Every estimated marginal mean and every pairwise difference has its own degrees of freedom, so the table of means has a df column. The covariate means that EMMEANS uses count frequency weights but not REGWGT.
- Pairwise comparisons with REFCAT list each level against the reference level once, and the Bonferroni and Sidak adjustments count those comparisons (levels less one) instead of every pair.
- TEST tables: Contrast Estimates (each row's estimate, the value it is tested against, standard error, df, t, significance and the interval of the estimate) and Test Results (the joint F test of the rows). A test without a label is called Custom Hypothesis Test 1, 2 and so on.
- PRINT G without RANDOM and PRINT R without REPEATED are ignored with a warning. R shows the structure of one subject without regression weights. COVB shows 0 and CORB nothing for a redundant parameter.
- Covariance structures are written as the mixed-model literature writes them: CSH, ARH1, TPH, AD1 and UNR scale correlations by the standard deviations of a variance per level; ARMA11 has a variance, gamma (the correlation of neighbours) and rho (the factor for each further step); HF has a variance per level and lambda, the covariance of two levels being their mean variance less lambda; FA1 and FAH1 have loadings and a diagonal offset. Variances that scale correlations stay above 0, other variances and offsets at least 0, and correlations between -1 and 1; nothing else is constrained, so a fit can stop where the matrix is just positive definite. The loadings of a factor are shown with a positive sum, since their negatives fit the same. A structure over a single level is fitted as one variance (ID), with a warning, since its other parameters would enter nothing; ARMA11 and FAH1 over two levels are refused, having a parameter more than the matrix shows. nlme fits the structures it has (corCompSymm, corAR1, corARMA and corSymm with varIdent) to the same -2 log likelihood; the others were checked against a separate calculation in R.
- DFMETHOD(KENWARDROGER) follows Kenward and Roger (1997), the covariance of the covariance parameters being the inverse of the expected information, as in the paper and in R's pbkrtest, whose numbers it reproduces (F, its scale, the denominator df and the adjusted covariance). For one row the df equal Satterthwaite's computed with that covariance. The adjusted covariance serves EMMEANS, TEST, COVB, CORB, SEFIXP and DFFIXP too; rows with random effects (SOLUTION on RANDOM, SEPRED, DFPRED, and every row of a TEST in which some tested row has coefficients after `|`) keep the unadjusted covariance and Satterthwaite's df.
- TEST: ALL and effect names never mix in one TEST, in a row or across rows; the first of them sets the form, as the documentation asks of the rows. **Beyond the documentation:** coefficients for random effects may come as bare values after `|` in either form, also as a whole row, since an effect on two RANDOM subcommands can be given no other way.
- DFMETHOD(KENWARDROGER) with METHOD=ML: the documentation does not say what happens. The method was derived for REML estimates, so with ML it still runs, on the ML estimates, but with a warning that the adjustment is then only approximate.
- SSTYPE(1) builds each effect's hypothesis from X'X (with regression weights) after the earlier effects, as the sums of squares appendix does, so its rows do not depend on the covariance parameters; the rows of redundant columns are left out.
- The predicted random effects, TEST rows with random coefficients, SEPRED and DFPRED use the prediction error covariance of the mixed model equations. Their Satterthwaite df take the gradient in the covariance parameters by central differences (or a one-sided difference at the edge of the parameter space). The random coefficients of a TEST row are one subject's; with SUBJECT they are divided among all subjects.
- PRINT CPS counts cases with their frequency weights. HISTORY shows the covariance and the fixed parameters at the start and after every n-th iteration, and the last iteration always; the update type is Scoring for the first SCORING iterations and when a Newton step falls back to the expected Hessian.

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
- The goodness-of-fit test of PARALLEL and STRICTPARALLEL uses the determinant of the constrained matrix, as the manual's text describes (its likelihood ratio formula prints k + 1 and 1/k where that matrix has k - 1 and 1/(k - 1)).
- FRIEDMAN and COCHRAN use the values as they are, so the items must be ranks within each case, or 0 and 1.
- With ICC MODEL(MIXED), the average measures are computed as if there were no interaction, with a caption saying so.
- Cases with a zero or missing weight are not counted in the Case Processing Summary.

### WEIGHTED KAPPA and FLEISS MULTIRATER KAPPA

- Case weights are rounded to whole numbers, and a weight of 0.5 or less drops the case. The manual says so for FLEISS MULTIRATER KAPPA; WEIGHTED KAPPA does the same, as its counts are whole numbers.
- The confidence interval is kappa plus and minus the normal quantile at 1/2 + level/200 times the asymptotic standard error. The FLEISS MULTIRATER KAPPA formula prints 1 - level/200, which would not give a two-sided interval; its example table agrees with 1/2 + level/200.
- MISSING CLASSMISSING=INCLUDE makes user-missing values valid; system-missing values always drop the case or pair.
- WEIGHTED KAPPA: values are used as they are (FLEISS MULTIRATER KAPPA cuts fractions, as documented). String values lose trailing blanks. The kappa table adds the number of cases of each pair. A pair of a numeric and a string variable, or with fewer than two cases or one category, is skipped with a warning. With two lists, a variable on both lists gives two pairs, one for each order. With one list, TABULATION names a pair in either order; its table keeps the earlier variable in the rows.
- FLEISS MULTIRATER KAPPA: the raters must all be numeric or all string, and a variable may be named once; anything else is an error. A category shows with the value label that the first rater's variable gives it.
- **Beyond the documentation**: WEIGHTED KAPPA also accepts a leading VARIABLES=, as FLEISS MULTIRATER KAPPA does.

### RATIO STATISTICS

- Case weights are rounded to the nearest whole number, halves up, as the syntax chapter says (the algorithms chapter speaks of truncation). A case needs a numerator and a denominator above 0, as the algorithms chapter requires.
- Groups include missing values of the group variable, system-missing sorting first in ascending order. Groups are formed from the cases used.
- MDCOV divides the squared distances from the median by N - 1, like the standard deviation.
- The weighted mean's interval uses t with N - 1 df like the mean's. The variances of the means of numerators and denominators divide by N, counting a frequency weight as repeated cases; the manual's sum of squared weights over N squared gives the same with unit weights.
- The median's interval is the distribution-free one, with its actual coverage. When even the smallest and largest ratios do not reach the level (five or fewer ratios at 95%), the interval is empty.
- BCOC intervals include both ends; WCOC counts the ratios from (1 - g) times to (1 + g) times the median.
- Percents in the Case Processing Summary are of all cases with a positive weight.
- OUTFILE writes the overall record first, with the group variable empty, then one record per group, in the order of the table. Variable names: Mean, Median, WgtMean, Minimum, Maximum, Range, StdDev, AAD, PRD, COD, MnCOV, MdCOV, BCOC_1 and on, WCOC_1 and on, and _Lower, _Upper and Median_Coverage for the intervals, each with a label. OUTFILE under SPLIT FILE is an error.

### POWER

All eleven POWER commands are supported: MEANS ONESAMPLE, INDEPENDENT and RELATED, PROPORTIONS ONESAMPLE, INDEPENDENT and RELATED, ONEWAY ANOVA, PEARSON ONESAMPLE, SPEARMAN ONESAMPLE, PARTIALCORR and UNIVARIATE LINEAR.

- The commands need no data and change nothing.
- A one-sided test looks in the direction of the effect given (the sign of MEAN minus NULL, ALTERNATIVE minus NULL, p1 - p2 or the difference of the discordant shares).
- With POWER, a mean difference of 0 is an error, as equal proportions are.
- A power that no sample size reaches leaves the size empty, with a warning.
- The table shows the effect size and the mean difference without their sign for a two-sided test.
- PROPORTIONS RELATED: of VALUES=PROPORTIONS(a b), a is the share of pairs with a success first and a failure second and b the reverse; with MARGINAL=TRUE, a and b are the success rates of the first and the second occasion. The risk difference is b - a, the risk ratio b / a. Marginal rates and a correlation that would make a discordant share negative, or the two add up past 1, are an error. CORRELATION may be left out and is then 0.5.
- PROPORTIONS INDEPENDENT with ESTIMATE=BINOMIAL: the one-sided chi-square test with a continuity correction subtracts the correction from the difference, as the two-sided test does (the algorithms chapter adds it, which would make the correction raise the power). POOLED=FALSE uses the separate standard errors of the observed proportions.
- ESTIMATE=BINOMIAL while searching for a sample size: sizes above 500 trials in all use the normal approximation, and a found size above 500 gives a warning. For two groups the search rounds a fractional first size up and the second size (ratio times the first) to the nearest whole number. With METHOD=FISHER the search runs on the exact chi-square test (pooled, no continuity correction), with a warning, and the table gives the power of Fisher's test at the size found.
- When TIMER runs out, there is a warning and no table.
- **Beyond the documentation**: N, PROPORTIONS and the values of VALUES may be written with or without parentheses.
- Charts come as data: a line chart of power for a keyword alone, and power at a grid of points of both quantities (a plot in three dimensions) for a pair of _XAXIS and _YAXIS keywords. VROTATE and HROTATE are checked and have no effect. A lone _XAXIS or _YAXIS keyword is an error.
- Default chart ranges: sample sizes from the smallest allowed to twice the size in the table (at least 10 more); effect sizes from 0 to twice the effect (from minus to plus that for a one-sided test, within -5 and 5); the group size ratio from 0.2 to 5; proportions and correlations from 0.01 to 0.99 (-0.99 for correlations); risk and odds ratios from half the smaller to twice the larger of 1 and the ratio in the table. Points where the test is not possible (a proportion outside 0 to 1) are left out.
- ONEWAY ANOVA: the overall test is always two-sided; TEST in CONTRAST sets only the direction of the contrast test, and a one-sided contrast test looks in the direction of the contrast of the means given. Coefficients that do not add up to 0 (beyond rounding, 1e-10 of the largest) are corrected as described, with a warning. With POWER and all means equal there is an error; a contrast or a pair of groups with no difference gets an empty row and a warning. GROUP_WEIGHTS is ignored when GROUP_SIZES is given. With POWER each test (overall, contrast, every pair) gets its own sizes, listed in a Group Sizes table. The effect size of the overall test weights the grand mean by the group sizes (by GROUP_WEIGHTS when searching). A CONTRAST subcommand with neither COEFFICIENTS nor PAIRWISE=TRUE is ignored with a warning. The charts show the overall test. TOTAL_N keeps the groups in proportion; its default range runs from the smallest total that leaves every group at least 2 to twice the table's total, and a given range must stay within that smallest total and the total at which the largest group reaches 5000. POOLED_SD runs from half to twice the value given.
- PEARSON ONESAMPLE, SPEARMAN ONESAMPLE and PARTIALCORR: NULL and ALTERNATIVE may be -1 or 1 when N is given (a perfect correlation on one side only has power 1); with POWER they lie strictly between -1 and 1 and differ. SPEARMAN ONESAMPLE follows the PEARSON rules here, though its page states less. Equal NULL and ALTERNATIVE with N give the power at no effect. The two-sided sample size search of PARTIALCORR uses the two-sided power (the algorithms chapter writes the search with one tail only, which would differ only in the far tail). The sample size never falls below the smallest N the test allows. Correlation chart axes run from -0.99 to 0.99; PARTIALOUT from 0 to N - 4. PEARSON accepts ADJUST BIAS as two words, as its syntax chart writes it.
- UNIVARIATE LINEAR: TEST_PREDICTORS is the number of predictors tested, those of the full model that the nested model leaves out, in both ways of giving the effect; the algorithms chapter uses it that way as the numerator degrees of freedom, though the keyword's description can also be read as the size of the nested model. Given FULL_MODEL and NESTED_MODEL, the partial correlation is sqrt((full - nested) / (1 - nested)). The random-predictor power sums the series from its largest term outwards, to 1e-17 of the total, so it holds for large samples too. The PARTIAL_CORR chart runs from 0 to 0.99 (the power depends only on its square). An F test has no direction, so the table has no note on sides.
- An abbreviation that fits keywords of different families (GRO for GROUP_SIZES, GROUP_MEANS and GROUP_WEIGHTS, RIS for RISK_DIFF and RISK_RATIO) is an error; one that fits a keyword and its longer forms (NUL for NULL and NULL_XAXIS) names the shorter.
- The NRATIO chart keeps n1 + n2 and moves the split (rounded for proportions); MEANS INDEPENDENT NRATIO_XAXIS and NRATIO_YAXIS take ratios, not whole numbers. RISK_DIFF, RISK_RATIO and ODDS_RATIO move p1 (INDEPENDENT) or b (RELATED) and keep the other; DIFF moves ALTERNATIVE; NULL and ALTERNATIVE each move one value.

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
- The scree plot shows the eigenvalues of the correlation matrix whatever the extraction. The loading plot shows the rotated loadings (the pattern for oblique rotations) when there is a rotation, else the extracted ones; with one factor there is none, with a warning.

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
- Plots come as chart data. The cases are laid out so that no branches cross: a join puts the second cluster's cases after the first's. The dendrogram rescales the heights to 1 at the first join and 25 at the last, so the first joins stay apart from the cases. Icicle clusters are numbered along that layout.
- Saved memberships are F8.0 nominal variables labeled with the method and the number of clusters.

### PROXIMITIES

- The standard deviation of Z and SD has n - 1 in its denominator. MAX divides by the largest value, not the largest absolute value.
- The diagonal of the Proximity Matrix and of a matrix file holds each item's proximity with itself. RESCALE on MEASURE maps the smallest and the largest entry of the whole matrix, the diagonal included, to 0 and 1.
- A pair for which the measure is undefined (such as JACCARD of two cases with no presence) is system-missing, with a warning; the command goes on.
- With MATRIX=IN no measure is computed: MEASURE and STANDARDIZE are ignored with a warning, and only ABSOLUTE, REVERSE and RESCALE apply.
- Under a LAYERED split the Proximity Matrix of cases heads its columns by place in the group (1, 2, ...), so a layered table lines the groups up; the rows keep the case labels. MATRIX=OUT(*) keeps the split on the new matrix dataset, so MATRIX=IN(*) reads it group by group.
- Cases are labeled with their case number, followed by the ID value after a colon. The ID variable in a matrix file keeps its values whole.
- MEASURE=DEFAULT is EUCLID. A matrix that MATRIX=IN read and MATRIX=OUT writes again has only SIMILARITY or DISSIMILARITY as the value label of PROX. Reading a matrix file without an item list, the items are those with a row of type PROX or a value in one (a split group may have fewer). Under SPLIT FILE a matrix file in another dataset or on disk is cut to the rows whose split variables have the values of the group that runs. ALL in the item list means every item of the group. A split variable that is numeric in one file and a string in the other is an error. An item with two rows is an error, and so is an item named twice in the list. An ID variable named like another variable of the matrix file is an error.
- In a matrix file the order is ROWTYPE_, the ID variable, CASENO_, VARNAME_. Under SPLIT FILE a group with fewer items than another has its missing item variables blank, and the item variables carry no labels.

### PROXSCAL

- Normalized raw stress is reported with the configuration optimally dilated, 1 - rho^2 / (eta^2(dhat) eta^2(d)); Stress-I and Stress-II are the square roots of the manual's ratios, S-Stress is divided by the sum of the fourth powers of the transformed proximities. The decomposition gives every object half of the misfit of each of its pairs, so its rows and its columns both add up to the normalized raw stress.
- The transformed proximities are normalized to n(n-1)/2 weighted squares per source (per all sources with UNCONDITIONAL), also for the first iteration.
- With SHAPE=BOTH a pair's weight is the sum of the two triangles' weights. With TABLE a pair given only in the other triangle is used too.
- INTERVAL and SPLINE shift the proximities so the smallest is 0 before fitting (the intercept absorbs it). SPLINE adds a nonnegative intercept to the monotone spline basis, so it fits at least as well as INTERVAL; the inner knots sit at equally spaced quantiles of the proximities.
- ACCELERATION doubles the step of the identity model; a doubled step that would raise the stress is replaced by the plain one.
- The individual differences models restart a smaller dimensionality from the common space's principal axes with identity weights (the weighted model keeps its most important dimensions and their weights).
- Each dimension of the output is signed so that the first object with a coordinate other than 0 is positive; the weighted model's dimension weights are all positive.
- RANDOM starts draw from the session's random generator (SET SEED); Random Starts lists each start's stress, not its seed.
- The output after several dimensionalities gives every table for each, titled with the number of dimensions; the charts and OUTFILE show the smallest.
- With MATRIX=IN the matrix is scaled as similarities when the file says SIMILARITY or PROXIMITIES=SIMILARITIES is given. The file is one source: TABLE is ignored with a warning, and under SPLIT FILE every group is scaled on its own.
- The weight of WEIGHT is not used, also with TABLE.
- TABLE row and column codes must number the objects 1, 2, ... without gaps; otherwise the command stops before building the matrices.
- REDUCED(r) starts from identity weights cut to r dimensions and keeps every update at rank r in the metric of the majorizing function, so stress never rises. The classical scaling start gives a pair missing in every source the mean squared proximity.
- With TABLE row BY column BY source the sources are the codes that occur, in order; a gap in the codes makes no empty source.
- PLOT=TRANSFORMATIONS and RESIDUALS give one plot per parenthesized list of sources. SPLINE accepts INKNOT=0.

### ALSCAL

- Only symmetric matrices with the EUCLID and INDSCAL models: SHAPE=ASYMMETRIC and RECTANGULAR, CONDITION=ROW, ASCAL, AINDS, GEMSCAL and FILE are refused with an error.
- INTERVAL(n) and RATIO(n) fit the squared disparities as a polynomial of degree n of the squared data (with and without an intercept) to the squared distances, so the disparities of RATIO(1) are proportional to the data and exact distances fit with S-stress 0 at every level.
- The start fills a missing value with the mean of its matrix (NOULB changes nothing) and starts INDSCAL with all weights 1, without a rotation toward diagonal weights; the iterations find the orientation. Without NEGATIVE the weights are nonnegative least squares.
- CONVERGE(0) runs until S-stress stops falling or ITER is reached.
- Kruskal's stress compares the distances with the root of the squared disparities and divides by the sum of squared distances; the average over matrices is the root of the mean of the squared values.
- The Euclidean coordinates are rotated to principal axes with a mean square of 1; the INDSCAL coordinates have a sum of squares equal to the number of stimuli in every dimension, the dimensions ordered by their sum of squared weights. Each dimension is signed so that the first stimulus is positive.
- With MATRIX=IN the stimuli are named by the matrix variables; an ID variable in the file is not used.
- The output file has no flattened weights (FLA). MATNUM is 0 for the coordinates.
- A similarity matrix file is scaled with SIMILAR at the ordinal level; at INTERVAL and RATIO each similarity is taken from the largest one, with a warning; NOMINAL uses the values as read. With MATRIX=IN the VARIABLES list picks the items of the file (TO runs in the file's order).
- Under SPLIT FILE each group runs on its own rows and writes OUTFILE in turn, so the file keeps the last group (PROXSCAL OUTFILE alike).
- Several dimensionalities give every table for each, titled with the number of dimensions; the plots show the smallest.

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
- PLOT comes as chart data and a table. COMBINED and SEPARATE plot the classified cases (ungrouped ones in a group of their own on the combined plot) on the first two functions, with the group centroids labeled; with one function they are histograms of the scores. Weighted cases are plotted once each.
- CASES lists the classified cases in file order in Casewise Statistics. The significance of the highest group is that of its squared distance under chi-square with as many df as functions. A misclassified case has ** after its predicted group. There are no cross-validated rows yet.
- MAP gives each group's territory as a convex polygon: the points of the plane of the first two functions where the group has the highest ln(prior) minus half the squared distance to its centroid. The polygons fill a box around the plotted cases and centroids, widened by 1 on each side. Beyond two functions the map ignores the others, so a case may lie outside the territory of the group it is classified into. With CLASSIFY=SEPARATE, or with one function, there is no map, with a warning. The window and `--charts` draw each territory as a tinted polygon outlined in its color, with each centroid as a dot labeled with its group; the office files hold the map as its data table.

### CORRESPONDENCE

- Signs of dimensions are arbitrary in any singular value decomposition. Here, in every dimension, the active row with the largest absolute score has a positive score; the column scores follow. Other software may show a dimension mirrored.
- Margins, masses and the grand total come from the active part of the table: supplementary and empty rows and columns are left out, and the members of an EQUAL group count once, in the group's first category.
- A supplementary point is placed by projecting its cells of the standardized table on the solution, with its own total over the active categories as its row or column total. For the chi-square measure and for RMEAN and RSUM this is the profile formula of the Algorithms manual; for CMEAN and CSUM, and for PRINCIPAL normalization, the manual's formula would not put a supplementary copy of an active point on that point, so the projection is used. Its mass is its total over the active part divided by the active grand total (1/rows or 1/columns where the standardization fixes the masses); its share of every dimension is 0.
- EQUAL: the first category of a group carries the merged mass, inertia and contributions; the other members show the shared scores and leave the rest empty. The input table and the profiles show the categories as they were. Confidence Row and Column Points show the group on its first category; the VARIANCE file leaves the group out, as it does supplementary points. Under SPLIT FILE every group writes the OUTFILE files again, so they hold the last group.
- Standard deviations and correlations of the scores come from the delta method on the standard scores (the generalized singular vectors, with the change of the masses), then rescaled by the power of the singular value the normalization gives; that rescaling treats the singular value as fixed, as the manual describes. The Summary gives the standard deviation of every singular value it lists and the correlations among the dimensions of the solution. The numbers were checked against R: MASS::corresp and a base singular value decomposition for the solution, and central differences of the whole solution for the delta method.
- Chi Square and Sig. appear only with the chi-square measure. With MEASURE=EUCLID, RCMEAN uses the row and column totals as its metrics, as the manual's formulas say, so its results equal the chi-square analysis.
- Values of the TABLE variables are truncated to whole numbers. With TABLE=ALL a missing or negative cell counts as 0, and WEIGHT does not apply (a warning says so).
- Plots are charts as data: one scatter chart for each pair of dimensions instead of a scatterplot matrix, the transformation plots as line charts of the category scores in the order of the category values (ROWCAT_ codes with TABLE=ALL; case numbers for all rows, with a warning, when ROWCAT_ misses a row). Supplementary points are drawn with the active ones. BIPLOT with NORMALIZATION=PRINCIPAL gives a warning and no chart.
- The last row of the Row Profiles (Mass) and the last column of the Column Profiles are the profiles of the active margins. PERMUTATION puts categories without a score last.
- DIMENSION above the largest possible number gives a warning and the largest number; dimensions with a singular value of at most sqrt(k1 k2) 1e-7 are left out with a warning.
- RMEAN and RSUM take out only the row means (CMEAN and CSUM only the column means), so the table can keep one more dimension than the limit of one fewer than the smaller side. The Summary lists it, so that the shares of inertia add up to 1; the solution itself keeps to the limit.
- Equal singular values (equal to 1e-8 relative) have no derivatives, so they get no standard deviations or correlations, and the scores of their dimensions none either; the other dimensions keep theirs, and a warning says so.
- The biplot that PLOT gives by default is left out quietly under NORMALIZATION=PRINCIPAL and with one dimension; a PLOT subcommand that names a plot or NDIM gets a warning. A table of more than a million cells is refused, and the standard deviations of the scores are left out (with a warning) when their work would be too large: it grows with the cube of the number of active categories and with the square of the number of dimensions, so the limit lies near 400 categories on each side for two dimensions and lower for more.

### MULTIPLE CORRESPONDENCE

- The solution does not depend on a random start. Without CONFIGURATION the iterations start from the exact solution, found from the eigenvectors of the weighted cross products of the categories with the trivial dimension taken out, so the Iteration History shows the start (iteration 0) and one step that confirms it. With more than 600 categories in all the start is a fixed pseudo-random configuration instead, so a run repeats exactly. CONFIGURATION=INITIAL iterates from the file as the manual says. The numbers were checked against R: ca::mjca on the indicator matrix for data without missing values, and a separate alternating least squares script for passive missing values, case weights and variable weights.
- Signs of dimensions are arbitrary. Here, in every dimension, the object with the largest absolute score scores positive.
- NORMALIZATION: the singular value of a dimension is taken as the square root of its inertia (the eigenvalue over the summed variable weights), as in correspondence analysis of the indicator matrix. The objects get its power (1 + q)/2 and the categories (1 - q)/2, both from the variable principal solution. The manual's formula writes the factor with the eigenvalue and the number of variables in a way that does not reduce to a singular value; other software may show the normalized scores on another scale. The eigenvalues, inertias, discrimination measures and fit do not depend on this choice.
- The weight in parentheses on ANALYSIS (and the specifications on DISCRETIZATION and MISSING) covers every variable named since the previous parenthesis: `a b (WEIGHT=2)` weighs both a and b.
- Mass, inertia and contributions use the variable weights too: a category's mass is its variable's weight times its weighted count over the summed active weights of all objects, so masses add up to 1 and the contributions of all categories to a dimension add up to 1. Supplementary variables and categories that only supplementary objects have get centroids but no mass, inertia or contributions. Such a category's centroid is the plain mean of the scores of its supplementary objects.
- A supplementary object is placed by the transition formula: the weighted mean of the centroids of its categories, divided by the inertia of each dimension; a category that no active object has is left out for it. An object with no active value has no score.
- DISCRETIZATION GROUPING with NORMAL cuts the z-scores (with the sample standard deviation) at the optimal quantizer points of a standard normal curve for the number of groups, found by the usual alternation of centroids and midpoints; groups that no case falls into stay empty. UNIFORM adds the distinct values in order to a group until one more would take its count further from n/NCAT than it is. Both rank the values when there are no more of them than groups. MULTIPLYING with a constant variable codes every case 1. RANKING numbers the distinct values (ties share a code). Codes made by grouping are labeled by their number; ranked codes keep the label of their value.
- MISSING ACTIVE EXTRACAT puts the missing values in a new category one above the largest code, labeled Missing; RANDIMPU draws a category with probability proportional to its weighted count, from the session's random number generator. The mode is taken over the cases in the analysis before listwise deletion.
- A case with a weight of 0, a negative weight or a missing weight is left out like a listwise case.
- CONFIGURATION reads the first numeric variables of the file or dataset, one case per case of the active data. With FIXED the coordinates are centered and turned into the nearest uncorrelated dimensions of variance 1 (rather than keeping their own spread), so the discrimination measures stay squared correlation ratios; NORMALIZATION does not apply then, and the Model Summary and Iteration History are not shown, since no variable is active.
- Descriptives count the cases in the analysis with their weights, before any imputation; the mode is the smallest of equal modes. In the correlation tables a passive missing value is filled as the PASSIVE keyword says (RANDIMPU as the mode). The Model Summary has a Total row (alpha from the summed eigenvalue, as the manual's formula) and a Mean row (alpha from the mean eigenvalue).
- Quantifications show centroids and contributions in one table, which has no layers. Object Scores lists the supplementary objects with the others and names them in a note; Object Contributions lists the active objects.
- Plots are charts as data: with two dimensions one scatter, with more a scatterplot matrix, and with NDIM(a, b) one scatter of a against each of a+1 to b. Objects labeled by a variable are colored by its categories. TRANS gives line charts of the quantification against the category code; RESID scatters the quantified value against the object score. BIPLOT with INDEPENDENT gives a warning and no chart.
- SAVE names that are taken move to the next free set number, separately for TRDATA and OBJECT. OUTFILE TRDATA names its variables TRAk_d and OUTFILE OBJECT OBSCOd, without a set number; DISCRDATA keeps the variables' names and holds the codes before imputation.
- SPLIT FILE is ignored: one analysis runs on all cases.

### CATPCA

- The fit does not depend on a random start. The first phase, in which every variable but the multiple nominal ones is numeric, has an exact solution (the leading eigenvectors of the cross products of the standardized codes and the category indicators), so the iterations start from it; its fit is iteration 0. With more than 600 columns in that problem a fixed pseudo-random start is used instead. CONFIGURATION=INITIAL iterates from the file.
- The Iteration History shows iteration 0 and the last iteration, or every iteration with HISTORY. Iteration 0 is left out when every variable is numeric or multiple nominal and no extra category was imputed, since the first phase is then the whole fit.
- Spline levels use I-splines: a spline of the given degree over the codes, with the boundary knots at the lowest and highest code and the interior knots at equally spaced quantiles of the active cases' codes. INKNOT above the number of categories minus 2 is lowered to it. SPORD keeps the spline coefficients at or above 0, so the curve never falls. A spline variable with fewer than two categories is quantified as nominal.
- The quantification of a single variable is restricted on its own scale, then given a weighted sum of squares equal to the weighted number of cases (not centered first). When a restriction leaves no spread (an ordinal variable that runs against the components, for example), the previous quantification is kept for that step.
- Principal axes: after the iterations, the dimensions are turned to the eigenvectors of the summed fitted cross products of the active variables (squared loadings for single variables, centroid variances for multiple nominal ones), so the eigenvalues come out in descending order. A dimension is reflected when the mean square of its negative loadings exceeds that of its positive loadings, as the manual says; with only multiple nominal variables, the case with the largest absolute score is positive.
- Model Summary: the per-dimension eigenvalue is the variance accounted for; percent of variance divides by the summed variable weights. The Total eigenvalue sums the single variables' parts over the dimensions and averages the multiple nominal parts, as the manual's total; the Total alpha uses the plain sum of the dimensions' eigenvalues, as the manual's alpha formula reads. With multiple nominal variables these two totals differ.
- Normalization uses the root of each eigenvalue as the singular value: the scores get its power (1 + q)/2, the loadings and multiple nominal centroids its power (1 - q)/2 less one, and the centroid coordinates of single variables are the centroids of the normalized scores. Categories that only supplementary cases have get the plain mean of those cases' scores as centroid (the manual's formula has an extra root of n that does not fit its own scale).
- Supplementary cases are placed by one more score update from the final quantifications, with the centering and orthonormalizing transformation of the active cases.
- In the correlations of the quantified variables, a passive missing value takes the quantification of the mode, of an extra category (the weighted mean score of the cases with the gap, projected on the loadings for single variables, scaled like the other centroids for multiple nominal ones) or of a random category drawn by weighted frequency, as PASSIVE says. Multiple nominal variables enter the matrix of dimension d through their standardized centroids in that dimension.
- ROTATION reuses FACTOR's rotations on the normalized loadings of the active single variables (at most 100 rotation iterations, FACTOR's convergence criterion). The scores and all centroids turn with the same transformation; supplementary loadings too. The Model Summary and VAF table stay unrotated. A rotation needs two dimensions and two variables with loadings.
- RESAMPLE=BOOTSTRAP is not supported yet and stops the command with an error. The confidence ellipse keywords of PLOT, SAVE and OUTFILE (LDELLAREA, OBELLAREA, CTELLAREA, NELLPNT, ELLCOORD) are read and ignored with a warning.
- ROTATION and RESAMPLE may appear together when one of them asks for nothing (NOROTATE or NONE); only a real rotation with BOOTSTRAP is an error.
- SAVE APPROX takes a root name only; a number of dimensions after it is an error.
- PLOT: a repeated keyword replaces the earlier one. The biplot and triplot shrink the scores (and the triplot's centroids) by the manual's scale factor; a biplot with centroids does not. PROJCENTR projects the centroids of the normalized scores on each variable's loading direction, as a table and as a line chart with one line per category. TRANS draws one line per single variable and one per requested dimension for a multiple nominal one. VAF bars show squared loadings for single variables and centroid variances for multiple nominal ones; the Total bar averages the latter over dimensions.
- SAVE APPROX of a multiple nominal variable holds its approximation in the first dimension (the case's first score), so each variable gets one name as the naming rule describes. OUTFILE names: TRAk (TRAk_d for multiple nominal variables), OBSCOd, APPk, without a set number.
- Variable weights may be any positive number, not only whole numbers.
- SPLIT FILE is ignored: one analysis runs on all cases.

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
- The tree diagram comes as chart data with the statistics of every node; its orientation (DISPLAY) and NODES are not kept.
- PLOT GAIN, RESPONSE and INDEX come as line charts, one chart per keyword with one line per TARGET category, over the cumulative percentile groups of INCREMENT. The terminal nodes take the order of the gain tables (SORT), and the node that holds a cut counts in proportion to its part below the cut. Without a TARGET category there is no chart, with a warning.

### GRAPH

- BAR, LINE, PIE, ERRORBAR, PARETO, HILO, HISTOGRAM and SCATTERPLOT (BIVARIATE, OVERLAY, MATRIX and XYZ, with a control variable and point labels) make one chart, as data and as a picture like the charts of the procedures. The chart's layout follows the function specification; a SIMPLE or GROUPED keyword that does not fit it changes nothing, as the manual says. Several areas stack.
- Categories ascend by value. With MISSING=REPORT the categories of missing values come after the valid ones, user-missing values first and system-missing last.
- PCT is a share of all cases in the chart. CUFREQ, CUSUM and CUPCT add up along the categories within each series, and CUPCT is a share of that series' total, so each series ends at 100.
- MEDIAN and PTILE use (W + 1) p as FREQUENCIES does. MODE shows the smallest of tied values. GMEDIAN treats the values as group midpoints.
- Weights count in every function. VALUE and SCATTERPLOT draw each case once, unweighted, and leave out cases whose weight is zero, negative or missing.
- Without TITLE the chart is named after what it shows (for example "Mean of price by region"), so the output has a name for it; a two-line title or footnote becomes one line. With SPLIT FILE the group goes in the title, as for the other charts. A histogram keeps its line with the mean, standard deviation and N after a FOOTNOTE.
- The limit of 3000 categories is checked; a chart with a single category is drawn.
- ERRORBAR accepts SE as another name for STERROR, as INTERVAL spells it, and the number in parentheses as well as after the keyword. A category whose cases weigh 1 or less in total shows its mean without a bar.
- PARETO: categories with equal totals keep their order by value. NOCUM leaves out the right axis together with the line, although the manual keeps the axis; the axis means nothing without the line.
- BAR(RANGE), LINE(DROP), LINE(DIFFERENCE) and HILO take their series as the ends of the ranges and check that their number fits: pairs for RANGE, two or more for DROP, two for DIFFERENCE, two or three for HILO.
- PANEL draws a separate chart for each combination of the panel categories, named in its title, not a grid of panels sharing axes; each chart scales its own axes. A combination without cases gets no chart, so CROSS and NEST give the same charts. A case missing on a panel variable is left out unless MISSING=REPORT. A panel whose chart cannot be made gives a warning and the others are drawn.
- SCATTERPLOT: the variables decide the kind of plot, as the function specification does for bars, whatever keyword is given. In an overlay the variables before WITH run across and those after it up, as in a single plot; each pair is a group named "y by x", and an axis of several variables is titled by them all and shows the format with the most decimals. LISTWISE leaves out a case missing on any variable of the plot; VARIABLE keeps it in every pair, or every cell of a matrix, where both values are valid.
- The scatterplot matrix is one chart: a square cell for each pair, the column's variable across and the row's up, the variable names on the diagonal. A case needs valid values on two of the variables.
- A scatterplot with three axes (XYZ) is not drawn yet: it shows as the table of its points, with a footnote that says so. Vega-Lite draws its first two axes.
- INTERVAL adds error bars to simple and clustered bar and line charts of MEAN (CI, SE and STDDEV, computed as for ERRORBAR) and of MEDIAN (CI only). The median's interval comes from interpolated order statistics, so its ends need not lie at the same distance; it needs whole case weights. Counts, percentages and the other functions give an error, as do stacked bars and areas: the documented formulas cover means and medians only.
- In the office files a difference area chart, and range bars with more than one pair, go in as pictures; drop-line and high-low charts are line charts of points with high-low lines, error bar charts line charts of points with custom error bars, side by side only in the picture and in Vega-Lite.
- Not supported yet, each with an error: TEMPLATE, HILO with more than three series (GROUPED), and INTERVAL around counts, percentages and functions other than MEAN and MEDIAN.

### PPLOT

- The plots come as chart data and pictures, one per variable and kind: Q-Q (observed value against the expected quantile, both in data units) or P-P (observed against expected cumulative proportion), each with the line y = x, and their detrended forms with the line y = 0. The points are listed by observed value.
- Case weights are not used; the Case Processing Summary says so when a weight is set.
- The half-normal scale is estimated as the square root of the mean square (maximum likelihood), not of the sum of squares.
- The exponential parameter and the second gamma parameter are rates (1 / mean and mean / S^2), as the estimates and the CDF functions of transformations take them; the tables call them Rate.
- Lognormal, Pareto and Weibull estimates take logs, so they need positive values; with parameters given, values of 0 are allowed. Values a distribution cannot take, and a series whose parameters cannot be estimated (one that does not vary), leave the variable out with a warning.
- SDIFF without PERIOD is left out with a warning: there is no TSET, and DATE keeps no periodicity for it.
- Not supported yet: APPLY of a named model (APPLY alone uses the previous PPLOT) and TEMPLATE, each with an error.

### ROC

- Weights count cases in every statistic, not only in the Case Processing Summary, and need not be whole numbers.
- The Case Processing Summary adds a Missing row: the weighted cases left out because the state or any test variable is missing.
- The confidence interval is two-sided at the CI level. The interval is not cut at 0 or 1.
- With TESTPOS(SMALL) and DISTRIBUTION(NEGEXPO), the bi-negative exponential error is computed from the area in the larger-is-positive direction and kept for the turned area, as the formulas give it.
- The curves of all test variables share one chart; the points are joined in order from (0, 0) to (1, 1). PLOT=CURVE draws no diagonal unless REFERENCE is given. PRINT=SE(MODELQUALITY) adds a horizontal bar chart of the lower bounds of the intervals, cut at 0.

Where GNU PSPP differs:

- Its interval uses the one-sided normal quantile (at 90 percent its interval is that of 80 percent) and its coordinates table misses some cutoffs; the area, the standard error and the significance agree.

### MVA

Choices where the documentation says nothing:

- Case weights are not used; every case counts once.
- Extremes: the Algorithms manual draws the limits at the quartiles themselves, the syntax documentation 1.5 interquartile ranges beyond them; Statscraft follows the latter. The quartiles are taken at (n + 1) p, as in EXAMINE's default, and the switch to 2 standard deviations uses the base-10 logarithm, v n log10(n) > 150000, with v the variables on the list and n the cases.
- MPATTERN, TPATTERN and MISMATCH put the variables in order of their number of missing values, fewest first (ties in list order), while DPATTERN, which has no NOSORT, keeps the list order; and sort the patterns by their missing values read from the variable with the most missing values back to the one with the fewest, the complete pattern first. MPATTERN keeps file order among cases of the same pattern.
- MISMATCH PERCENT leaves out the variables with less than that percent missing. CROSSTAB columns are the total and the valid categories; the user-missing rows name the defined value.
- The t test is left blank when a group has fewer than two values. The column of the indicator variable itself shows its counts and present mean.
- LISTWISE uses the cases complete on every variable of the list, categorical ones included.
- EM and REGRESSION run on the variables of both lists (predicted and after WITH) and show the predicted ones; a predictor that is missing is estimated inside EM but not saved. Little's test covers the same variables; a variable that fails the TOLERANCE check in a pattern counts neither in the statistic nor in its df. A pair with fewer than two cases starts EM with covariance 0.
- The summary tables appear when two or more ways to estimate are shown, with a row for each: All Values for the univariate statistics, Pairwise (the same means and standard deviations), Listwise, EM and Regression.
- EM's covariance is divided by (n - 1) sum(w) / n, as the Algorithms manual gives it, so EM with no missing data gives the sample covariances (n - 1), not the maximum likelihood ones. Because the conditional covariances enter the iteration, the result is close to, but not exactly, n / (n - 1) times the maximum likelihood estimate that EM programs with an n divisor give. With the n divisor the same program agrees with a published EM implementation, and Little's test computed from those estimates agrees with a published implementation of the test.
- Convergence follows the Algorithms manual: every variance changes by at most CONVERGENCE relative to its new value. When ITERATIONS runs out first, the last estimates are shown with a warning.
- The mixed normal weight follows the Algorithms manual's formula, with LAMBDA and PROPORTION both required.
- REGRESSION enters predictors while F to enter = (drop in residual variance) / (residual variance / (n_j - q - 1)) reaches FLIMIT, with n_j the valid values of the estimated variable and q the predictors after entry. A candidate whose drop would use up the whole residual variance (pairwise covariances need not be positive definite) is passed over. NPREDICTORS=0 fills every missing value with the mean, with no random term. A complete case for RESIDUAL is one valid on every variable of the model. NORMAL and T errors are scaled by the residual standard deviation of the regression on the pairwise covariances; T is not rescaled to unit variance.
- OUTFILE writes the analyzed cases (with FILTER, the selected ones), with the missing values, system- and user-missing, of the estimated variables filled in, and drops the user-missing definitions of those variables, so that no filled value reads as missing.
- A variable named twice on the list is analyzed once. ALL on CATEGORICAL, EM or REGRESSION means every variable on the MVA list. A quantitative variable with fewer than two valid values is left out of EM and REGRESSION with a warning; when none is left to estimate, that estimate is skipped with a warning and the other tables stay.

### MULTIPLE IMPUTATION

Choices where the documentation says nothing:

- Once MISSINGSUMMARIES or IMPUTATIONSUMMARIES is written, only the keywords on it are shown: `/MISSINGSUMMARIES PATTERNS` drops the overall summary, `/IMPUTATIONSUMMARIES DESCRIPTIVES` the models.
- The overall summary and the patterns are tables. A pattern row marks missing values with X; the columns run from the fewest to the most missing values (ties in list order) and the rows are sorted from the last column back to the first, valid before missing.
- The summaries count replicated cases (WEIGHT rounded to whole numbers). A case whose rounded weight or analysis weight is 0 or less, or whose analysis weight is missing, is left out of everything, summaries included. Means and standard deviations use the weight times the analysis weight.
- A variable without any valid value is left out of the model with a warning.
- AUTO orders the variables from the fewest to the most missing values (ties in list order) for both methods; the Imputation Sequence row shows that order. With an explicit MONOTONE, a variable is predicted only by the variables before it on the list, and the monotone check covers every variable of the model, so a complete variable listed after an incomplete one breaks the pattern.
- Categorical predictors enter as indicators of every category but the last; TWOWAY adds the products of the indicators of each pair of categorical predictors. The multinomial logistic model takes the last category as its base and is fitted by Newton's method (at most 50 steps, halved while the likelihood falls); when the categories are separated, a ridge of 1e-8 on the information keeps the draw defined.
- A linear model needs more replicated cases than parameters; otherwise the command stops with an error.
- RND rounds a drawn value before MIN and MAX are checked, so the saved value meets both. RND also rounds the values that PMM takes, while MIN and MAX do not apply to PMM. PMM predicts the observed and the missing cases with the same drawn coefficients and picks a donor with equal chances among the k nearest, ignoring weights.
- FCS starts a scale variable from normal draws with its observed mean and standard deviation, cut to its observed range, and a categorical one from its observed shares (replicated cases).
- IMPUTATIONS: the new first variable Imputation_ (F8.0, labeled, 0 labeled "Original data") is followed by every variable of the data. The cases are the analyzed ones (with FILTER, the selected ones), first the original data, then each imputation; under SPLIT FILE each imputation holds the groups in order, and the new data are split LAYERED by Imputation_ and then the split variables. The dictionary, missing value definitions included, is copied unchanged. Values that are not imputed (cases left out, ROLE=IND variables, variables left out of the model) keep their original values.
- FCSITERATIONS holds Imputation_, Iteration_ (1 to MAXITER), SummaryStatistic_ (1 mean, 2 standard deviation, labeled), the split variables when SPLIT FILE is on, and one column per imputed scale variable: the mean and standard deviation (n - 1, replicated and weighted) of its imputed values after each iteration.
- Pooling: when Imputation_ is the first split variable, DESCRIPTIVES adds a table of pooled means, with the squared standard error of each mean (variance over the weighted N) as its within-imputation variance, and FREQUENCIES a table of frequencies and percents averaged over the imputations (a value absent from an imputation counts 0 there). Further split variables after Imputation_ give a pooled row per group. When the imputations agree exactly, the df are infinite (left empty) and the interval uses the normal distribution; when every within variance is 0 but the estimates differ, the df are m - 1.
- Pooled tables of the other procedures, each estimate by Rubin's rules with a 95% interval whatever level the procedure itself uses:
  - MEANS: the mean of every Report row (each group and the Totals), variance over the weighted N as within variance. The other cell statistics are not pooled.
  - T-TEST: the mean difference of each test (one sample: mean minus the test value; independent samples: both rows, each with its own standard error; paired samples), with t and its two-sided significance against Rubin's df. Group statistics, Levene's test, the paired correlations and effect sizes are not pooled.
  - CORRELATIONS: every pair of different variables once, as Fisher's z = atanh(r) with within variance 1 / (N - 3), N the weighted count of the pair; the estimate and interval are turned back into r, and the significance is two-sided even with ONETAIL. An imputation where a pair has N of 3 or less or r of plus or minus 1 is left out of that pair.
  - REGRESSION: the coefficients of every model with their t tests, and per model the joint F test that every slope is 0, from the covariance matrix of the slopes. Standardized coefficients, R square and the ANOVA table are not pooled.
  - LOGISTIC REGRESSION: the coefficients of the last step of each block, tested with t against Rubin's df instead of the Wald chi-square, with Exp(B) of the pooled B and its interval (the constant included), and the joint F test that every coefficient but the constant is 0. The overall rows of categorical terms are not pooled.
  - PLUM and NOMREG: every estimated parameter, tested with t against Rubin's df instead of the Wald chi-square; NOMREG adds Exp(B) of the pooled B and its interval (the intercepts included). A redundant parameter (fixed at 0) is not pooled, so one that is redundant in every imputation is left out of the pooled table, and one redundant in some imputations only gives the warning below.
- The joint F test uses a generalized inverse of the average within covariance and takes its rank as the number of parameters; when the estimates do not vary between imputations, the test is a chi-square on that many df (df2 left empty).
- A pooled row or test that some imputations do not give (a group absent from one imputation, a variable entered in some imputations only) is left out of the pooled table with a warning instead of stopping the command; the tables of each imputation stay. A value one imputation could not compute (an empty mean, a standard error that is not finite) is pooled from the other imputations.
- SET MIOUTPUT acts on every procedure run on data split by Imputation_: OBSERVED and IMPUTED choose which groups are shown, POOLED the pooled tables and DIAGNOSTICS the last three columns of the tables of single estimates.

Not supported yet: pooled results of the other procedures (GENLIN and the other models), joint tests for PLUM and NOMREG, pooled statistics in the FREQUENCIES Statistics table, and pooled R square.

### KM

- A case takes part when its time, status, factor and stratum are valid (user-missing values count as missing) and its time is 0 or more; a missing status is left out rather than counted as censored. Weights count cases and need not be whole numbers.
- With LOST, the Censored columns count only the censored cases that are not lost, and the Lost columns the rest.
- The survival table has a row per case, events before censored cases at a tied time; the estimate and its error stand on the last event row of each time. SAVE CUMEVENT gives every case the events at or before its time, so tied events get the same number whatever the case order.
- Percentile standard errors need survival to reach p - 0.05; otherwise the error is left empty. Time 0, where survival is 1, counts as the last time at which survival is still p + 0.05 or more.
- The mean and the median get 95% intervals from the normal distribution, estimate plus and minus 1.96 standard errors.
- With TREND, the overall comparisons test the trend (one degree of freedom); pairwise comparisons stay the ordinary tests, which a trend over two levels equals. TREND with fewer weights than levels is an error.
- A comparison gives the degrees of freedom of the rank of its covariance matrix, so a level without any case at risk at an event time does not break the test. An event time with at most one case at risk adds nothing to a test; with fractional weights that is a weight of 1 or less.
- **Beyond the documentation**: KM takes PLOTS as another name for PLOT.
- The plots are step curves, corners doubled so the line runs flat then upright, with an upright tick at each censored time. LOGSURV and HAZARD end where survival reaches 0. Each stratum gets its own chart.
- Compared with R's survival package: the restricted mean with a per-curve upper limit and its error agree, except where the longest time holds both an event and a censored case: there the documented error multiplies by d / (d - 1), the package does not. The package's quantile q means survival 1 - q, so its 0.75 quantile is the 25% percentile here; on a flat stretch exactly at the level the package takes the middle of the stretch, the documented rule its start.

### SURVIVAL

- The median lies inside the interval where cumulative survival first drops below 0.5, interpolated over that interval's own width.
- When survival never drops below 0.5, the median shows the start of the last interval that cases entered, with a plus sign, rather than the start of the open interval, which may lie past every observed time.
- The open last interval has no width, so its density, hazard and their errors are empty.
- When the cumulative survival reaches 0, its later standard errors are 0.
- Control values are truncated toward 0 before the range check, so -1.5 counts as -1.
- With weights, every score and sum in Gehan's statistic is weighted, so a weight of n gives the same result as n identical cases.
- CALCULATE=CONDITIONAL is the same as EXACT: exact comparisons always fit in memory. APPROXIMATE scores each case by the start of its interval; ties between an event and a censored case in one interval put the event first, as for exact times.
- MISSING=LISTWISE leaves out a case missing on any survival or control variable named on TABLES; a missing status leaves the case out of that survival variable's tables only, in both modes.
- The plots draw survival, its log and one minus survival through the ends of the closed intervals, joined by straight lines from (0, 1); hazard and density as a level across each closed interval.
- Not supported yet: WRITE=TABLES and WRITE=BOTH, which need PROCEDURE OUTPUT; each gives an error.

### Complex samples: CSPLAN, CSDESCRIPTIVES and CSTABULATE

- **The plan file** is a small JSON file of Statscraft's own (`"format": "statscraft-analysis-plan"`), not the XML plan the documentation describes, which Statscraft neither reads nor writes. It keeps the analysis weight, the simple random sampling estimator and, per stage, the label, strata, clusters, estimator and population size or inclusion probability. People and agents can read and write it directly.
- **R's survey package** was the reference for the numbers, as GNU PSPP has no complex samples procedures. Estimates, standard errors, intervals (with the design df, number of first-stage clusters minus first-stage strata, which R calls `degf`), the finite population correction of one and two stages, domain estimates and the Rao-Scott F test of independence agree. Where the manual defines a number differently, Statscraft follows the manual:
  - **Design effects of subpopulations and of variables with missing values** compare with simple random sampling over every case in the design, with the whole sample's size and estimated population (the manual's formula); R compares with sampling inside the domain or the cases with valid values. Without missing values and subpopulations both agree.
  - **The test of homogeneity** of a one-way table takes the multinomial covariance from the estimated proportions, as the manual says; R's `svygofchisq` takes it from the hypothesized equal proportions and gives a slightly different test.
  - **The likelihood ratio tests** (independence and homogeneity) are adjusted with the same design effect matrix as the Pearson test; R's `svychisq` has no such test.
  - Under SRSESTIMATOR WOR a design effect exists only when the sample is smaller than the estimated population, as the manual says; otherwise the cell is empty.
- **Domains.** A subpopulation, and under MISSING SCOPE the cases without valid analysis values, keep the whole design: cases outside count as zero, strata and clusters stay as they are. So a stratum or cluster without a case of the domain still counts in the variance and the df.
- **Missing design values.** A case with a weight of 0 or less, or with a missing weight, stratum, cluster, population size or inclusion probability leaves the design. A population size or inclusion probability given by a variable must be the same within a stratum, else the command stops; a population size below the number of units drawn stops it too. A stratum with a single unit adds nothing to the variance, as the manual says.
- **Tests with SUBPOP.** CSTABULATE always tests the whole population, and also each subpopulation when the SUBPOP variables are all first-stage strata, as the manual describes; otherwise a warning says the tests are for the whole population only. The F tests use the design df of the whole sample also inside a subpopulation.
- **Layout.** CSDESCRIPTIVES shows one Univariate Statistics table (means, then sums) and one Ratios table, with SUBPOP a block per subpopulation and then a Total block for the whole population (the manual's t tests cover both); a two-way table of CSTABULATE has a row per cell, row total, column total and grand total, with a column group per CELLS keyword. EXPECTED and RESID are table percents; ASRESID is the residual over its design-based standard error. Odds ratios, relative risks and risk differences show their standard errors and intervals in one table, with the interval at the CIN level (95 by default).
- **Beyond the documentation:** keywords may be shortened as elsewhere in Statscraft, although the manual asks for them in full. The equals signs of the syntax charts are required.
- Not supported yet: CSPLAN SAMPLE (sample designs and their METHOD, MOS, SIZE, RATE and STAGEVARS), the UNEQUAL_WOR estimator, which needs joint inclusion probabilities, and so JOINTPROB; reading XML plan files.

### Complex samples models: CSGLM and CSLOGISTIC

- **References.** R's survey package (`svyglm` with the gaussian and quasibinomial families, and `svytotal` for the design covariance of score totals) gave the estimates and their design-based covariance matrices. R has no complex samples multinomial logistic model installed here, so that model was fitted in R by solving the weighted score equations with Newton-Raphson and forming the sandwich from `svytotal`'s covariance of the score totals; the same code reproduces `svyglm` for the binary model. Estimates, standard errors and covariances agree to about 1e-8. Where the manual's definitions differ from R's, Statscraft follows the manual and the tests convert:
  - **Degrees of freedom.** Every t test, interval and F test uses the design df (first-stage clusters minus first-stage strata, or CRITERIA DF). R's `summary.svyglm` uses the design df minus the number of parameters plus one.
  - **The Wald F** is the manual's: the Wald chi-square times (df - r + 1) / (r df), on r and df - r + 1 degrees of freedom. The expected values were computed in R from the package's estimates and covariance with this formula, not taken from its own tests (`regTermTest`), which scale and refer the statistic differently.
  - **Design effects and the adjusted tests** compare with the manual's simple random sampling covariance: the sandwich built from fpc N / (n - 1) times the weighted sum of the squared deviations of the scores from their weighted mean (0 at a converged fit) over every case in the design, with the sample size and estimated population of the whole design (fpc = 1 under SRSESTIMATOR WR). The expected values were computed in R with this formula. The adjusted F divides the chi-square from the simple random sampling covariance by the trace of the generalized design effect matrix and refers it to F on d and d times df. When that covariance is not defined (without replacement, a sample as large as the estimated population), design effects and adjusted tests of several rows stay empty with a warning; a single row's adjusted test is its Wald test. The Sample Design Information shows the design's own df even when CRITERIA DF replaces it in the tests.
- **Cases left out.** Cases outside the DOMAIN and cases with a missing dependent, factor or covariate stay in the design with zero scores, so strata and clusters stay as they are and the df is that of the whole design; R's `subset` of a design gives the same variances.
- **Pseudo R squares** use the summed weights of the analysis cases as N and the weighted log likelihoods of the model and of its start (the intercept-only fit, or equal probabilities without intercept). The Model Summary of CSGLM shows R Square only: the syntax reference mentions a root mean square error, but the Algorithms manual gives no formula for it, so it is not shown yet.
- **Defaults.** Without STATISTICS, CSGLM shows the Parameter Estimates with estimate, standard error, interval and t test, as its basic output is described; CSLOGISTIC shows no parameter table. KMATRIX alone tests the intercept's row of the estimable functions (with factors, the intercept at the last level of each factor). The default model lists the factors before the covariates.
- **Layout.** CUSTOM and EMMEANS COMPARE show one table each with a row per contrast (for CSLOGISTIC, a row per contrast and logit unless the row uses ALL) and a last Overall Test row; the Sig. of the single rows is adjusted as PADJUST says. ODDSRATIOS shows a row per comparison and logit, with the values the other variables are held at in the caption. An odds ratio that goes through a cell without cases is not estimable and is left empty. The classification table sums the weights. Saved variables cover the cases in the analysis; the others stay missing. PREDVAL keeps the format and value labels of the dependent variable; names that are taken get a suffix.
- **Beyond the documentation:** keywords may be shortened as elsewhere in Statscraft, although the manual asks for them in full.
- Not supported yet: JOINTPROB and the UNEQUAL_WOR estimator, OUTFILE, PRINT GEF, and the modified estimates of marginal means that skip empty cells (a mean that is not estimable is left empty). The check for separation looks for complete separation only.

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
- /DOC and /PPT write the current Word and PowerPoint formats (.docx, .pptx), whatever the file is called, on every system and without the office programs installed. A table becomes a Word table or a slide with a table; a bar, pie, line or area chart becomes an editable chart with its workbook inside, in the document's theme. PAGESIZE, SLIDESIZE, the margins, WIDETABLES and PAGEBREAKS are accepted and not applied.
- /XLSX puts a chart beside its data table as an editable chart. A statistical plot (histogram, boxplot, Q-Q plot, dendrogram and the rest) has no editable form: /XLSX puts its picture beside the data table, and /DOC and /PPT hold the picture alone.
- HTML holds each chart inside the page as a picture (a data URL), so the page stays one file; IMAGEFORMAT picks JPG (the default), BMP or PNG. TEXT writes each chart as a picture file in the text's folder, named after the text file with a number from 0, and puts the chart's title and the file name where the chart stands. CSV shows a chart as its data table.
- The image subcommands /PNG, /JPG, /BMP and /TIF write one file per chart, IMAGEROOT followed by 0, 1, 2 and so on, when no document subcommand is given; each one given writes its own set. With a document subcommand IMAGEROOT is not used, and the image subcommand of the format in use sets the size and colors of the pictures in that document. Pictures are on white paper; PERCENTSIZE=100 is the 1280 pixel wide PNG that `statscraft run --charts` writes. DEPTH=COLORS16 and COLORS256 keep full color, and COMPRESS=YES writes an uncompressed BMP.
- /EMF and /EPS, and IMAGEFORMAT=EMF or EPS on TEXT, are accepted, but the pictures are not written yet: a warning, and the charts go out as their data tables. A chart kind that is not drawn yet (a stem-and-leaf display, a plot in three dimensions) also stays a data table and gets no picture file.
- Documents are named Document1, Document2 and so on.
- In the JSON of a run, each command names the document its tables went to (`commands[].document`), and `describe` lists the open documents and the designated one, with the file of each and whether it holds unsaved output.

### OUTPUT SAVE and OUTPUT OPEN

- Output documents are saved as Statscraft output files (`.scout`, JSON), not in the other program's output format, which Statscraft neither writes nor reads. A path ending in `.spv`, `.spo` or `.spw` is written as `.scout` under the same name, with a warning, so their files are never overwritten; OUTPUT OPEN of such a path reads that `.scout` file.
- OUTPUT SAVE: TYPE, LOCK and STOREMODEL are accepted and change nothing. /PASSPROTECT with a password is an error and nothing is saved; PASSWORD=NONE is accepted.
- OUTPUT OPEN: LOCK and PASSWORD are accepted and change nothing. A file that is missing, is no Statscraft output file or comes from a newer Statscraft gives a warning, and the tables keep going to the document they went to.
- Each command's item keeps the syntax that ran and its warnings beside its tables and charts. Deleting tables in an output window deletes them from the document too.
- A file session of the command line keeps no output documents between runs; save in the same run, or use the window session.
- The window shows each output document in an output window of its own. The documents decide what OUTPUT EXPORT writes, including tables deleted from the view.
- The window session keeps its documents from run to run; a command-line run starts without any.
- OUTPUT ACTIVATE of a name no document has, and OUTPUT CLOSE of one, give a warning, so the run goes on.
- OUTPUT EXPORT /XLSX puts the tables one below the other on one sheet (SHEET, or Tables), a blank row between. MODIFYSHEET leaves one blank column (LASTCOLUMN) or row (LASTROW) before the new tables.
- CREATESHEET and MODIFYSHEET add only to a workbook that OUTPUT EXPORT wrote in the same session and that has not changed since, because the whole workbook is written again and what Statscraft did not write (formulas, charts, formatting) would be lost. Other workbooks are refused with an error.
- ENCODING=LOCALE writes UTF-8.
- In TEXT, the columns are as wide as their widest cell (COLUMNWIDTH=n wraps at n characters) and numbers align right.

### BEGIN CHART

- **Beyond the documentation:** BEGIN CHART and END CHART are a Statscraft extension, not a command of the language. The JSON lines between them are a chart that joins the designated output document like a procedure's chart. See [BEGIN CHART](commands/begin-chart.md) and [Charts as input](command-line.md#charts-as-input).

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
- A matrix made from sizes (IDENT, MAKE, MDIAG, MAGIC, UNIFORM, NORMAL, KRONEKER, SSCP and the product `*`) holds at most 100 million elements. DO IF and LOOP blocks nest at most 100 levels, and a LOOP index that would pass the largest whole number ends the loop.

## Not supported yet

- **Commands.** The commands of the language that Statscraft recognizes but does not run are listed at the end of [the command index](commands/README.md#not-supported-yet). That list is generated from the program, so it is always current. A script that uses one stops at that line, and nothing is changed.
- **Subcommands.** Every command page lists what it accepts. Notably:
  - ONEWAY has no POLYNOMIAL, RANGES, EFFECTS or WALLER.
  - NONPAR CORR has no MATRIX or CI. PARTIAL CORR has no MATRIX.
  - NPAR TESTS has no exact significance for K-S, the median, Kruskal-Wallis, Jonckheere-Terpstra, Friedman, Kendall, Cochran and marginal homogeneity tests.
  - REGRESSION has no METHOD=TEST, SELECT, MATRIX or OUTFILE.
  - RELIABILITY has no MODEL=OMEGA or MATRIX.
  - LOGISTIC REGRESSION has no SELECT or OUTFILE.
  - COXREG has no OUTFILE.
  - NOMREG has no TEST, OUTFILE, or PRINT CELLPROB, HISTORY and ASSOCIATION.
  - LOGLINEAR, the older loglinear command with contrasts, is not supported; GENLOG and HILOGLINEAR cover its models.
  - GENLIN has no OUTFILE, TWEEDIE, INITIAL, CITYPE=PROFILE or PRINT GEF, HISTORY, LAGRANGE and LMATRIX, and with REPEATED no multinomial distribution and no generalized score tests.
  - CSPLAN has no SAMPLE designs; CSDESCRIPTIVES, CSTABULATE, CSGLM and CSLOGISTIC have no JOINTPROB and no UNEQUAL_WOR estimator; CSGLM and CSLOGISTIC have no OUTFILE or PRINT GEF.
  - UNIANOVA and GLM have no RANDOM, TEST, ROBUST, OUTFILE, MEASURE, POSTHOC VS, CONTRAST SPECIAL, PRINT GEF and TEST(LMATRIX) or the heteroskedasticity tests, and no multivariate tests in EMMEANS COMPARE of the multivariate model. With WSFACTOR they have no EMMEANS, POSTHOC, CONTRAST, LMATRIX, MMATRIX, SAVE, PLOT, or PRINT PARAMETER, HOMOGENEITY, LOF, RSSCP and TEST(SSCP).
  - MIXED has no spatial (SP_*) or Kronecker (UN_AR1, UN_CS, UN_UN) structures on REPEATED, and no KRONECKER or SPCOORDS.
  - FACTOR has no METHOD=COVARIANCE, MATRIX=OUT or MATRIX=IN(FAC=...).
  - CLUSTER has no MATRIX=OUT.
  - MVA has no OUTFILE under SPLIT FILE.
  - DISCRIMINANT has no ROTATE=COEFF or STRUCTURE, MATRIX or OUTFILE, and no cross-validated rows in the casewise listing.
  - USE takes case numbers only with /PERMANENT.
  - ADD FILES, MATCH FILES and UPDATE take no PASSWORD.
  - GET DATA reads no ODBC or OLEDB sources.
  - SAVE TRANSLATE writes CSV, TAB, Excel 2007 workbooks and Stata files only (no SAS, dBASE, 1-2-3, SYLK, ODBC, the older binary Excel files or Stata 4 to 7 files), has no APPEND, and RENAME takes valid variable names only.
  - FILE HANDLE refuses MODE=BINARY, MULTIPUNCH, IMAGE and 360, since only text and data files are read.
- **Expression functions.** APPLYMODEL and STRAPPLYMODEL, which need MODEL HANDLE. NUMBER reads the formats DATA LIST reads (F, N, E, COMMA, DOT, DOLLAR, PCT, DATE, ADATE, EDATE and SDATE). STRING writes the number, date and time formats, but not the custom currencies CCA to CCE.
- **Transformations.** PRINT and WRITE have no binary formats (P, PK, IB, PIB, PIBHEX, RB, RBHEX, Z). DATA LIST reads only F, N, E, COMMA, DOT, DOLLAR, PCT, A, DATE, ADATE, EDATE and SDATE, and only UTF-8 files. Commands such as VALUE LABELS cannot stand inside LOOP. AUTORECODE has no templates.
- **Charts.** GRAPH (see [GRAPH](#graph) for the parts not supported yet), FREQUENCIES, CROSSTABS, EXAMINE, ONEWAY, REGRESSION, LOGISTIC REGRESSION, COXREG, UNIANOVA, GLM, DISCRIMINANT, FACTOR, CLUSTER and TREE give their charts as data (`--json` gives each chart's description and `--vega-lite` a Vega-Lite specification), and the window and `statscraft run --charts` draw them. The text output shows each chart's numbers as a table. A stem-and-leaf display and a plot in three dimensions are not drawn yet and show as their data table. Error bars (the profile plots of UNIANOVA and GLM) are drawn in the pictures, in Vega-Lite and in the office charts, where they are custom error bars whose lengths come from the chart's data table. Workbooks, Word documents and presentations hold the bar, pie and line charts (the means, scree, profile and gain charts among them) as editable charts and the statistical plots as pictures, a workbook with the data table beside each picture.
- **TREE.** CHAID and exhaustive CHAID with a nominal or ordinal dependent. Not supported yet: CRT, QUEST, a scale dependent, split-sample and cross-validation, SPLITMERGED=YES, custom costs, profits, percentile gain tables, rules, influence variables, model files and TARGETRESPONSE.
- **Data files.** Statscraft reads and writes .sav and .zsav files, portable (.por) files with IMPORT and EXPORT, and Stata (.dta) files with GET STATA and SAVE TRANSLATE /TYPE=STATA.
  - Text files are read with DATA LIST, GET DATA and GET TRANSLATE /TYPE=TAB; workbooks (.xlsx, .xlsm, .xls) with GET DATA and GET TRANSLATE; dBASE III and IV files with GET TRANSLATE. 1-2-3, Symphony, SYLK and dBASE II files are not read.
  - SAVE TRANSLATE writes CSV, tab-delimited text, .xlsx and Stata .dta files.
  - An old .sav file that does not state its code page is read as Western European (windows-1252). There is no way to choose another encoding yet.
- **Output.** Tables come out as Markdown, JSON, HTML and PDF (from the window and OUTPUT EXPORT), text and CSV (OUTPUT EXPORT) and Excel workbooks.
  - OUTPUT SAVE and OUTPUT OPEN write and read Statscraft output files (`.scout`) only.
  - OUTPUT MODIFY deletes, hides, renames, numbers, sorts and transposes items and changes cell formats and texts. Statscraft tables have no fonts, colors, styles, column widths, table looks or layers, so those keywords (and SIZE, TABLESUMMARY, PIVOT, UNGROUP, REVERTTODEFAULT and /GRAPHS) change nothing and give a warning. PARENT and CHILD conditions are not supported. Command identifiers are the command names (FREQUENCIES, T-TEST) and subtypes are table titles. Documents hold no logs, notes, models or text items, so selecting those kinds picks nothing.
  - OUTPUT EXPORT /PDF writes A4 portrait pages with 2 cm margins; there is no page setup to change the size, the margins or the page header, which shows the TITLE text. A table wider than the page shrinks to 70 percent at most and then splits its columns into parts with the row labels repeated; there are no break points or keep-together groups. The bundled font has no bold face, so titles and headings get a thin outline instead. With EMBEDFONTS=NO, characters outside Windows Latin come out as question marks.
  - OUTPUT EXPORT writes no Excel 97-2004 files. Its image subcommands write PNG, JPG, BMP and TIF pictures; EMF and EPS pictures are not written yet. The window saves a chart as PNG or SVG, and `statscraft run --charts` writes every chart of a run.
- **Packages.** The Windows installer and the macOS app are not signed yet. There are packages for x86_64 Windows and Linux and for macOS on Apple silicon and Intel, but none for Windows or Linux on ARM processors.
