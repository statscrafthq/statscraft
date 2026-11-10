<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# Commands

One page per supported command, generated from the same table that the parser and `statscraft help` use, so the pages always match the program.

## Data files and dictionary

| Command | What it does |
|---|---|
| [GET](get.md) | Read a data file and make it the active dataset. |
| [SAVE](save.md) | Write the active dataset to a data file. |
| [ADD FILES](add-files.md) | Append the cases of several data files, matching variables by name. |
| [MATCH FILES](match-files.md) | Combine the variables of several data files case by case or by key. |
| [VARIABLE LABELS](variable-labels.md) | Set the labels of variables. |
| [VALUE LABELS](value-labels.md) | Replace the value labels of variables. |
| [MISSING VALUES](missing-values.md) | Declare user-missing values. |
| [VARIABLE LEVEL](variable-level.md) | Set the measurement level of variables. |
| [FORMATS](formats.md) | Set the display format of variables. |
| [RENAME VARIABLES](rename-variables.md) | Change the names of variables. |
| [DELETE VARIABLES](delete-variables.md) | Remove variables from the dataset. |
| [SORT CASES](sort-cases.md) | Sort the cases by the values of variables. |
| [WEIGHT](weight.md) | Weight the cases by a variable, or turn weighting off. |
| [AGGREGATE](aggregate.md) | Aggregate groups of cases into new variables or a new dataset. |
| [DATA LIST](data-list.md) | Define text data, inline or in a file, and read it as the new active dataset. |
| [BEGIN DATA](begin-data.md) | Inline data for the DATA LIST before it, up to a line that holds END DATA. |
[removed]
| [FILTER](filter.md) | Leave out of procedures the cases whose filter value is 0 or missing. |
| [TEMPORARY](temporary.md) | Make the changes that follow last only until the next procedure. |
| [N OF CASES](n-of-cases.md) | Keep only the first n cases from the next data pass on. |
| [N](n.md) | Short for N OF CASES. |
| [SAMPLE](sample.md) | Keep a random sample of the cases. |

## Transformations

| Command | What it does |
|---|---|
| [COMPUTE](compute.md) | Assign the value of an expression to a variable. |
| [IF](if.md) | Assign a value only for the cases where a condition holds. |
| [DO IF](do-if.md) | Start a block of transformations that run only where a condition holds. |
| [ELSE IF](else-if.md) | Start the next conditional branch of a DO IF block. |
| [ELSE](else.md) | Start the branch of a DO IF block for the cases no condition matched. |
| [END IF](end-if.md) | End a DO IF block. |
| [RECODE](recode.md) | Map values of variables to new values. |
| [COUNT](count.md) | Count, per case, how many variables hold one of the listed values. |
| [SELECT IF](select-if.md) | Keep only the cases where a condition holds. |
| [STRING](string.md) | Declare new string variables, filled with empty strings. |
| [NUMERIC](numeric.md) | Declare new numeric variables, filled with system-missing. |
| [EXECUTE](execute.md) | Run the pending transformations. |
| [DO REPEAT](do-repeat.md) | Repeat transformations with stand-in names replaced by variables or values. |
| [LOOP](loop.md) | Repeat transformations for each case, with an index, a condition or both. |
| [END LOOP](end-loop.md) | End a LOOP, optionally when a condition holds. |
| [BREAK](break.md) | Leave the innermost LOOP, usually inside DO IF. |
| [VECTOR](vector.md) | Name a range of variables, or new variables, as a vector for indexed use. |

## Procedures

| Command | What it does |
|---|---|
| [FREQUENCIES](frequencies.md) | Frequency tables. |
| [DESCRIPTIVES](descriptives.md) | Summary statistics of numeric variables, optionally saved as z-scores. |
| [CROSSTABS](crosstabs.md) | Contingency tables with percentages, residuals and chi-square tests. |
| [MEANS](means.md) | Statistics of dependent variables within groups of control variables. |
| [T-TEST](t-test.md) | One-sample, independent-samples and paired-samples t tests with effect sizes. |
| [ONEWAY](oneway.md) | One-way analysis of variance with contrasts and post hoc comparisons. |
| [CORRELATIONS](correlations.md) | Pearson correlations with significance and case counts. |
| [NONPAR CORR](nonpar-corr.md) | Spearman's rho and Kendall's tau-b rank correlations with significance. |
| [EXAMINE](examine.md) | Descriptives, percentiles, M-estimators, extreme values and normality tests, by factor cells. |

## Custom tables

| Command | What it does |
|---|---|
| [CTABLES](ctables.md) | Custom tables: counts, percentages and statistics by categories. |
| [MRSETS](mrsets.md) | Define, delete and list multiple response sets. |

## General rules of the language

```text
  A command ends with a period at the end of a line, at a blank line, or at the end of input.
  Command names and keywords are case-insensitive and may be shortened to three or more letters.
  varlist: variable names, a range written as first TO last, or ALL.
  Strings use single or double quotes; double the quote character to include it.
  Comments: a command starting with * or COMMENT, or /* text */ inside a line.
  Expressions: + - * / **, = <> < > <= >= (or EQ NE LT GT LE GE), AND OR NOT (or & | ~),
  functions such as MEAN(a, b) or SUM(q1 TO q5); LAG(var [, n]) is the value n cases back.
  Scratch variables start with #: they begin at 0 or blank, keep their value from case to case,
  are never saved and are gone when the next procedure starts. $CASENUM is the case number.
```

## Not supported yet

These commands of the language are recognised, but StatsCraft does not run them yet. A script that uses one stops with the error "command ... is not supported yet" at its line, and nothing is changed.

ADD DOCUMENT, ADD VALUE LABELS, ALTER TYPE, ANOVA, APPLY DICTIONARY, AUTORECODE, CACHE, CASESTOVARS, CD, CLEAR TRANSFORMATIONS, CLUSTER, DATASET ACTIVATE, DATASET CLOSE, DATASET COPY, DATASET DECLARE, DATASET DISPLAY, DATASET NAME, DATE, DEFINE, DISCRIMINANT, DISPLAY, DOCUMENT, DROP DOCUMENTS, ECHO, END CASE, END DATA, END FILE, END FILE TYPE, END INPUT PROGRAM, END MATRIX, END REPEAT, ERASE, EXPORT, FACTOR, FILE HANDLE, FILE LABEL, FILE TYPE, FINISH, FLIP, GET DATA, GET TRANSLATE, GLM, GRAPH, HOST, IMPORT, INCLUDE, INPUT PROGRAM, INSERT, LEAVE, LIST, LOGISTIC REGRESSION, MATRIX, MATRIX DATA, MULT RESPONSE, NEW FILE, NPAR TESTS, OUTPUT CLOSE, OUTPUT EXPORT, OUTPUT NEW, OUTPUT SAVE, PARTIAL CORR, PERMISSIONS, PRESERVE, PRINT, PRINT EJECT, PRINT FORMATS, PRINT SPACE, QUICK CLUSTER, RANK, RECORD TYPE, REGRESSION, RELIABILITY, REPEATING DATA, REPORT, REREAD, RESTORE, SAVE TRANSLATE, SET, SHOW, SORT VARIABLES, SUBTITLE, SYSFILE INFO, TITLE, UNIANOVA, UPDATE, USE, VARIABLE ALIGNMENT, VARIABLE ATTRIBUTE, VARIABLE ROLE, VARIABLE WIDTH, VARSTOCASES, WRITE, WRITE FORMATS, XSAVE
