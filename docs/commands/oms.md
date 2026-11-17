<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# OMS

Send the tables, charts and warnings of the commands that follow to a file, or keep them out of the results.

## Syntax

```text
OMS [/SELECT kinds | ALL [EXCEPT=[kinds]]]
   [/IF [COMMANDS=['name' ...]] [SUBTYPES=['title' ...]] [LABELS=['text' ...]] [INSTANCES=[n ... LAST]]]
   [/EXCEPTIF [COMMANDS=[...]] [SUBTYPES=[...]] [LABELS=[...]]]
   /DESTINATION [FORMAT=SAV | HTML | TEXT | TABTEXT | XLSX | DOC | PDF OUTFILE='file' | dataset]
        [VIEWER=YES | NO] [NUMBERED[='name']] [IMAGES=YES | NO] [IMAGEFORMAT=PNG | JPG | BMP] [CHARTSIZE=n]
   [/COLUMNS DIMNAMES=['name' ...] | SEQUENCE=[R1 R2 ... RALL C1 ... CALL]]
   [/TAG='name'] [/NOWARN].
```

Kinds: TABLES, CHARTS, WARNINGS (the warnings of a command, as one item), TEXTS (the output of ECHO, TITLE and SUBTITLE); HEADINGS, LOGS, MODELS and TREES choose nothing, as Statscraft makes no such items (HTML and PDF show each command's heading anyway). COMMANDS take names such as 'Frequencies' or 'T-Test', any case; SUBTYPES take table titles, any case, and the tables of single variables from FREQUENCIES also answer to 'Frequencies'; LABELS take the outline text, any case. A trailing \* matches every text that starts with what comes before it. INSTANCES keep the nth of the matching items of each command. An item must meet every list of /IF, and is left out when it meets every list of /EXCEPTIF. The request lasts until OMSEND, and its file is written again after each command that adds to it. FORMAT=SAV makes a data file of the tables: string variables Command\_, Subtype\_ and Label\_, Var1, Var2, ... for the row labels, and one variable per column, named after its header (levels joined by \_); a column of numbers is numeric. A table whose number of row labels differs from the earlier tables is left out with a warning. OUTFILE may name a dataset that DATASET DECLARE made. HTML, TEXT, TABTEXT, XLSX, DOC and PDF write as OUTPUT EXPORT does. VIEWER=NO keeps the chosen items out of the results; when several requests choose an item, the latest decides. COLUMNS puts the named row labels (DIMNAMES, by the header of their column) or positions (SEQUENCE) in the columns and the rest in the rows, in the files only. OXML, SPV, XLS, the report formats, XMLWORKSPACE and OUTPUTSET are not written: a warning, and the request only lists in OMSINFO. Keywords that change nothing here, such as IMAGEMAP, give a warning unless NOWARN.

## Example

```text
OMS /SELECT TABLES /IF COMMANDS=['Descriptives'] /DESTINATION FORMAT=SAV OUTFILE='stats.sav'
  VIEWER=NO /TAG='desc'.
```

`statscraft help OMS` prints the same text in the terminal. [All commands](README.md)
