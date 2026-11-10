<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# APPLY DICTIONARY

Copy dictionary information from a data file or dataset to the active dataset.

## Syntax

```text
APPLY DICTIONARY FROM 'file' | dataset | * [/SOURCE VARIABLES=varlist]
                 [/TARGET VARIABLES=varlist] [/NEWVARS]
                 [/FILEINFO [ATTRIBUTES[=REPLACE | MERGE]] [DOCUMENTS[=...]] [FILELABEL]
                            [MRSETS[=...]] [VARSETS[=...]] [WEIGHT] [ALL]]
                 [/VARINFO [ALIGNMENT] [ATTRIBUTES[=...]] [FORMATS] [LEVEL] [MISSING]
                           [ROLE] [VALLABELS[=REPLACE | MERGE]] [VARLABEL] [WIDTH] [ALL]].
```

Variables match by name and type; with a different type only the label is copied.

Without VARINFO everything but attributes is copied, value labels replacing; without FILEINFO only the weight. SOURCE with one variable and TARGET copies that variable's information to every target of the same type. Empty information in the source does not overwrite the active dataset's. NEWVARS is not supported yet.

## Example

```text
APPLY DICTIONARY FROM 'template.sav' /FILEINFO VARSETS=MERGE MRSETS.
```

`statscraft help APPLY DICTIONARY` prints the same text in the terminal. [All commands](README.md)
