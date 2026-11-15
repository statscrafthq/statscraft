<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# SUMMARIZE

Statistics of dependents within groups of control variables, optionally listing the cases.

## Syntax

```text
SUMMARIZE [TABLES=]deps [BY controls [BY ...]] [/[TABLES=]deps BY ...] [/TITLE='text']
          [/FOOTNOTE='text'] [/CELLS=cell ... | NONE]
          [/MISSING=[EXCLUDE | INCLUDE | DEPENDENT] [VARIABLE | TABLE]]
          [/FORMAT=[NOLIST | LIST [LIMIT=n] | VALIDLIST [LIMIT=n]] [CASENUM | NOCASENUM]
                   [TOTAL | NOTOTAL] [MISSING='text']]
          [/STATISTICS=ANOVA | LINEARITY | ALL | NONE].
```

cell: as on MEANS; DEFAULT (MEAN COUNT STDDEV) when CELLS is left out, ALL for every cell.

Each table list gives a Case Processing Summary and one Case Summaries table per combination of controls (one from each BY): the groups of the first control with the next nested inside, each level ending in a Total, one row per cell statistic and one column per dependent. LIST adds the cases of each innermost group, numbered unless NOCASENUM; VALIDLIST lists only cases valid on every dependent; LIMIT=n works on the first n cases only. LIST and VALIDLIST allow one table list. NOTOTAL keeps the total rows without their label. MISSING='text' replaces the period of a system-missing value in the listing. VARIABLE (default) drops a case from the dependents it is missing on, TABLE from all of them when it is missing on any. A case missing on a control is always dropped; INCLUDE counts user-missing values as valid, DEPENDENT only for controls. A string dependent gets COUNT, FIRST, LAST and NPCT only. ANOVA and LINEARITY work as on MEANS, by each control of the first BY. TITLE and FOOTNOTE (\\n breaks a line) name the summary tables. A subcommand given twice: the last one counts. Weighted when a weight variable is set.

## Example

```text
SUMMARIZE TABLES=income spend BY region /FORMAT=LIST LIMIT=20 NOCASENUM
  /CELLS=COUNT MEAN MAX /TITLE='Spending by region'.
```

`statscraft help SUMMARIZE` prints the same text in the terminal. [All commands](README.md)
