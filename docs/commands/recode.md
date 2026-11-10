<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# RECODE

Map values of variables to new values.

## Syntax

```text
RECODE varlist (inputs = output) [(inputs = output) ...] [INTO newvars] [/ varlist ...].
```

inputs: values, low THRU high (with LOWEST or HIGHEST), MISSING, SYSMIS, ELSE.

output: a value, SYSMIS or COPY.

The first matching (inputs = output) wins. A value or range also matches user-missing values.

Without a match a variable keeps its value, and so does an existing INTO target; a new target becomes system-missing.

A string target of INTO must exist already. A quoted value longer than its variable is an error.

## Example

```text
RECODE age (LOWEST THRU 29 = 1) (30 THRU 59 = 2) (60 THRU HIGHEST = 3) INTO age_group.
```

`statscraft help RECODE` prints the same text in the terminal. [All commands](README.md)
