<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# ORTHOPLAN

Generates an orthogonal main-effects plan of profiles for a conjoint study.

## Syntax

```text
ORTHOPLAN [/FACTORS=name ['label'] [name ['label'] ...] (value ['label'] ... | 'level' ...) ...]
   [/REPLACE | /OUTFILE='file' | dataset] [/MINIMUM=n] [/HOLDOUT=n] [/MIXHOLD=YES | NO].
```

Builds the smallest plan in which every pair of factors is balanced against each other (the full factorial when that is smallest), with at least MINIMUM profiles when a plan of at most 81 can have them, else the largest one. A factor has 2 to 9 levels, 90 levels in all; the levels are values, each with an optional label, or quoted texts that become levels 1, 2, ... with the texts as labels, and a level list serves every name written since the last list. The plan's columns and order are random: SET SEED repeats them. HOLDOUT adds random profiles that repeat neither the plan nor each other, after the plan or, with MIXHOLD=YES, mixed into it. STATUS\_ is 0 for the plan, 1 for holdouts and 2 for simulation profiles; CARD\_ numbers the profiles. Without FACTORS the variables of the active dataset are the factors and their value labels the levels. Cases in the active dataset whose variables are the factors are kept after the plan as simulation profiles.

The plan replaces the active dataset with REPLACE, goes to the file or dataset of OUTFILE, and is otherwise added to the active dataset. SPLIT FILE and WEIGHT are not used. A table counts the profiles of each kind.

## Example

```text
ORTHOPLAN FACTORS=brand 'Brand' ('Acme' 'Brio' 'Cora') price 'Price' (5 '5 euro' 7 '7 euro' 9 '9 euro') organic (0 'no' 1 'yes') /HOLDOUT=2.
```

`statscraft help ORTHOPLAN` prints the same text in the terminal. [All commands](README.md)
