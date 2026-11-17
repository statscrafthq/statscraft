<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# AGGREGATE

Aggregate groups of cases into new variables or a new dataset.

## Syntax

```text
AGGREGATE [/OUTFILE=* | 'path' | dataset [MODE=ADDVARIABLES | REPLACE]] [/MISSING=COLUMNWISE] [/PRESORTED]
          [/BREAK=varlist [(A | D)] ...]
          /target ['label'] [target ...] = function(varlist) [/target ...].
```

OUTFILE=\* replaces the active dataset with one case per break group (break variables, then targets); with MODE=ADDVARIABLES it keeps the cases and adds the targets to each of them.

OUTFILE='path' writes the aggregated dataset to a file and leaves the active dataset alone; OUTFILE=dataset puts it into a dataset that DATASET DECLARE or DATASET NAME defined.

Without OUTFILE the targets are added to every case, as with OUTFILE=\* MODE=ADDVARIABLES.

functions: SUM MEAN SD MEDIAN MIN MAX FIRST LAST N NU NMISS NUMISS; N and NU also without (varlist).

Missing values are skipped; with MISSING=COLUMNWISE a group with one missing value gets a missing result (N, NU, NMISS and NUMISS still count). PRESORTED keeps the cases in file order and makes a group of each run of adjacent cases with the same break values. With a weight variable SUM, MEAN, SD, MEDIAN, N and NMISS are weighted.

## Example

```text
AGGREGATE /OUTFILE=* MODE=ADDVARIABLES /BREAK=region /region_mean = MEAN(income) /region_n = N.
```

`statscraft help AGGREGATE` prints the same text in the terminal. [All commands](README.md)
