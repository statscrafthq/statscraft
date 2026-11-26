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

PGT PLT FGT FLT CGT CLT (varlist, value) and PIN POUT FIN FOUT CIN COUT (varlist, low, high): the percentage, fraction or count of the valid values above, below, inside (bounds included) or outside (bounds excluded); bounds the wrong way round are swapped, strings compare as text.

Missing values are skipped; a period after the function name (SUM.(x), PGT.(x, 30), N.(x)) counts user-missing values as valid, so NMISS.(x) and NUMISS.(x) count the system-missing ones alone; FIRST., LAST., MIN. and MAX. may pick a user-missing value, which keeps its status in the target. On N or NU without a variable the period changes nothing, and a period that ends the line ends the command. A weight variable's own user-missing values still weigh nothing.

With MISSING=COLUMNWISE a group with one missing value gets a missing result (N, NU, NMISS and NUMISS still count). PRESORTED keeps the cases in file order and makes a group of each run of adjacent cases with the same break values. With a weight variable SUM, MEAN, SD, MEDIAN, N and NMISS are weighted.

## Example

```text
GET FILE='survey.sav'.
AGGREGATE /OUTFILE=* MODE=ADDVARIABLES /BREAK=region
  /region_spend = MEAN(spend) /region_n = N.
COMPUTE above_region = spend > region_spend.
GET FILE='survey.sav'.
AGGREGATE /OUTFILE=* /BREAK=region
  /respondents = N /mean_spend = MEAN(spend) /share_recommend = MEAN(recommend).
LIST.
```

The example reads the sample data file [survey.sav](../sample/survey.sav), an invented customer survey; save it in the folder you run the syntax from.

## Example output

### List

| region | respondents | mean_spend | share_recommend |
| --- | --- | --- | --- |
| 1 | 75 | 36.14 | .73 |
| 2 | 62 | 40.50 | .63 |
| 3 | 81 | 38.47 | .54 |
| 4 | 82 | 39.19 | .60 |

`statscraft help AGGREGATE` prints the same text in the terminal. [All commands](README.md)
