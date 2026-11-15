<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# GET STATA

Read a Stata data file (.dta) and make it the active dataset.

## Syntax

```text
GET STATA FILE='path' | handle [/ENCODING='name'].
```

Reads formats 113 to 119 (Stata 8 to 19), in either byte order. ENCODING names the code page of text in files older than Stata 14 (LOCALE and SYSTEM: the system's); without it such text is UTF-8 when it reads as UTF-8, else in the system's code page. Newer files are always UTF-8.

Names keep their case; one that differs from an earlier one only in case gets \_A, \_B, ..., one that starts with \_ gets V in front and a reserved word gets \$. Variable labels, value labels and the file label (cut to 64 bytes) come along. Every Stata missing value, . and .a to .z, is system-missing, and labels of .a to .z are dropped. %td dates become DATE11 and %tc and %tC datetimes DATETIME20; weekly, monthly, quarterly, half-yearly and yearly values stay counts since 1960 in F format. Long strings (strL) become strings of up to 32767 bytes; one holding binary data is blank.

## Example

```text
GET STATA FILE='clinic.dta'.
GET STATA FILE='panel2009.dta' /ENCODING='windows-1257'.
```

`statscraft help GET STATA` prints the same text in the terminal. [All commands](README.md)
