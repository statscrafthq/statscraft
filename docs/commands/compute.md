<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# COMPUTE

Assign the value of an expression to a variable.

## Syntax

```text
COMPUTE target = expression.
```

The command sets target to the value of the expression for every case. A new target is numeric with format F8.2; an existing one keeps its format and labels. A string target must exist first (STRING), and a longer text is cut to its width.

When a value the expression needs is missing (system-missing, or a user-missing value of the variable), the result is system-missing, except in functions that skip missing values, such as MEAN, SUM and NVALID, and in MISSING, SYSMIS and VALUE. Operators and functions are listed under the general rules on the command index and in statscraft help.

COMPUTE is a transformation: it runs when the next procedure or EXECUTE reads the data.

## Example

```text
GET FILE='survey.sav'.
COMPUTE spend_year = spend * 12.
COMPUTE channels = SUM(ch_web, ch_app, ch_phone, ch_store, ch_email).
COMPUTE days_in = DATEDIFF(interview_date, DATE.DMY(5, 10, 2026), 'days').
DESCRIPTIVES spend_year channels days_in.
```

The example reads the sample data file [survey.sav](../sample/survey.sav), an invented customer survey; save it in the folder you run the syntax from.

## Example output

### Descriptive Statistics

| Variable | Label | N | Mean | Std Dev | Minimum | Maximum |
| --- | --- | --- | --- | --- | --- | --- |
| spend_year |  | 284 | 462.11 | 158.57 | 163.56 | 1134.24 |
| channels |  | 300 | 2.08 | 1.02 | 0 | 5 |
| days_in |  | 300 | 20.98 | 12.82 | 0 | 41 |
| Valid N (listwise) |  | 284 |  |  |  |  |
| Missing N (listwise) |  | 16 |  |  |  |  |

`statscraft help COMPUTE` prints the same text in the terminal. [All commands](README.md)
