<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# AUTORECODE

Recode values into consecutive integers in new variables, labeled with the old values.

## Syntax

```text
AUTORECODE [VARIABLES=] varlist /INTO newlist [/BLANK=VALID | MISSING] [/GROUP]
           [/DESCENDING] [/PRINT] [/APPLY TEMPLATE='file'] [/SAVE TEMPLATE='file'].
```

Valid values get 1, 2, ... in ascending order (DESCENDING: descending), user-missing values the codes after them, still user-missing; system-missing stays missing. Each code is labeled with the value label or the value. GROUP uses one scheme for all variables, BLANK=MISSING makes a blank string user-missing. All cases are recoded, also those a filter leaves out. SAVE TEMPLATE writes the scheme of the first variable (the common one with GROUP or APPLY TEMPLATE) to a data file of the variables Source\_ and Target\_, the valid values only and their codes; a name without an extension gets .sat. APPLY TEMPLATE keeps those codes for those values and gives the other valid values the codes above the highest, in the order of the command, and then the user-missing ones (those of the first variable that has any); the variables must be of the type of the template and share its scheme. With both, the template written is the template applied plus the new values, also in the same file.

## Example

```text
AUTORECODE VARIABLES=country /INTO country_code /PRINT.
```

`statscraft help AUTORECODE` prints the same text in the terminal. [All commands](README.md)
