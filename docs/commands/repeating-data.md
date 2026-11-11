<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# REPEATING DATA

In an input program or FILE TYPE: one case per repeating group of a record.

## Syntax

```text
REPEATING DATA [FILE='path' | handle] [/ENCODING='UTF-8'] /STARTS=column[-column]
    /OCCURS=n [/LENGTH=n] [/CONTINUED[=column[-column]]] [/ID=columns=var]
    [/TABLE | /NOTABLE] /DATA=variables as on DATA LIST.
```

STARTS, OCCURS, LENGTH and the end column on STARTS take a number or a variable.

Reads OCCURS groups from the record the last DATA LIST read, starting in column STARTS; the columns on DATA count from the start of a group. A group is LENGTH columns, by default up to the last variable or skip. Each group gives a case with the values of the other variables spread to it. CONTINUED: groups that do not fit before the end column (default 80, or the longest record) go on in the next records, from the column given (default 1); ID checks that an identifier on those records matches.

OCCURS missing or below 1 gives no case.

## Example

```text
DATA LIST /seqnum 2-4 numveh 9-10.
REPEATING DATA STARTS=12 /OCCURS=numveh
 /DATA=make 1-8 (A) model 9 (A) numcyl 10.
```

`statscraft help REPEATING DATA` prints the same text in the terminal. [All commands](README.md)
