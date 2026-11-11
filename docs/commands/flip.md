<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# FLIP

Transpose the data: variables become cases and cases become variables.

## Syntax

```text
FLIP [VARIABLES=ALL | varlist] [/NEWNAMES=var].
```

Each variable becomes a case, named in the new string variable CASE\_LBL; each case becomes a numeric variable (F8.2), named VAR001, VAR002, ... or by the values of NEWNAMES (the values of CASE\_LBL after an earlier FLIP). Names are upper case; a number gets K\_ in front, as does a name that does not start with a letter; other characters a name cannot hold become \_, and a repeated name gets \_A, \_B, ...; a missing or blank value keeps the default name. String values and user-missing values become system-missing. Every case is transposed whatever FILTER says. Labels, value labels, the weight, the split and the filter are gone.

Transformations before FLIP become permanent: TEMPORARY ends. A table lists the new names.

## Example

```text
FLIP VARIABLES=income price year /NEWNAMES=country.
```

`statscraft help FLIP` prints the same text in the terminal. [All commands](README.md)
