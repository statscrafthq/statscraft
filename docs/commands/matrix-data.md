<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# MATRIX DATA

Read matrix materials (correlations, means, counts) as a matrix data file.

## Syntax

```text
MATRIX DATA VARIABLES=varlist [/FILE=INLINE | 'path'] [/FORMAT=[LIST | FREE] [LOWER | UPPER | FULL]
            [DIAGONAL | NODIAGONAL]] [/SPLIT=varlist] [/FACTORS=varlist] [/CELLS=n] [/N=n]
            [/CONTENTS=[CORR] [COV] [MAT] [MSE] [DFE] [MEAN] [STDDEV | SD] [N_VECTOR | N] [N_SCALAR]
            [N_MATRIX] [COUNT] [PROX] [(types of within-cells records) ...]].
```

The new active dataset holds the split variables (F4.0), ROWTYPE\_ (A8), the factors (F4.0), VARNAME\_ (A8, wider for longer names) and the continuous variables (F10.4): those of VARIABLES that are neither ROWTYPE\_ nor a split or factor, which come last on VARIABLES.

With ROWTYPE\_ on VARIABLES each record starts with its fields in the order of VARIABLES: the record type (a name or a three-letter abbreviation, quoted or not) and the split and factor values (. for a pooled record). Without it CONTENTS gives the records in order, CORR when left out; types in parentheses are within-cells records, read for each of the CELLS cells with the factor values first. A matrix has one record per variable: LOWER and UPPER give a triangle, which is mirrored, FULL the whole matrix; with NODIAGONAL a correlation diagonal is 1 and any other diagonal missing. N\_SCALAR gives one value for every variable.

N=n adds an N record to each split. LIST starts each row on a new record (a row may go on over the next records; values left on a record are ignored with a warning), FREE reads the values as one stream. Values are separated by blanks or commas, and by + and - when not after D or E; . is missing. SPLIT may name one variable that is not on VARIABLES: the sets of materials are then numbered 1, 2, ... The result is split by the split variables. The data follow between BEGIN DATA and END DATA, or come from FILE. FACTOR, CLUSTER and the other procedures with MATRIX=IN(\*) read it.

## Example

```text
MATRIX DATA VARIABLES=ROWTYPE_ q1 TO q3 /FORMAT=UPPER NODIAGONAL.
BEGIN DATA
MEAN 12 7 30
SD 2 1.5 4
N 40 40 40
CORR .25 .4
CORR -.3
END DATA.
```

`statscraft help MATRIX DATA` prints the same text in the terminal. [All commands](README.md)
