<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# DISPLAY

Show dictionary information of the active dataset.

## Syntax

```text
DISPLAY [SORTED] [NAMES | INDEX | VARIABLES | LABELS | DICTIONARY | ATTRIBUTES | @ATTRIBUTES
         | SCRATCH | VECTOR | MACROS | DOCUMENTS | FILE LABEL] [/VARIABLES=varlist].
```

NAMES (the default): names. INDEX: names and positions. VARIABLES: also formats and missing values. LABELS: positions and labels. DICTIONARY: everything, value labels included.

ATTRIBUTES: custom attributes except those starting with @ or \$@; @ATTRIBUTES all of them.

MACROS: the names of the macros defined in this run and the runs of the session before, sorted. SORTED sorts by name; /VARIABLES limits the list.

## Example

```text
DISPLAY SORTED DICTIONARY /VARIABLES=age TO q5.
```

`statscraft help DISPLAY` prints the same text in the terminal. [All commands](README.md)
