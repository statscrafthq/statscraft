<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# COMPARE DATASETS

Compare the values and dictionary of the active dataset with another dataset or .sav file.

## Syntax

```text
COMPARE DATASETS /COMPDATASET 'file.sav' [PASSWORD='text'] | dataset
  /VARIABLES varlist | ALL [/CASEID varlist]
  [/SAVE [FLAGMISMATCHES=YES | NO] [VARNAME=name]
         [MATCHDATASET=NO | YES] [MATCHNAME=dataset | 'file' [MATCHPASS='text' | NONE]]
         [MISMATCHDATASET=NO | YES] [MISMATCHNAME=dataset | 'file' [MISMATCHPASS=...]]
         [ENCRYPTEDPW=NO | YES]]
  [/OUTPUT [VARPROPERTIES=NONE | ALL | MEASURE LABEL VALUELABELS COLUMNS MISSING ALIGN ROLE
                          ATTRIBUTES WIDTH] [CASETABLE=YES | NO [TABLELIMIT=100 | n | NONE]]].
```

Every subcommand and keyword is written in full. Without CASEID the first case of each file is paired with the first of the other, and so on; with CASEID cases pair by their key values and both files must be sorted by the keys in ascending order (a key held twice pairs in file order).

Each listed variable that the other file also has, with the same type, is compared value by value: numbers exactly, system-missing equal only to system-missing, strings without trailing blanks. A variable the other file lacks or holds with the other type is listed, not compared.

The tables: a summary of cases and variables, the variables not compared, the number of cases that differ per variable, and the case table (one row per differing value, the first 100 cases by default). The flag variable, CasesCompare unless VARNAME names another, is added to the active dataset unless FLAGMISMATCHES=NO: 0 the paired case has the same values, 1 it differs, -1 the other file has no such case; an existing numeric variable of that name is overwritten. MATCHDATASET and MISMATCHDATASET write the active cases flagged 0, or flagged 1 or -1, to a dataset declared before or a .sav file; encrypted files cannot be written.

VARPROPERTIES lists every requested property that differs between the files. SPLIT FILE and FILTER are ignored.

## Example

```text
COMPARE DATASETS /COMPDATASET 'wave1_checked.sav' /VARIABLES q1 TO q12 /CASEID respid
  /SAVE VARNAME=changed MISMATCHDATASET=YES MISMATCHNAME=review
  /OUTPUT VARPROPERTIES=LABEL VALUELABELS TABLELIMIT=50.
```

`statscraft help COMPARE DATASETS` prints the same text in the terminal. [All commands](README.md)
