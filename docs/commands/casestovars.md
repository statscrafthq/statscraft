<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# CASESTOVARS

Turn groups of cases into one case each, with a variable per value of an index.

## Syntax

```text
CASESTOVARS [/ID=varlist] [/INDEX=varlist] [/FIXED=varlist] [/AUTOFIX=YES | NO]
            [/VIND [ROOT=name]] [/COUNT=new ['label']] [/RENAME var=root ...]
            [/SEPARATOR='text'] [/GROUPBY=VARIABLE | INDEX] [/DROP=varlist].
```

A row group is a run of cases with the same values of the split and ID variables; the data must be sorted by them. Cases with a system-missing or blank ID are left out with a warning. Every other variable that varies within a group becomes a variable group: one variable per index value (INDEX: the values or combinations that occur, ascending; without INDEX the position in the group), named root, separator ('.') and the index values. With one string index the values are the names (root, separator and value when several variables are restructured); names that cannot be used become V1, V2, ... AUTOFIX=YES copies variables that do not vary (one value plus system-missing or blanks counts as not varying), and warns about FIXED variables that vary; AUTOFIX=NO makes such a FIXED variable an error. New variables keep the dictionary of their source; the label adds the index values. VIND adds a 0/1 indicator (F1.0) per index value, COUNT the cases of the group (F4.0). Order: split, ID and fixed variables as in the file, the variable groups, the indicators, COUNT. User-missing values count as values. All cases are used whatever FILTER says; WEIGHT, SPLIT FILE and FILTER stay unless their variables are gone or spread. Not after TEMPORARY.

## Example

```text
CASESTOVARS /ID=person /INDEX=wave /GROUPBY=VARIABLE.
```

`statscraft help CASESTOVARS` prints the same text in the terminal. [All commands](README.md)
