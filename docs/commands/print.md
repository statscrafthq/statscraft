<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# PRINT

Show the values of variables for every case, as lines of text.

## Syntax

```text
PRINT [OUTFILE='path'] [ENCODING='name'] [RECORDS=n] [TABLE | NOTABLE]
      /[line] item [item ...] [/[line] ...].
```

item: varlist, varlist first-last \[(type\[,decimals\])\], varlist (format list), 'text' \[column\], or \* to end the variables a format applies to. Format list: formats such as F8.2, n(...) repeats, Tn goes to column n, nX skips n columns.

A transformation: the lines appear when the data are next read, case by case, one line per slash. A variable in its own print format is followed by a blank; one with a given format or columns is not. System-missing shows as a period. OUTFILE writes the lines to a file, each starting with a blank (the carriage control), replaced on each data pass.

TABLE shows where each variable goes. Formats P, PK, IB, PIB, PIBHEX, RB, RBHEX and Z are not supported yet.

## Example

```text
PRINT /id name (A10) score (F6.2).
```

`statscraft help PRINT` prints the same text in the terminal. [All commands](README.md)
