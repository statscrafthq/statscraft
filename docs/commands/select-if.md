<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# SELECT IF

Keep only the cases where a condition holds.

## Syntax

```text
SELECT IF condition.
```

The command keeps the cases where the condition is true and drops the others, also those where it is missing. The cases are gone for every later command; after TEMPORARY the selection holds only for the next procedure. FILTER hides cases without deleting them. Like other transformations, it runs when the next procedure or EXECUTE reads the data.

## Example

```text
GET FILE='survey.sav'.
SELECT IF (region = 3).
FREQUENCIES satisfaction.
GET FILE='survey.sav'.
SELECT IF (age >= 30 AND age < 60 AND NOT MISSING(spend)).
DESCRIPTIVES age spend.
```

The example reads the sample data file [survey.sav](../sample/survey.sav), an invented customer survey; save it in the folder you run the syntax from.

## Example output

### Descriptive Statistics

| Variable | Label | N | Mean | Std Dev | Minimum | Maximum |
| --- | --- | --- | --- | --- | --- | --- |
| age | Age in years | 205 | 44.76 | 7.87 | 30 | 59 |
| spend | Monthly spend in euros | 205 | 38.28 | 12.81 | 13.63 | 94.52 |
| Valid N (listwise) |  | 205 |  |  |  |  |
| Missing N (listwise) |  | 0 |  |  |  |  |

`statscraft help SELECT IF` prints the same text in the terminal. [All commands](README.md)
