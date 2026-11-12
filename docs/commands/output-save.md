<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# OUTPUT SAVE

Save an output document as a Statscraft output file (.scout).

## Syntax

```text
OUTPUT SAVE [NAME=* | name] OUTFILE='path' [TYPE=SPV] [LOCK=NO | YES] [STOREMODEL=YES | NO]
   [/PASSPROTECT PASSWORD=NONE [ENCRYPTEDPW=NO | YES]].
```

Writes the designated document (NAME=\*), or the named one, with every command's syntax, warnings, tables and charts, as JSON that File \> Open, OUTPUT OPEN and any JSON tool read.

A file that exists is replaced; the document stays open and keeps getting tables. An unknown NAME is an error. A path ending in .spv, .spo or .spw (another program's output files) is written with .scout in its place and a warning, so such a file is never overwritten. TYPE, LOCK and STOREMODEL change nothing. A password on /PASSPROTECT is an error and nothing is saved: Statscraft output files are never encrypted.

## Example

```text
OUTPUT SAVE OUTFILE='wave2-results.scout'.
```

`statscraft help OUTPUT SAVE` prints the same text in the terminal. [All commands](README.md)
