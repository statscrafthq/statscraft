<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# UPDATE

Replace values of a primary file with the values of transaction files, by key.

## Syntax

```text
UPDATE FILE=* | 'path' | dataset [/RENAME=(old = new) ...] [/IN=var]
       /FILE=... [/RENAME=...] [/IN=var] [/FILE=...] /BY varlist
       [/MAP] [/KEEP=varlist] [/DROP=varlist].
```

The first FILE is the primary file, which may hold each key once; the others are transactions. Every file must be sorted by the keys. A transaction value replaces the value of the case with its key unless it is system-missing, user-missing or blank; transactions apply in order, case by case and file by file. A key only a transaction has adds a case, a variable only a transaction has adds a variable. Dictionaries combine as in MATCH FILES.

IN adds a variable at the end, 1 for the cases that file holds, else 0 (F1.0); KEEP and DROP cannot name it. MAP lists the variables at that point with their names in each file.

The result keeps the primary file's weight. FILE=\* cannot follow TEMPORARY.

## Example

```text
UPDATE FILE='customers.sav' /FILE='changes.sav' /IN=changed /BY id.
```

`statscraft help UPDATE` prints the same text in the terminal. [All commands](README.md)
