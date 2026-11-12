<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# EXPORT

Write the active dataset to a portable (.por) file.

## Syntax

```text
EXPORT OUTFILE='path' | handle [/TYPE=COMM | TAPE] [/UNSELECTED=RETAIN | DELETE]
       [/KEEP=varlist] [/DROP=varlist] [/RENAME=(old = new) ...] [/MAP] [/DIGITS=n].
```

KEEP, DROP and RENAME shape the file only, in the order written; KEEP sets the order of the variables. Each MAP shows the variables as the subcommands before it leave them.

UNSELECTED=DELETE leaves out the cases FILTER leaves out. DIGITS rounds values that are not whole to n significant digits; without it every value is written exactly.

The file holds names (upper case, at most 8 bytes: longer ones are shortened and made unique, as MYLONGRO and MYLONG\_2), variable and value labels, missing values, formats, the weight variable and documents. Strings hold at most 255 bytes; text is UTF-8.

TYPE is accepted and changes nothing; control characters in text are written as 0.

## Example

```text
EXPORT OUTFILE='wave2.por' /RENAME=(v1 TO v3 = id sex age) /MAP.
```

`statscraft help EXPORT` prints the same text in the terminal. [All commands](README.md)
