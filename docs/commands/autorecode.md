<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# AUTORECODE

Recode values into consecutive integers in new variables, labelled with the old values.

## Syntax

```text
AUTORECODE [VARIABLES=] varlist /INTO newlist [/BLANK=VALID | MISSING] [/GROUP]
           [/DESCENDING] [/PRINT].
```

Valid values get 1, 2, ... in ascending order (DESCENDING: descending), user-missing values the codes after them, still user-missing; system-missing stays missing. Each code is labelled with the value label or the value. GROUP uses one scheme for all variables, BLANK=MISSING makes a blank string user-missing. All cases are recoded, also those a filter leaves out. APPLY TEMPLATE and SAVE TEMPLATE are not supported yet.

## Example

```text
AUTORECODE VARIABLES=country /INTO country_code /PRINT.
```

`statscraft help AUTORECODE` prints the same text in the terminal. [All commands](README.md)
