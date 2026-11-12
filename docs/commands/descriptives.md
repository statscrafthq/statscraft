<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# DESCRIPTIVES

Summary statistics of numeric variables, optionally saved as z-scores.

## Syntax

```text
DESCRIPTIVES [/]VARIABLES=varlist [/STATISTICS=stat ...] [/MISSING=VARIABLE | LISTWISE [INCLUDE]] [/SAVE].
```

stat: MEAN SEMEAN STDDEV VARIANCE KURTOSIS SKEWNESS RANGE MINIMUM MAXIMUM SUM DEFAULT ALL.

DEFAULT is MEAN STDDEV MINIMUM MAXIMUM; N is always shown. Weighted when a weight variable is set.

MISSING=VARIABLE (the default) leaves out the missing values of each variable on its own; LISTWISE leaves out a case missing on any listed variable; INCLUDE counts user-missing values.

The row Valid N (listwise) counts the cases valid on every listed variable. String variables get no statistics; a warning names them. var (name) in the list adds a z-score under that name; SAVE adds one for every variable, named Z plus the variable name (ZSC001 to ZSC099, STDZ01 to STDZ09, ZZZZ01 to ZZZZ09, ZQZQ01 to ZQZQ09 when that name is taken), labeled Zscore: plus the label.

## Example

```text
DESCRIPTIVES VARIABLES=age income (z_inc) /STATISTICS=DEFAULT SUM /SAVE.
```

`statscraft help DESCRIPTIVES` prints the same text in the terminal. [All commands](README.md)
