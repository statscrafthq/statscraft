<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# WEIGHT

Weight the cases by a variable, or turn weighting off.

## Syntax

```text
WEIGHT BY var.
WEIGHT OFF.
```

The command makes each case count as many times as its value of var, a numeric variable: a case with weight 2.5 counts as two and a half cases. Weighting stays on until WEIGHT OFF, until the variable is deleted, or until another data file is read, and SAVE stores it with the data. Fractional weights are used as they are in counts, percentages and statistics; most significance tests, confidence intervals and standard errors round them. A case whose weight is zero, negative or missing is left out. SHOW WEIGHT tells which variable weights the data.

## Example

```text
GET FILE='survey.sav'.
WEIGHT BY weight.
FREQUENCIES age_group.
WEIGHT OFF.
FREQUENCIES age_group.
```

The example reads the sample data file [survey.sav](../sample/survey.sav), an invented customer survey; save it in the folder you run the syntax from.

## Example output

### Age group

| Category | Value | Label | Frequency | Percent | Valid Percent | Cumulative Percent |
| --- | --- | --- | --- | --- | --- | --- |
| valid | 1 | 18-29 | 50.68 | 15.97 | 15.97 | 15.97 |
| valid | 2 | 30-44 | 106.51 | 33.55 | 33.55 | 49.52 |
| valid | 3 | 45-59 | 117.82 | 37.12 | 37.12 | 86.64 |
| valid | 4 | 60 and over | 42.41 | 13.36 | 13.36 | 100 |
| total |  |  | 317.42 | 100 |  |  |

`statscraft help WEIGHT` prints the same text in the terminal. [All commands](README.md)
