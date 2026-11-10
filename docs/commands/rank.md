<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# RANK

Ranks, percentile groups, proportion estimates, normal and Savage scores as new variables.

## Syntax

```text
RANK [VARIABLES=]varlist [(A | D)] [varlist [(A | D)] ...] [BY varlist]
     [/RANK] [/RFRACTION] [/PERCENT] [/PROPORTION] [/N] [/NTILES(k)] [/SAVAGE] [/NORMAL]
     [/TIES=MEAN | LOW | HIGH | CONDENSE] [/FRACTION=BLOM | RANKIT | TUKEY | VW]
     [/PRINT=YES | NO] [/MISSING=EXCLUDE | INCLUDE].
```

Each function, given at most once and followed by INTO names if wished, adds one new variable per ranked variable; without any, RANK is done. A (ascending, the default) or D (the largest value first) applies to the variables before it back to the previous A or D. BY ranks within each combination of the BY values; a missing BY value forms a group of its own. With the sum of weights W: RFRACTION is the rank r divided by W, PERCENT that times 100, N is W, NTILES(k) the group 1 to k, the integer part of r k / (W + 1) plus 1. PROPORTION follows FRACTION: BLOM (r - 3/8) / (W + 1/4) (the default), RANKIT (r - 1/2) / W, TUKEY (r - 1/3) / (W + 1/3), VW r / (W + 1); NORMAL is its standard normal quantile and SAVAGE the exponential score. TIES gives tied values the mean (default), lowest or highest rank; CONDENSE ranks the distinct values 1, 2, 3. A missing value (user-missing unless INCLUDE) or a missing, zero or negative weight gives system-missing. Names after INTO are used in order; the others are the function's first letter and the first seven characters of the name, then the function's first three letters and 001, 002 when taken, then RNK, two letters and 01. Labels read RANK of salary (by the BY variables). PRINT=NO leaves out the table of the new variables. Within SPLIT FILE groups the ranks start over.

## Example

```text
RANK VARIABLES=salary (D) BY region /RANK /NTILES(4) INTO salary_q /TIES=LOW.
```

`statscraft help RANK` prints the same text in the terminal. [All commands](README.md)
