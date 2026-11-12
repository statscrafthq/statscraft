<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# SPLIT FILE

Run each procedure once per run of consecutive cases that share the split values.

## Syntax

```text
SPLIT FILE [LAYERED | SEPARATE] BY varlist.
SPLIT FILE OFF.
```

A group ends where a split variable changes value, so sort the cases first.

LAYERED (the default) shows the groups in one table, with the split variables as the outermost row labels; SEPARATE gives every group tables of its own. AGGREGATE ignores the split; SORT CASES turns it off. The split is kept with the dataset.

## Example

```text
SPLIT FILE SEPARATE BY region.
```

`statscraft help SPLIT FILE` prints the same text in the terminal. [All commands](README.md)
