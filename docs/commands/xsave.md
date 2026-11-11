<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# XSAVE

Write the cases to a data file as the next data pass reads them.

## Syntax

```text
XSAVE OUTFILE='path' | handle [/KEEP=varlist] [/DROP=varlist] [/RENAME=(old = new) ...]
      [/MAP] [/COMPRESSED | /ZCOMPRESSED] [/PERMISSIONS=READONLY | WRITEABLE].
```

A transformation: the file holds the cases as they reach XSAVE in the next procedure or EXECUTE (or the end of the run), with the variables of that moment, scratch variables left out. KEEP, DROP and RENAME as on SAVE. MAP lists the saved variables. Two XSAVE commands cannot write the same file in one pass. /UNCOMPRESSED is not supported yet.

## Example

```text
XSAVE OUTFILE='wave2.sav' /DROP=tmp1 TO tmp5.
```

`statscraft help XSAVE` prints the same text in the terminal. [All commands](README.md)
