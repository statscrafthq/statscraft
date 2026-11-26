<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# SAVE CODEPAGE

Write the active dataset to a data file whose text is in a code page.

## Syntax

```text
SAVE CODEPAGE OUTFILE='path' | handle [/ENCODING=LOCALE | 'encoding'] [/UNSELECTED=RETAIN | DELETE]
     [/KEEP=varlist] [/DROP=varlist] [/PASSPROTECT PASSWORD=NONE].
```

The file is a data file like the one SAVE writes, but every piece of text in it (variable names and labels, value labels, string values, the file label and the notes) is in the code page that ENCODING names, for programs that do not read data files in UTF-8. A code page is a number such as '1252', or a name such as 'windows-1257', 'iso-8859-1' or 'cp1252'; LOCALE, the default, is the legacy code page of the system. The file says which code page it is in. If any text has a character the code page lacks, nothing is written and the command stops with an error that names the text. The active dataset stays as it is, in UTF-8. UNSELECTED=DELETE leaves out the cases FILTER leaves out. KEEP and DROP shape the file only; the variables come in the order KEEP lists them. A file named .zsav gets zlib blocks and any other the case by case form. PASSPROTECT takes only PASSWORD=NONE: Statscraft does not encrypt files.

## Example

```text
SAVE CODEPAGE OUTFILE='orders_latin.sav' /ENCODING='1252' /DROP=internal_note.
```

`statscraft help SAVE CODEPAGE` prints the same text in the terminal. [All commands](README.md)
