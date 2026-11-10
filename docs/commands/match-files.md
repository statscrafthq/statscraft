<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# MATCH FILES

Combine the variables of several data files case by case or by key.

## Syntax

```text
MATCH FILES /FILE=* | 'path' | dataset [/RENAME=(old = new) ...] [/TABLE='path' | dataset ...]
            /FILE=... [/BY varlist] [/KEEP=varlist] [/DROP=varlist].
```

A dataset name (DATASET NAME), quoted or not, is that open dataset and comes before a file of the same name.

With BY, cases with equal keys are joined; every file must be sorted by the keys (use SORT CASES). Without BY the n-th cases of the files are joined. A TABLE adds its variables to the cases whose key it holds and adds no cases; it may hold each key once. A variable in several files takes its value from the first file that has the case. The result has no weight variable.

## Example

```text
MATCH FILES /FILE=* /TABLE='regions.sav' /BY region.
```

`statscraft help MATCH FILES` prints the same text in the terminal. [All commands](README.md)
