<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# PRINT EJECT

Like PRINT, starting a new page each time it runs.

## Syntax

```text
PRINT EJECT [OUTFILE='path'] [ENCODING='name'] [RECORDS=n] [TABLE | NOTABLE] /item ....
```

As PRINT. In the output each new page is a new table; in a file its first line starts with 1 in place of the blank. Usually inside DO IF, for headings.

## Example

```text
DO IF $CASENUM = 1.
PRINT EJECT /'Name' 1 'Score' 12.
END IF.
```

`statscraft help PRINT EJECT` prints the same text in the terminal. [All commands](README.md)
