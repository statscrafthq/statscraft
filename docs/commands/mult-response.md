<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# MULT RESPONSE

Frequency tables and crosstabulations of multiple response groups and variables.

## Syntax

```text
MULT RESPONSE [/]GROUPS=name ['label'] (varlist (value)) | (varlist (min, max)) [name ...]
              [/VARIABLES=varlist (min, max) [varlist (min, max) ...]]
              [/FREQUENCIES=items]
              [/TABLES=items BY items [BY items ...] [(PAIRED)] [/items BY ...]]
              [/MISSING=[TABLE | MDGROUP | MRGROUP] [INCLUDE]]
              [/FORMAT=[LABELS | NOLABELS] [TABLE | CONDENSE | ONEPAGE] [DOUBLE]]
              [/BASE=CASES | RESPONSES] [/CELLS=[COUNT] [ROW] [COLUMN] [TOTAL] [ALL]].
```

GROUPS, VARIABLES, FREQUENCIES and TABLES come in this order, before the others; at least one of GROUPS and VARIABLES and one of FREQUENCIES and TABLES. One value makes a dichotomy group (its categories are the variables with that value), two a category group (the values from min to max, counted over all its variables). Groups exist only in this command.

items: groups, and variables named on VARIABLES; TO follows their order there. Values are truncated to whole numbers; values out of range do not count. Empty categories are left out.

A case is missing for a group with no counted value; MDGROUP and MRGROUP leave out a case missing on any variable of a dichotomy or category group (MRGROUP also for a value out of range); INCLUDE counts user-missing values. FREQUENCIES: responses, percent of responses and percent of cases. TABLES: counts, and the percentages CELLS asks for (ALL without keywords), based on cases or with BASE=RESPONSES on responses. PAIRED pairs the first variable of a category group with the first of another, and so on; its percentages are based on responses. NOLABELS and CONDENSE show values for category groups and variables; ONEPAGE and DOUBLE only change printed layout and are ignored with a warning. Weights count as they are.

## Example

```text
MULT RESPONSE GROUPS=media 'Media used' (tv radio web (1))
  brands 'Brands known' (brand1 TO brand3 (1, 9))
  /VARIABLES=gender (1, 2)
  /FREQUENCIES=media brands
  /TABLES=media brands BY gender
  /CELLS=COLUMN.
```

`statscraft help MULT RESPONSE` prints the same text in the terminal. [All commands](README.md)
