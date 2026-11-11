<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# MATCH FILES

Combine the variables of several data files case by case or by key.

## Syntax

```text
MATCH FILES /FILE=* | 'path' | dataset [/RENAME=(old = new) ...] [/IN=var]
            [/TABLE='path' | dataset | * [/RENAME=...] [/IN=var] ...]
            /FILE=... [/BY varlist] [/FIRST=var] [/LAST=var]
            [/MAP] [/KEEP=varlist] [/DROP=varlist].
```

A dataset name (DATASET NAME), quoted or not, is that open dataset and comes before a file of the same name.

With BY, cases with equal keys are joined; every file must be sorted by the keys (use SORT CASES). Without BY the n-th cases of the files are joined. A TABLE adds its variables to the cases whose key it holds and adds no cases; it may hold each key once. A variable in several files takes its value from the first file that has the case.

RENAME and IN follow their FILE or TABLE; BY, FIRST, LAST, MAP, KEEP and DROP follow every FILE and TABLE. IN (with BY) adds a variable that is 1 for the cases that file or table has, else 0. FIRST and LAST (with BY) add one that is 1 for the first or the last case of each key. These are F1.0 and go at the end, IN first; KEEP and DROP cannot name them. MAP lists the variables of the result at that point with their names in each file.

The result has the documents of every file and keeps the weight when every weighted file uses the same variable and KEEP or DROP leave it; otherwise a warning says so.

## Example

```text
MATCH FILES /FILE=* /TABLE='regions.sav' /BY region.
```

`statscraft help MATCH FILES` prints the same text in the terminal. [All commands](README.md)
