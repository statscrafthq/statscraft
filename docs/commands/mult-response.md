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
GET FILE='survey.sav'.
MULT RESPONSE GROUPS=channels 'Channels used' (ch_web TO ch_email (1))
  /FREQUENCIES=channels.
MULT RESPONSE GROUPS=channels 'Channels used' (ch_web TO ch_email (1))
  /VARIABLES=age_group (1, 4)
  /TABLES=channels BY age_group /CELLS=COUNT COLUMN /BASE=CASES.
```

The example reads the sample data file [survey.sav](../sample/survey.sav), an invented customer survey; save it in the folder you run the syntax from.

## Example output

### Channels used Frequencies

| Channels used | Responses / N | Responses / Percent | Percent of Cases |
| --- | --- | --- | --- |
| Website | 212 | 33.97 | 74.39 |
| Mobile app | 86 | 13.78 | 30.18 |
| Phone | 149 | 23.88 | 52.28 |
| Store | 65 | 10.42 | 22.81 |
| Email | 112 | 17.95 | 39.30 |
| Total | 624 | 100 | 218.95 |

channels is a dichotomy group counting the value 1.

`statscraft help MULT RESPONSE` prints the same text in the terminal. [All commands](README.md)
