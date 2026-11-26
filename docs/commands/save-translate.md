<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# SAVE TRANSLATE

Write the active dataset to a CSV or tab-delimited text file, an Excel workbook, a Stata file, a SAS transport file or a dBASE file.

## Syntax

```text
SAVE TRANSLATE OUTFILE='path' | handle /TYPE=CSV | TAB | XLS /VERSION=12 | STATA | SAS /VERSION=X | DB3 | DB4 [/REPLACE]
    [/FIELDNAMES [VALUE=NAMES | LABELS]] [/CELLS=VALUES | LABELS] [/MISSING=IGNORE | RECODE]
    [/TEXTOPTIONS [DELIMITER='c'] [QUALIFIER='c'] [DECIMAL=DOT | COMMA] [FORMAT=PLAIN | VARIABLE]]
    [/EXCELOPTIONS SHEET='name'] [/ENCODING='name' [BOM=YES | NO]] [/UNSELECTED=RETAIN | DELETE]
    [/KEEP=varlist] [/DROP=varlist] [/RENAME=(old = new) ...] [/MAP]
    [/VERSION=8 to 14] [/EDITION=INTERCOOLED | SE].
```

TYPE may be left out for a .csv, .xlsx or .dta file; TYPE=XLSX is short for XLS /VERSION=12. An existing file is overwritten only with REPLACE. FIELDNAMES writes a first row of names (VALUE=LABELS: the variable labels, else the names). CELLS=LABELS writes the value label where a value has one. MISSING=RECODE writes user-missing values as empty. UNSELECTED=DELETE leaves out the cases FILTER leaves out. KEEP, DROP and RENAME shape the file only, in the order written; KEEP does not change the order of the variables. Each MAP lists the names in the file next to the names in the dataset, as the subcommands before it leave them.

Text: system-missing is empty. FORMAT=PLAIN (default) writes numbers with the decimals they need, dates as mm/dd/yyyy and times as hh:mm:ss; VARIABLE writes the display formats. The decimal mark is DECIMAL's, else SET DECIMAL's. CSV separates values with a comma, or with a semicolon when the decimal mark is a comma, and puts the qualifier (default ") around a value that holds the delimiter, the qualifier (doubled inside) or a line break. TAB never qualifies.

ENCODING as for INSERT; the default is UTF-8 with a byte order mark, which BOM=NO leaves out.

Workbook: numbers keep their display format and dates and times are Excel dates; system-missing is \#NULL!. EXCELOPTIONS names the sheet (default Sheet1); more cases than a sheet holds continue on further sheets.

Stata: VERSION is the Stata release that reads the file (default 14; 8 and 9 give format 113, 10 and 11 format 114, 12 format 115, 13 format 117, 14 and later format 118).

INTERCOOLED (the default) keeps up to 2047 variables and 80 bytes of each string; SE keeps 32767 variables and 244 bytes before Stata 13, whole strings from 13 on (over 2045 bytes as strL). Names keep their case: characters other than letters, digits and \_ become \_, and names are cut to 32 bytes and made unique; before Stata 14 a name that is not ASCII becomes v and its position. Variable labels and the file label keep 80 bytes. Value labels go with numeric variables, for whole values up to 2147483620 (80 bytes each for Stata 8). Dates become %td days since 1960, dates with a time %tc milliseconds; times and other numbers are plain. User-missing values stay valid values unless MISSING=RECODE. Text is UTF-8 from Stata 14 on, else in ENCODING (default: the system's code page).

SAS: TYPE=SAS /VERSION=X writes a transport file (the layout of version 5, which SAS and other software read); the other SAS versions are not written. Names are cut to 8 bytes, with \_ for any other character than a letter or digit, and made unique; the data set is named after the file. The label is the variable label, else the name (40 bytes at most).

Dates are days since 1960-01-01 with the format DATE, dates with a time of day seconds with DATETIME, times seconds with TIME; other numbers have no format. Every missing value, user missing ones too, is the SAS missing value. Text is Windows Latin-1, 200 bytes at most; value labels are not written. ENCODING is not used.

dBASE: DB3 holds 128 fields and DB4 255, a record up to 4000 bytes; more variables are left off with a warning. A field name is the variable name in capitals with \_ for any other character than a letter or digit, V in front of a name that does not start with a letter, cut to 10 bytes and made unique. A numeric variable is a numeric field as wide as its print format (19 at most for DB3, 20 for DB4) with its decimals; a missing value, or one that does not fit, is asterisks, so widen the format with PRINT FORMATS first. Dates are date fields (a time of day is dropped); a string variable is a character field of its width, 254 bytes at most, in Windows Latin-1. MISSING=RECODE blanks user-missing values. ENCODING is not used.

## Example

```text
SAVE TRANSLATE OUTFILE='wave2.xlsx' /TYPE=XLS /VERSION=12 /FIELDNAMES /CELLS=LABELS /REPLACE.
SAVE TRANSLATE OUTFILE='wave2.csv' /TYPE=CSV /FIELDNAMES /TEXTOPTIONS DECIMAL=COMMA /REPLACE.
SAVE TRANSLATE OUTFILE='clinic.dta' /TYPE=STATA /EDITION=SE /DROP=nurse_note /MAP /REPLACE.
SAVE TRANSLATE OUTFILE='stock.dbf' /TYPE=DB4 /REPLACE.
SAVE TRANSLATE OUTFILE='stock.xpt' /TYPE=SAS /VERSION=X /REPLACE.
```

`statscraft help SAVE TRANSLATE` prints the same text in the terminal. [All commands](README.md)
