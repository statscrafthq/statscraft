<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# FILTER

Leave out of procedures the cases whose filter value is 0 or missing.

## Syntax

```text
FILTER BY var.
FILTER OFF.
```

The cases stay in the dataset. The filter is kept with the dataset, follows a rename and ends when the variable is deleted or the dataset replaced.

## Example

```text
GET FILE='survey.sav'.
COMPUTE app_user = (ch_app = 1).
FILTER BY app_user.
DESCRIPTIVES age spend.
FILTER OFF.
DESCRIPTIVES age spend.
```

The example reads the sample data file [survey.sav](../sample/survey.sav), an invented customer survey; save it in the folder you run the syntax from.

## Example output

### Descriptive Statistics

| Variable | Label | N | Mean | Std Dev | Minimum | Maximum |
| --- | --- | --- | --- | --- | --- | --- |
| age | Age in years | 86 | 39.90 | 12.16 | 18 | 64 |
| spend | Monthly spend in euros | 84 | 39.24 | 13.65 | 15.79 | 94.52 |
| Valid N (listwise) |  | 84 |  |  |  |  |
| Missing N (listwise) |  | 2 |  |  |  |  |

`statscraft help FILTER` prints the same text in the terminal. [All commands](README.md)
