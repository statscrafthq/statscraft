<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# IF

Assign a value only for the cases where a condition holds.

## Syntax

```text
IF (condition) target = expression.
```

The parentheses around the condition are optional. The command sets target only for the cases where the condition is true; where it is false or missing the target keeps its value. A new target starts as system-missing (numeric, F8.2) for every case, so cases the condition leaves out stay missing. A string target must exist first (STRING). Several IF commands in a row are checked one after another, so a later one may overwrite an earlier one.

## Example

```text
GET FILE='survey.sav'.
IF (satisfaction <= 2 AND switching = 1) at_risk = 1.
IF (age >= 60 AND ch_app = 0) offline = 1.
RECODE at_risk offline (SYSMIS = 0).
FREQUENCIES at_risk offline.
```

The example reads the sample data file [survey.sav](../sample/survey.sav), an invented customer survey; save it in the folder you run the syntax from.

## Example output

### at_risk

| Category | Value | Label | Frequency | Percent | Valid Percent | Cumulative Percent |
| --- | --- | --- | --- | --- | --- | --- |
| valid | 0 |  | 269 | 89.67 | 89.67 | 89.67 |
| valid | 1 |  | 31 | 10.33 | 10.33 | 100 |
| total |  |  | 300 | 100 |  |  |

`statscraft help IF` prints the same text in the terminal. [All commands](README.md)
