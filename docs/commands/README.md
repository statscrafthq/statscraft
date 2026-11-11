<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# Commands

One page per supported command, generated from the same table that the parser and `statscraft help` use, so the pages always match the program.

## Data files and dictionary

| Command | What it does |
|---|---|
| [GET](get.md) | Read a data file and make it the active dataset. |
| [GET DATA](get-data.md) | Read a text data file, delimited or in fixed columns, as the active dataset. |
| [SAVE](save.md) | Write the active dataset to a data file. |
| [ADD FILES](add-files.md) | Append the cases of several data files, matching variables by name. |
| [MATCH FILES](match-files.md) | Combine the variables of several data files case by case or by key. |
| [VARIABLE LABELS](variable-labels.md) | Set the labels of variables. |
| [VALUE LABELS](value-labels.md) | Replace the value labels of variables. |
| [MISSING VALUES](missing-values.md) | Declare user-missing values. |
| [VARIABLE LEVEL](variable-level.md) | Set the measurement level of variables. |
| [VARIABLE WIDTH](variable-width.md) | Set how many characters wide variables show in the data grid. |
| [FORMATS](formats.md) | Set the display format of variables. |
| [RENAME VARIABLES](rename-variables.md) | Change the names of variables. |
| [DELETE VARIABLES](delete-variables.md) | Remove variables from the dataset. |
| [SORT CASES](sort-cases.md) | Sort the cases by the values of variables. |
| [WEIGHT](weight.md) | Weight the cases by a variable, or turn weighting off. |
| [AGGREGATE](aggregate.md) | Aggregate groups of cases into new variables or a new dataset. |
| [ADD VALUE LABELS](add-value-labels.md) | Add or change value labels, keeping the other labels of the variables. |
| [FILE LABEL](file-label.md) | Set the label of the data file. |
| [DOCUMENT](document.md) | Add free text to the documents of the data file. |
| [ADD DOCUMENT](add-document.md) | Add quoted lines to the documents of the data file. |
| [DROP DOCUMENTS](drop-documents.md) | Remove the documents of the data file. |
| [VARIABLE ROLE](variable-role.md) | Set the role of variables for dialogs. |
| [VARIABLE ALIGNMENT](variable-alignment.md) | Set the alignment of variables in the data grid. |
| [VARIABLE ATTRIBUTE](variable-attribute.md) | Set or delete custom attributes of variables. |
| [DATAFILE ATTRIBUTE](datafile-attribute.md) | Set or delete custom attributes of the data file. |
| [ALTER TYPE](alter-type.md) | Change the type, string width or format of variables. |
| [APPLY DICTIONARY](apply-dictionary.md) | Copy dictionary information from a data file or dataset to the active dataset. |
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
| [AUTORECODE](autorecode.md) | Recode values into consecutive integers in new variables, labelled with the old values. |
| [RANK](rank.md) | Ranks, percentile groups, proportion estimates, normal and Savage scores as new variables. |

## Procedures

| Command | What it does |
|---|---|
| [FREQUENCIES](frequencies.md) | Frequency tables, summary statistics and percentiles. |
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

## Macros, files and session

| Command | What it does |
|---|---|
| [DEFINE](define.md) | Define a macro: a name that stands for syntax, with arguments. |
| [INSERT](insert.md) | Run the commands of another syntax file in place of this command. |
| [INCLUDE](include.md) | Run the commands of another syntax file, read by the batch rules. |
| [BEGIN PROGRAM](begin-program.md) | A block of code in another programming language; it is skipped with a warning. |
| [SET](set.md) | Change settings of the session. |
| [SHOW](show.md) | Show the current settings. |
| [PRESERVE](preserve.md) | Store the current settings and folder, to bring them back with RESTORE. |
| [RESTORE](restore.md) | Bring back the settings and folder the last PRESERVE stored. |
| [CACHE](cache.md) | Accepted and has nothing to do: the data are always kept in memory. |
| [CD](cd.md) | Change the folder relative file names start from. |
| [DATASET NAME](dataset-name.md) | Give the active dataset a name, so that it stays open when another one becomes active. |
| [DATASET ACTIVATE](dataset-activate.md) | Make a named dataset the active one. |
| [DATASET COPY](dataset-copy.md) | Copy the active dataset, as it is now, into a named dataset. |
| [DATASET DECLARE](dataset-declare.md) | Reserve a dataset name for a dataset that a later command creates. |
| [DATASET CLOSE](dataset-close.md) | Close a named dataset, or take the name off the active one. |
| [DATASET DISPLAY](dataset-display.md) | List the open datasets. |
| [USE](use.md) | Use all cases, or a range of cases, in the commands that follow. |
| [DISPLAY](display.md) | Show dictionary information of the active dataset. |
| [LIST](list.md) | List the values of cases. |

## General rules of the language

```text
  A command ends with a period at the end of a line, at a blank line, or at the end of input.
  Command names and keywords are case-insensitive and may be shortened to three or more letters.
  varlist: variable names, a range written as first TO last, or ALL.
  Strings use single or double quotes; double the quote character to include it. A string may
  run on over line breaks, which it keeps; 'a' + 'b' joins two strings into one.
  Comments: a command starting with * or COMMENT, or /* text */ inside a line.
  Expressions: + - * / **, = <> < > <= >= (or EQ NE LT GT LE GE), AND OR NOT (or & | ~),
  functions such as MEAN(a, b) or SUM(q1 TO q5); LAG(var [, n]) is the value n cases back.
  Scratch variables start with #: they begin at 0 or blank, keep their value from case to case,
  are never saved and are gone when the next procedure starts. $CASENUM is the case number.
```

## Not supported yet

These commands of the language are recognised, but Statscraft does not run them yet. A script that uses one stops with the error "command ... is not supported yet" at its line, and nothing is changed.

ANOVA, CASESTOVARS, CLEAR TRANSFORMATIONS, CLUSTER, DATE, DISCRIMINANT, ECHO, END CASE, END DATA, END FILE, END FILE TYPE, END INPUT PROGRAM, END MATRIX, END REPEAT, ERASE, EXPORT, FACTOR, FILE HANDLE, FILE TYPE, FINISH, FLIP, GET TRANSLATE, GLM, GRAPH, HOST, IMPORT, INPUT PROGRAM, LEAVE, LOGISTIC REGRESSION, MATRIX, MATRIX DATA, MULT RESPONSE, NEW FILE, NPAR TESTS, OUTPUT CLOSE, OUTPUT EXPORT, OUTPUT NEW, OUTPUT SAVE, PARTIAL CORR, PERMISSIONS, PRINT, PRINT EJECT, PRINT FORMATS, PRINT SPACE, QUICK CLUSTER, RECORD TYPE, REGRESSION, RELIABILITY, REPEATING DATA, REPORT, REREAD, SAVE TRANSLATE, SORT VARIABLES, STAR JOIN, SUBTITLE, SYSFILE INFO, TITLE, TREE, UNIANOVA, UPDATE, VARSTOCASES, WRITE, WRITE FORMATS, XSAVE
