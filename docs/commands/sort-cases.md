<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# SORT CASES

Sort the cases by the values of variables.

## Syntax

```text
SORT CASES [BY] varlist [(A | D)] [varlist [(A | D)] ...].
```

A is ascending (the default), D descending; a direction covers the variables before it.

System-missing sorts lowest. Cases with equal keys keep their order.

## Example

```text
GET FILE='survey.sav'.
SORT CASES BY spend (D).
LIST id region spend /CASES=FROM 1 TO 5.
SORT CASES BY region age_group (A) age (D).
```

The example reads the sample data file [survey.sav](../sample/survey.sav), an invented customer survey; save it in the folder you run the syntax from.

## Example output

### List

| id | region | spend |
| --- | --- | --- |
| 1 | 2 | 94.52 |
| 113 | 2 | 90.56 |
| 80 | 3 | 85.18 |
| 49 | 3 | 79.74 |
| 61 | 2 | 72.36 |

`statscraft help SORT CASES` prints the same text in the terminal. [All commands](README.md)
