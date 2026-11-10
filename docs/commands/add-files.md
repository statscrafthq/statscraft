<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# ADD FILES

Append the cases of several data files, matching variables by name.

## Syntax

```text
ADD FILES /FILE=* | 'path' | dataset [/RENAME=(old = new) ...] /FILE=... [/BY varlist]
          [/KEEP=varlist] [/DROP=varlist].
```

FILE=\* is the active dataset; a dataset name (DATASET NAME), quoted or not, is that open dataset and comes before a file of the same name. A variable missing from a file is blank in its cases.

The first file that has a variable gives its format and measurement level; a later file fills in a label, value labels or missing values the earlier ones lack. A string variable takes its widest width; a variable that is numeric in one file and a string in another is an error. BY interleaves files sorted by the key variables. The result stays weighted only when every file is weighted by the same variable.

## Example

```text
ADD FILES /FILE=* /FILE='wave2.sav' /RENAME=(q1_new = q1) /DROP=tmp.
```

`statscraft help ADD FILES` prints the same text in the terminal. [All commands](README.md)
