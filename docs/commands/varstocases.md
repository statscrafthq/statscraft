<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# VARSTOCASES

Turn groups of variables into cases: one new case per variable of the group.

## Syntax

```text
VARSTOCASES /MAKE new ['label'] [FROM] varlist [/MAKE ...]
            [/INDEX=i ['label'] | i ['label'] (make) | i ['label'] (n) j ['label'] (n) ...]
            [/ID=new ['label']] [/NULL=DROP | KEEP] [/COUNT=new ['label']]
            [/KEEP=ALL | varlist | /DROP=varlist].
```

Every case becomes one case per variable of the FROM lists, in their order; FROM lists have equal lengths and one type each. A MAKE variable takes the dictionary of its first FROM variable (strings their widest width), the label given replacing the label. The other variables are copied to every new case (KEEP: only those, in that order). INDEX numbers the new cases 1, 2, ...; with (make) it holds the names of that MAKE's FROM variables; with (n) several factors whose levels multiply to the length of the lists, the last varying fastest.

ID is the case number of the original case (F8.0), COUNT the number of new cases it gave (F4.0). NULL=DROP (the default) leaves out a new case whose MAKE values are all system-missing or blank. Order of the new file: ID, the copied variables, COUNT, INDEX, MAKE; ID and INDEX are nominal, COUNT scale. All cases are used whatever FILTER says; WEIGHT, SPLIT FILE and FILTER stay unless their variables are gone. Not after TEMPORARY.

## Example

```text
VARSTOCASES /MAKE score 'Score' FROM score1 TO score4 /INDEX=wave /ID=person.
```

`statscraft help VARSTOCASES` prints the same text in the terminal. [All commands](README.md)
