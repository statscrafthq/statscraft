<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# GET

Read a data file and make it the active dataset.

## Syntax

```text
GET FILE='path' | handle [/KEEP=varlist] [/DROP=varlist] [/RENAME=(old = new) ...] [/MAP].
```

KEEP, DROP and RENAME work in the order written, as in ADD FILES. MAP lists the variables as the options before it leave them, each with its name in the file; it may come as often as you like.

## Example

```text
GET FILE='survey.sav'.
GET FILE='survey.sav' /KEEP=id region satisfaction spend.
GET FILE='survey.sav' /DROP=ch_web TO ch_email /RENAME=(spend = monthly_bill) /MAP.
```

The example reads the sample data file [survey.sav](../sample/survey.sav), an invented customer survey; save it in the folder you run the syntax from.

## Example output

### Variables in the active dataset

| Variable | Name in the file |
| --- | --- |
| id | id |
| region | region |
| gender | gender |
| age | age |
| age_group | age_group |
| education | education |
| weight | weight |
| satisfaction | satisfaction |
| recommend | recommend |
| contacted | contacted |
| switching | switching |
| monthly_bill | spend |
| interview_date | interview_date |

`statscraft help GET` prints the same text in the terminal. [All commands](README.md)
