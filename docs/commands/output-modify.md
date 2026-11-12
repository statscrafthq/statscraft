<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# OUTPUT MODIFY

Delete, hide, rename, sort, transpose or reformat items of an output document.

## Syntax

```text
OUTPUT MODIFY [NAME=* | ALL | name] [/REPORT PRINTREPORT=NO | YES]
   /SELECT kinds | ALL [EXCEPT (kinds)]
   [/IF [COMMANDS=['name' 'name(n)' 'name(LAST)' LAST]] [SUBTYPES=['title' ...]]
        [LABELS=[EXACT('t') BEGINS('t') ENDS('t') CONTAINS('t')]] [INSTANCES=[n ... LAST]]
        [VISIBLEONLY=NO | YES]]
   [/DELETEOBJECT DELETE=NO | YES]
   [/INDEXING [INDEXTYPE=NUMBER | LETTERLOW | LETTERCAP | ROMANLOW | ROMANCAP] [INDEXSTART=n | 'a']]
   [/OBJECTPROPERTIES [VISIBLE=ASIS | YES | NO] [OUTLINELABEL='text'] [SIZE=...]]
   [/TABLE [TABLETITLE='text'] [TRANSPOSE=NO | YES] [SORT=COLLABEL('label') | COLPOSITION(n)]
        [SORTDIRECTION=ASCENDING | DESCENDING] [TLOOK=...] [SELECTEDLAYER=[...]] [TABLESUMMARY='text'] [PIVOT=[...]]]
   [/TABLECELLS SELECT=['label' POSITION(n) BODY HEADERS TITLE CAPTION FOOTNOTES COUNT MEAN ...]
        [SELECTDIMENSION=BOTH | COLUMNS | ROWS] [SELECTCONDITION=[ALL 'x<0.05' TOP(n) BOTTOM(n)]]
        [APPLYTO=CELL | COLUMN | ROW | COLUMNDATA | ROWDATA | COLUMNHEADER | ROWHEADER]
        [FORMAT=F8.2] [REPLACE='text' | number] [HIDE=NO | YES] [style keywords]]
   [/GRAPHS ...] [/TEXTS [TEXT='text'] ...]
   [/SELECT ... another block].
```

Works on the items the designated document (or NAME, or ALL documents) holds when it runs.

Each block begins with /SELECT and runs on what the blocks before it left. Kinds: TABLES, CHARTS, HEADINGS and OUTLINEHEADERS (the heading over each command's items), PAGETITLE (the output of TITLE and SUBTITLE) and UNKNOWN; LOGS, TEXTS, WARNINGS, NOTES, MODELS, TREES and OTHER pick nothing, as Statscraft documents have no such items. /IF keeps the items that meet every condition: COMMANDS names commands as written in the outline, such as 'Means', with (n) for the nth run or (LAST); LAST alone is the document's last command. SUBTYPES match table titles, any case. LABELS test the outline text, case counts. INSTANCES pick the nth item within each command. DELETE=YES removes the items; a heading goes with its command's items.

VISIBLE=NO hides items: the window and OUTPUT EXPORT with /CONTENTS EXPORT=VISIBLE leave them out, the document and its file keep them. OUTLINELABEL, TABLETITLE and TEXT take )LABEL (the text so far), )INDEX (from INDEXING, else 1, 2, 3), )DATE, )ADATE, )SDATE, )EDATE, )TIME and )ETIME. SORT orders the rows by a column; empty cells go last, rows labelled Total stay, and a table with nested row labels stays as it is. TABLECELLS picks rows or columns by label (any header level) or position, keeps the cells that meet every comparison on x (ABS(x), SYSMIS, numbers or quoted text) and then TOP or BOTTOM, and changes what APPLYTO names: FORMAT changes how numbers show, REPLACE puts in a value, HIDE takes the picked rows or columns out.

Keywords may be shortened to three letters. Fonts, colors, styles, sizes, column widths, table looks, layers, PIVOT, UNGROUP and chart templates change nothing and say so in a warning. PARENT and CHILD conditions are not supported. PRINTREPORT=YES adds a table of what changed.

## Example

```text
OUTPUT MODIFY /SELECT TABLES /IF COMMANDS=['Means'] /TABLE SORT=COLLABEL('Mean')
  SORTDIRECTION=DESCENDING /TABLECELLS SELECT=['Mean'] SELECTDIMENSION=COLUMNS FORMAT=F8.1.
```

`statscraft help OUTPUT MODIFY` prints the same text in the terminal. [All commands](README.md)
