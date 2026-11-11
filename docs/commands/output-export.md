<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# OUTPUT EXPORT

Write the tables of an output document to an Excel workbook, web page, text or CSV file.

## Syntax

```text
OUTPUT EXPORT [NAME=* | name] [/CONTENTS [EXPORT=ALL | VISIBLE] [LAYERS=...] [MODELVIEWS=...]]
   [/XLSX DOCUMENTFILE='path' [OPERATION=CREATEFILE | CREATESHEET | MODIFYSHEET] [SHEET='name']
          [LOCATION=LASTCOLUMN | LASTROW | STARTCELL('B2')] [NOTESCAPTIONS=YES | NO]]
   [/HTML DOCUMENTFILE='path' [NOTESCAPTIONS=YES | NO] [STYLING=YES | NO] [IMAGEFORMAT=...]
          [INTERACTIVELAYERS=NO | YES]]
   [/TEXT DOCUMENTFILE='path' [ENCODING=LOCALE | UTF8 | UTF16] [NOTESCAPTIONS=YES | NO]
          [COLUMNWIDTH=AUTOFIT | n] [SEPARATOR=SPACE | TAB] [ROWBORDER='-'] [COLUMNBORDER='|']]
   [/CSV DOCUMENTFILE='path' [NOTESCAPTIONS=YES | NO]] [/BMP | /EMF | /EPS | /JPG | /PNG | /TIF ...].
```

Writes the tables that the designated document (NAME=\*) or the named one holds so far, in order; an existing file is replaced. Subcommands and keywords are spelled in full. Every table is visible and has one layer, so CONTENTS changes nothing. NOTESCAPTIONS=NO leaves out the notes under the tables. XLSX puts the tables one below the other on one sheet, named by SHEET or Tables, laid out as the Excel export of the window. CREATESHEET and MODIFYSHEET add to a workbook that OUTPUT EXPORT wrote earlier in the session and that has not changed since; another workbook is refused, so its formulas and formatting cannot be lost. CREATESHEET replaces or adds the sheet; MODIFYSHEET places the tables after the last column or row of the sheet, one column or row apart, or from STARTCELL. HTML writes one web page; STYLING=NO leaves out the style sheet. TEXT lines the columns up between the borders, or with SEPARATOR=TAB separates the cells by tabs; COLUMNWIDTH=n wraps longer cells; LOCALE and UTF8 write UTF-8, UTF16 writes UTF-16 with a byte order mark. CSV, a Statscraft addition, writes each table as comma-separated values after its title, with a blank line between tables.

Statscraft draws no charts, so the image subcommands change nothing. /DOC, /PDF, /PPT, /XLS and /XLSM are refused.

## Example

```text
OUTPUT EXPORT /XLSX DOCUMENTFILE='results.xlsx' SHEET='Region 1'.
```

`statscraft help OUTPUT EXPORT` prints the same text in the terminal. [All commands](README.md)
