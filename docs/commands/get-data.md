<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# GET DATA

Read a text data file or a sheet of a workbook as the active dataset.

## Syntax

```text
GET DATA /TYPE=TXT /FILE='path' [/ENCODING='name'] [/ARRANGEMENT=DELIMITED | FIXED]
         [/FIRSTCASE=n] [/DELCASE=LINE | VARIABLES n] [/FIXCASE=n] [/DELIMITERS="chars"]
         [/QUALIFIER="c"] [/IMPORTCASE=ALL | FIRST n] [/MULTIPLESPACES IGNORE=NO | YES]
         [/LEADINGSPACES IGNORE=NO | YES]
         /VARIABLES=name format [name format ...].
GET DATA /TYPE=XLSX | XLSM | XLS /FILE='path' [/SHEET=INDEX n | NAME 'name']
         [/CELLRANGE=FULL | RANGE 'A1:K14'] [/READNAMES=ON | OFF] [/ASSUMEDSTRWIDTH=n]
         [/HIDDEN IGNORE=NO | YES] [/LEADINGSPACES IGNORE=NO | YES]
         [/TRAILINGSPACES IGNORE=NO | YES] [/DATATYPEMIN PERCENTAGE=n].
```

Text files. FIXED: /VARIABLES=\[/record\] name first-last format ..., columns counted from 0.

format: F, A, DATE, ADATE, SDATE, DATETIME, DOLLAR, COMMA and the other input formats, with or without a width; AUTO reads a number when every value is one, else a string as wide as the longest value; X skips the field. DELIMITERS: \\t is a tab, \\\\ a backslash; two delimiters in a row leave a value empty. Default delimiters: tab and comma. A value between qualifiers may hold delimiters and line breaks; a doubled qualifier is one. SET DECIMAL=COMMA reads a comma as the decimal mark. ENCODING as for INSERT; a byte order mark wins.

Workbooks (XLSX and XLSM from Excel 2007 on, XLS from Excel 95 to 2003). The first sheet by default; FULL reads from the first to the last cell with a value. READNAMES=ON (default) takes the first row as names, made valid: other characters become \_, a name that does not start with a letter gets a V in front, a repeated one \_2; OFF and an empty heading give V1, V2, ... by the column in the range. A column is numeric (F, 0 or 2 decimals) when all its values are numbers (TRUE and FALSE are 1 and 0), a date when all are dates or times (DATE11, TIME8 or DATETIME20), else a string as wide as its longest value. DATATYPEMIN PERCENTAGE=n (over 50) lets the kind of n percent of the values decide; the others become system-missing.

Empty rows and empty columns without a name are left out; error cells are empty. HIDDEN IGNORE=YES leaves out hidden rows and columns (XLSX only). ASSUMEDSTRWIDTH cuts longer strings (XLS: 255 by default). LEADINGSPACES IGNORE=YES removes leading blanks of strings; TRAILINGSPACES IGNORE=YES leaves trailing blanks out of the width.

ODBC and OLEDB are not supported yet.

## Example

```text
GET DATA /TYPE=TXT /FILE='wave2.csv' /DELIMITERS="," /QUALIFIER='"' /FIRSTCASE=2
  /VARIABLES=id F5 name A20 born SDATE10.
GET DATA /TYPE=XLSX /FILE='wave3.xlsx' /SHEET=NAME 'Answers' /CELLRANGE=RANGE 'A1:F200'.
```

`statscraft help GET DATA` prints the same text in the terminal. [All commands](README.md)
