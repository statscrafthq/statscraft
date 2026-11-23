<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# RECODE

Map values of variables to new values.

## Syntax

```text
RECODE varlist (inputs = output) [(inputs = output) ...] [INTO newvars] [/ varlist ...].
```

inputs: values, low THRU high (with LOWEST or HIGHEST), MISSING, SYSMIS, ELSE.

(CONVERT), for a string into a numeric target: at its place, a string that reads as a number becomes that number and a blank one system-missing; other strings go on to the next (inputs).

output: a value, SYSMIS or COPY.

The first matching (inputs = output) wins. A value or range also matches user-missing values.

Without a match a variable keeps its value, and so does an existing INTO target; a new target becomes system-missing.

A string target of INTO must exist already. A quoted value longer than its variable is an error.

## Example

```text
GET FILE='survey.sav'.
RECODE satisfaction (1, 2 = 1) (3 = 2) (4, 5 = 3) (MISSING = SYSMIS) INTO sat3.
VALUE LABELS sat3 1 'Dissatisfied' 2 'Neutral' 3 'Satisfied'.
RECODE age (LOWEST THRU 34 = 1) (35 THRU 54 = 2) (55 THRU HIGHEST = 3) INTO age3.
RECODE ch_web TO ch_email (SYSMIS = 0).
FREQUENCIES sat3.
```

The example reads the sample data file [survey.sav](../sample/survey.sav), an invented customer survey; save it in the folder you run the syntax from.

## Example output

### sat3

| Category | Value | Label | Frequency | Percent | Valid Percent | Cumulative Percent |
| --- | --- | --- | --- | --- | --- | --- |
| valid | 1 | Dissatisfied | 86 | 28.67 | 31.05 | 31.05 |
| valid | 2 | Neutral | 85 | 28.33 | 30.69 | 61.73 |
| valid | 3 | Satisfied | 106 | 35.33 | 38.27 | 100 |
| missing |  | System | 23 | 7.67 |  |  |
| total |  |  | 300 | 100 |  |  |

`statscraft help RECODE` prints the same text in the terminal. [All commands](README.md)
