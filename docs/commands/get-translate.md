<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# GET TRANSLATE

Read a tab-delimited text file, a dBASE file or an old Excel workbook.

## Syntax

```text
GET TRANSLATE FILE='path' | handle [/TYPE=TAB | DBF | XLS] [/FIELDNAMES]
              [/RANGE=A1..K14 | 'A1:K14'] [/KEEP=varlist] [/DROP=varlist]
              [/RENAME=(old = new) ...] [/MAP].
```

TYPE may be left out for a .tab, .dbf or .xls file. KEEP and DROP do not change the order of the variables. FIELDNAMES takes the names from the first row (of the range); they are made valid: other characters become \_, a reserved word gets \$ (GE\$), a repeated name a number, and a column without a name is left out. RANGE limits the rows and columns.

TAB: rows end with a line break and values with a tab. Names are VAR1, VAR2, ...

without FIELDNAMES. The first value of a column decides its type: a number gives F8.2 or wider, any other text a string of at least 8 bytes, as wide as that value; longer values are cut, and text in a numeric column is system-missing.

DBF (dBASE II, III and IV): every record, also those marked deleted, which get \* in an added string D\_R. Character and logical fields are strings, numeric and float fields numbers (F with the field's width and decimals), date fields dates (DATE11); memo fields are left out. FIELDNAMES is ignored.

XLS (Excel 95 to 2003, read as GET DATA reads it): the first sheet; names are the column letters without FIELDNAMES.

1-2-3, Symphony and SYLK files (WK, WK1, WKS, WR1, WRK, SYM, SLK) are not supported.

## Example

```text
GET TRANSLATE FILE='address.dbf' /DROP=phoneno entry /MAP.
GET TRANSLATE FILE='wave3.txt' /TYPE=TAB /FIELDNAMES.
```

`statscraft help GET TRANSLATE` prints the same text in the terminal. [All commands](README.md)
