<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# SAVE

Write the active dataset to a data file.

## Syntax

```text
SAVE OUTFILE='path' | handle [/COMPRESSED | /UNCOMPRESSED | /ZCOMPRESSED] [/KEEP=varlist] [/DROP=varlist]
     [/RENAME=(old = new) ...] [/MAP].
```

KEEP, DROP and RENAME shape the saved file only; the active dataset stays as it is. MAP lists the variables of the file as the options before it leave them, each with its name in the active dataset; it may come as often as you like.

COMPRESSED compresses case by case, ZCOMPRESSED in zlib blocks, UNCOMPRESSED not at all; without one a file named .zsav gets zlib blocks and any other the case by case form.

## Example

```text
SAVE OUTFILE='survey_clean.sav' /COMPRESSED.
```

`statscraft help SAVE` prints the same text in the terminal. [All commands](README.md)
