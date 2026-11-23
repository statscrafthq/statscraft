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
GET FILE='survey.sav'.
SAVE OUTFILE='survey_copy.sav'.
SAVE OUTFILE='satisfaction.sav' /KEEP=id region satisfaction /RENAME=(satisfaction = sat) /MAP.
```

The example reads the sample data file [survey.sav](../sample/survey.sav), an invented customer survey; save it in the folder you run the syntax from.

## Example output

### Variables in the file

| Name in the file | Variable |
| --- | --- |
| id | id |
| region | region |
| sat | satisfaction |

`statscraft help SAVE` prints the same text in the terminal. [All commands](README.md)
