<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# STAR JOIN

Add the fields of lookup files to every case of a data file, by keys, as an SQL left join.

## Syntax

```text
STAR JOIN /SELECT * | alias.field [AS name], alias.field [AS name], ...
          /FROM 'path' [PASSWORD='text'] | dataset | * AS alias
          /JOIN 'path' [PASSWORD='text'] | dataset | * AS alias
                ON alias.key = alias.key [AND alias.key = alias.key ...] [IN=name]
          [/JOIN ...] /OUTFILE FILE='path' | dataset | *
          [/PASSPROTECT PASSWORD='text' | NONE [ENCRYPTEDPW=NO | YES]].
```

The subcommands come in this order and every keyword is written in full. A dataset name, quoted or not, is that open dataset and comes before a file of the same name.

Every case of FROM is kept, in its order; a JOIN adds its fields to the cases whose keys it holds and leaves them blank (system-missing, empty) elsewhere. The files need no sorting, and each JOIN has its own keys. ON pairs a field of FROM with one of that JOIN, numeric with numeric, string with string (widths may differ; trailing blanks do not count). The keys of a JOIN must be unique there. A system-missing key matches nothing.

IN=name adds a variable that is 1 where the JOIN had the case and 0 where not.

The result holds the SELECT fields and every key: SELECT \* is every field of every source, and a JOIN key of the same name as its FROM key appears once, from FROM. Keys not named on SELECT come first; the rest follow in SELECT order; IN variables last.

A name that two sources bring gets its alias in front (t1.name, t2.name) unless AS names it. The dictionary comes from FROM (with its weight, filter and split); a key without labels or missing values takes them from the first JOIN that has them; the documents of every file are kept; the file label is dropped. Not after TEMPORARY.

OUTFILE=\* replaces the active dataset, a dataset name (DATASET DECLARE) fills that dataset, a path writes a data file. PASSWORD on FROM and JOIN is accepted for files that are not encrypted (encrypted ones cannot be read); PASSPROTECT with a password is refused for a file, since Statscraft writes no encrypted files, and ignored otherwise.

## Example

```text
STAR JOIN /SELECT t1.id, t1.region, t2.name /FROM * AS t1
  /JOIN 'regions.sav' AS t2 ON t1.region = t2.region /OUTFILE FILE=*.
```

`statscraft help STAR JOIN` prints the same text in the terminal. [All commands](README.md)
