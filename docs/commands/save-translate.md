<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# SAVE TRANSLATE

Write the active dataset to a CSV or tab-delimited text file or an Excel workbook.

## Syntax

```text
SAVE TRANSLATE OUTFILE='path' | handle /TYPE=CSV | TAB | XLS /VERSION=12 [/REPLACE]
    [/FIELDNAMES [VALUE=NAMES | LABELS]] [/CELLS=VALUES | LABELS] [/MISSING=IGNORE | RECODE]
    [/TEXTOPTIONS [DELIMITER='c'] [QUALIFIER='c'] [DECIMAL=DOT | COMMA] [FORMAT=PLAIN | VARIABLE]]
    [/EXCELOPTIONS SHEET='name'] [/ENCODING='name' [BOM=YES | NO]] [/UNSELECTED=RETAIN | DELETE]
    [/KEEP=varlist] [/DROP=varlist] [/RENAME=(old = new) ...].
```

TYPE may be left out for a .csv or .xlsx file; TYPE=XLSX is short for XLS /VERSION=12. An existing file is overwritten only with REPLACE. FIELDNAMES writes a first row of names (VALUE=LABELS: the variable labels, else the names). CELLS=LABELS writes the value label where a value has one. MISSING=RECODE writes user-missing values as empty. UNSELECTED=DELETE leaves out the cases FILTER leaves out. KEEP, DROP and RENAME shape the file only, in the order written; KEEP does not change the order of the variables.

Text: system-missing is empty. FORMAT=PLAIN (default) writes numbers with the decimals they need, dates as mm/dd/yyyy and times as hh:mm:ss; VARIABLE writes the display formats. The decimal mark is DECIMAL's, else SET DECIMAL's. CSV separates values with a comma, or with a semicolon when the decimal mark is a comma, and puts the qualifier (default ") around a value that holds the delimiter, the qualifier (doubled inside) or a line break. TAB never qualifies.

ENCODING as for INSERT; the default is UTF-8 with a byte order mark, which BOM=NO leaves out.

Workbook: numbers keep their display format and dates and times are Excel dates; system-missing is \#NULL!. EXCELOPTIONS names the sheet (default Sheet1); more cases than a sheet holds continue on further sheets.

Other types (SAS, STATA, dBASE, ODBC, 1-2-3, SYLK) and APPEND are not supported yet.

## Example

```text
SAVE TRANSLATE OUTFILE='wave2.xlsx' /TYPE=XLS /VERSION=12 /FIELDNAMES /CELLS=LABELS /REPLACE.
SAVE TRANSLATE OUTFILE='wave2.csv' /TYPE=CSV /FIELDNAMES /TEXTOPTIONS DECIMAL=COMMA /REPLACE.
```

`statscraft help SAVE TRANSLATE` prints the same text in the terminal. [All commands](README.md)
