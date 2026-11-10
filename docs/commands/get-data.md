<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# GET DATA

Read a text data file, delimited or in fixed columns, as the active dataset.

## Syntax

```text
GET DATA /TYPE=TXT /FILE='path' [/ENCODING='name'] [/ARRANGEMENT=DELIMITED | FIXED]
         [/FIRSTCASE=n] [/DELCASE=LINE | VARIABLES n] [/FIXCASE=n] [/DELIMITERS="chars"]
         [/QUALIFIER="c"] [/IMPORTCASE=ALL | FIRST n] [/MULTIPLESPACES IGNORE=NO | YES]
         [/LEADINGSPACES IGNORE=NO | YES]
         /VARIABLES=name format [name format ...].
```

FIXED: /VARIABLES=\[/record\] name first-last format ..., columns counted from 0.

format: F, A, DATE, ADATE, SDATE, DATETIME, DOLLAR, COMMA and the other input formats, with or without a width; AUTO reads a number when every value is one, else a string as wide as the longest value; X skips the field. DELIMITERS: \\t is a tab, \\\\ a backslash; two delimiters in a row leave a value empty. Default delimiters: tab and comma. A value between qualifiers may hold delimiters and line breaks; a doubled qualifier is one. SET DECIMAL=COMMA reads a comma as the decimal mark. ENCODING as for INSERT; a byte order mark wins.

Other types (XLS, XLSX, ODBC) are not supported yet.

## Example

```text
GET DATA /TYPE=TXT /FILE='wave2.csv' /DELIMITERS="," /QUALIFIER='"' /FIRSTCASE=2
  /VARIABLES=id F5 name A20 born SDATE10.
```

`statscraft help GET DATA` prints the same text in the terminal. [All commands](README.md)
