<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# Commands

One page per supported command, generated from the same table that the parser and `statscraft help` use, so the pages always match the program.

## Data files and dictionary

| Command | What it does |
|---|---|
| [GET](get.md) | Read a data file and make it the active dataset. |
| [GET DATA](get-data.md) | Read a text data file or a sheet of a workbook as the active dataset. |
| [GET TRANSLATE](get-translate.md) | Read a tab-delimited text file, a dBASE file or an old Excel workbook. |
| [IMPORT](import.md) | Read a portable (.por) file and make it the active dataset. |
| [SYSFILE INFO](sysfile-info.md) | Show the dictionary of a data file without opening it as the active dataset. |
| [SAVE](save.md) | Write the active dataset to a data file. |
| [SAVE TRANSLATE](save-translate.md) | Write the active dataset to a CSV or tab-delimited text file or an Excel workbook. |
| [EXPORT](export.md) | Write the active dataset to a portable (.por) file. |
| [XSAVE](xsave.md) | Write the cases to a data file as the next data pass reads them. |
| [NEW FILE](new-file.md) | Start a new, empty active dataset. |
| [ADD FILES](add-files.md) | Append the cases of several data files, matching variables by name. |
| [MATCH FILES](match-files.md) | Combine the variables of several data files case by case or by key. |
| [STAR JOIN](star-join.md) | Add the fields of lookup files to every case of a data file, by keys, as an SQL left join. |
| [UPDATE](update.md) | Replace values of a primary file with the values of transaction files, by key. |
| [VARIABLE LABELS](variable-labels.md) | Set the labels of variables. |
| [VALUE LABELS](value-labels.md) | Replace the value labels of variables. |
| [MISSING VALUES](missing-values.md) | Declare user-missing values. |
| [VARIABLE LEVEL](variable-level.md) | Set the measurement level of variables. |
| [VARIABLE WIDTH](variable-width.md) | Set how many characters wide variables show in the data grid. |
| [FORMATS](formats.md) | Set the display format of variables. |
| [PRINT FORMATS](print-formats.md) | Set the print format of variables: how procedures and PRINT show them. |
| [WRITE FORMATS](write-formats.md) | Set the write format of variables: how WRITE writes them. |
| [RENAME VARIABLES](rename-variables.md) | Change the names of variables. |
| [DELETE VARIABLES](delete-variables.md) | Remove variables from the dataset. |
| [SORT CASES](sort-cases.md) | Sort the cases by the values of variables. |
| [SORT VARIABLES](sort-variables.md) | Put the variables in order by a property of their dictionary entries. |
| [WEIGHT](weight.md) | Weight the cases by a variable, or turn weighting off. |
| [AGGREGATE](aggregate.md) | Aggregate groups of cases into new variables or a new dataset. |
| [VARSTOCASES](varstocases.md) | Turn groups of variables into cases: one new case per variable of the group. |
| [CASESTOVARS](casestovars.md) | Turn groups of cases into one case each, with a variable per value of an index. |
| [FLIP](flip.md) | Transpose the data: variables become cases and cases become variables. |
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
| [MATRIX DATA](matrix-data.md) | Read matrix materials (correlations, means, counts) as a matrix data file. |
| [INPUT PROGRAM](input-program.md) | Start an input program: commands that build the cases of a new active dataset. |
| [END INPUT PROGRAM](end-input-program.md) | End an input program and run it. |
| [END CASE](end-case.md) | In an input program: add the case built so far to the dataset and go on. |
| [END FILE](end-file.md) | In an input program or FILE TYPE: stop reading; the case being built is dropped. |
| [REREAD](reread.md) | In an input program: let the next DATA LIST read the last record again. |
| [REPEATING DATA](repeating-data.md) | In an input program or FILE TYPE: one case per repeating group of a record. |
| [FILE TYPE](file-type.md) | Start the definition of a file with several kinds of records. |
| [RECORD TYPE](record-type.md) | Inside FILE TYPE: the record types the following DATA LIST reads. |
| [END FILE TYPE](end-file-type.md) | End a FILE TYPE definition and read the file. |
| [SPLIT FILE](split-file.md) | Run each procedure once per run of consecutive cases that share the split values. |
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
| [LEAVE](leave.md) | Keep the values of new variables from case to case instead of resetting them. |
| [EXECUTE](execute.md) | Run the pending transformations. |
| [DO REPEAT](do-repeat.md) | Repeat transformations with stand-in names replaced by variables or values. |
| [LOOP](loop.md) | Repeat transformations for each case, with an index, a condition or both. |
| [END LOOP](end-loop.md) | End a LOOP, optionally when a condition holds. |
| [BREAK](break.md) | Leave the innermost LOOP, usually inside DO IF. |
| [VECTOR](vector.md) | Name a range of variables, or new variables, as a vector for indexed use. |
| [AUTORECODE](autorecode.md) | Recode values into consecutive integers in new variables, labeled with the old values. |
| [RANK](rank.md) | Ranks, percentile groups, proportion estimates, normal and Savage scores as new variables. |
| [DATE](date.md) | Add date variables that count the cases of a time series. |
| [PRINT](print.md) | Show the values of variables for every case, as lines of text. |
| [PRINT EJECT](print-eject.md) | Like PRINT, starting a new page each time it runs. |
| [PRINT SPACE](print-space.md) | Add blank lines to the PRINT output, for every case it runs on. |
| [WRITE](write.md) | Write the values of variables for every case to a text file. |
| [CLEAR TRANSFORMATIONS](clear-transformations.md) | Undo the transformations and dictionary changes since the last data pass. |

## Procedures

| Command | What it does |
|---|---|
| [FREQUENCIES](frequencies.md) | Frequency tables, summary statistics and percentiles. |
| [DESCRIPTIVES](descriptives.md) | Summary statistics of numeric variables, optionally saved as z-scores. |
| [CROSSTABS](crosstabs.md) | Contingency tables with percentages, residuals and chi-square tests. |
| [MULT RESPONSE](mult-response.md) | Frequency tables and crosstabulations of multiple response groups and variables. |
| [MEANS](means.md) | Statistics of dependent variables within groups of control variables. |
| [T-TEST](t-test.md) | One-sample, independent-samples and paired-samples t tests with effect sizes. |
| [ONEWAY](oneway.md) | One-way analysis of variance with contrasts and post hoc comparisons. |
| [CORRELATIONS](correlations.md) | Pearson correlations with significance and case counts. |
| [NONPAR CORR](nonpar-corr.md) | Spearman's rho and Kendall's tau-b rank correlations with significance. |
| [PARTIAL CORR](partial-corr.md) | Partial correlations of variables controlling for others, with significance. |
| [NPAR TESTS](npar-tests.md) | Nonparametric tests: one-sample, two and k related samples, two and k independent samples. |
| [EXAMINE](examine.md) | Descriptives, percentiles, M-estimators, extreme values and normality tests, by factor cells. |
| [REGRESSION](regression.md) | Linear regression with forced entry or removal and stepwise selection, residuals and diagnostics. |
| [RELIABILITY](reliability.md) | Reliability of a scale: Cronbach's alpha, split-half and other models, item statistics and ICC. |
| [FACTOR](factor.md) | Factor analysis: principal components and six factoring methods, rotations, factor scores. |
| [QUICK CLUSTER](quick-cluster.md) | K-means clustering of cases into a given number of clusters. |
| [CLUSTER](cluster.md) | Hierarchical clustering of cases by one of 37 proximity measures and seven methods. |
| [DISCRIMINANT](discriminant.md) | Linear discriminant analysis with direct or stepwise entry and classification of cases. |
| [LOGISTIC REGRESSION](logistic-regression.md) | Binary logistic regression with categorical covariates, stepwise selection and case diagnostics. |
| [NOMREG](nomreg.md) | Multinomial logistic regression: a nominal dependent variable on factors and covariates. |
| [UNIANOVA](unianova.md) | General linear model of one dependent variable: factorial ANOVA, ANCOVA and regression. |
| [GLM](glm.md) | General linear model: one dependent variable as in UNIANOVA, or repeated measures with WSFACTOR. |
| [ANOVA](anova.md) | Analysis of variance for factorial designs with covariates, the older form with MCA tables. |
| [TREE](tree.md) | A classification tree grown by CHAID or exhaustive CHAID. |
| [GRAPH](graph.md) | One chart: summaries by category as bars, lines, areas, a pie, points with error bars, a Pareto chart or ranges, a histogram or a scatterplot. |
| [REPORT](report.md) | Case listings and summary statistics by break groups, laid out as a report. |
| [MATRIX](matrix.md) | Run a program in the matrix language, up to END MATRIX. |

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
| [FILE HANDLE](file-handle.md) | Give a file or a folder a short name to use in place of its path. |
| [DATASET NAME](dataset-name.md) | Give the active dataset a name, so that it stays open when another one becomes active. |
| [DATASET ACTIVATE](dataset-activate.md) | Make a named dataset the active one. |
| [DATASET COPY](dataset-copy.md) | Copy the active dataset, as it is now, into a named dataset. |
| [DATASET DECLARE](dataset-declare.md) | Reserve a dataset name for a dataset that a later command creates. |
| [DATASET CLOSE](dataset-close.md) | Close a named dataset, or take the name off the active one. |
| [DATASET DISPLAY](dataset-display.md) | List the open datasets. |
| [USE](use.md) | Use all cases, or a range of cases, in the commands that follow. |
| [DISPLAY](display.md) | Show dictionary information of the active dataset. |
| [LIST](list.md) | List the values of cases. |
| [TITLE](title.md) | Put a title in the output, above the output that follows. |
| [SUBTITLE](subtitle.md) | Put a subtitle in the output, above the output that follows. |
| [ECHO](echo.md) | Show a text in the output. |
| [ERASE](erase.md) | Delete a file. |
| [PERMISSIONS](permissions.md) | Make a file read-only, or writeable for its owner. |
| [HOST](host.md) | Refused: Statscraft never runs operating system commands from syntax. |
| [OUTPUT NEW](output-new.md) | Start a new output document, which gets the tables from now on. |
| [OUTPUT NAME](output-name.md) | Name the output document that gets the tables. |
| [OUTPUT ACTIVATE](output-activate.md) | Send the tables from now on to an open output document. |
| [OUTPUT CLOSE](output-close.md) | Close an output document, or all of them. |
| [OUTPUT DISPLAY](output-display.md) | List the open output documents. |
| [OUTPUT EXPORT](output-export.md) | Write the tables and charts of an output document to a workbook, document, presentation, web page, text or CSV file. |
| [OUTPUT SAVE](output-save.md) | Save an output document as a Statscraft output file (.scout). |
| [OUTPUT OPEN](output-open.md) | Open a Statscraft output file (.scout) as a new output document. |
| [OUTPUT MODIFY](output-modify.md) | Delete, hide, rename, sort, transpose or reformat items of an output document. |
| [FINISH](finish.md) | End the job: the commands after it are not read. |

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
  A function name may be shortened to three or more letters that start no other function.
  Scratch variables start with #: they begin at 0 or blank, keep their value from case to case,
  are never saved and are gone when the next procedure starts. $CASENUM is the case number.

Functions of expressions (COMPUTE, IF, SELECT IF, DO IF, ELSE IF, LOOP IF, END LOOP IF):
  Arithmetic: ABS RND TRUNC MOD SQRT EXP LN LG10 LNGAMMA SIN COS TAN ARSIN ARCOS ARTAN.
  Statistical, with .n for the valid values needed (MEAN.2): SUM MEAN MEDIAN MIN MAX SD VARIANCE
  CFVAR. Missing values: MISSING SYSMIS VALUE NMISS NVALID. Logical: ANY RANGE. Also LAG and
  VALUELABEL.
  Strings: CONCAT LENGTH CHAR.LENGTH UPCASE LOWER LTRIM RTRIM NTRIM SUBSTR CHAR.SUBSTR INDEX
  CHAR.INDEX RINDEX CHAR.RINDEX LPAD CHAR.LPAD RPAD CHAR.RPAD REPLACE STRUNC CHAR.MBLEN
  MBLEN.BYTE NORMALIZE NUMBER STRING.
  Dates and times: DATE.DMY DATE.MDY DATE.MOYR DATE.QYR DATE.WKYR DATE.YRDAY YRMODA TIME.DAYS
  TIME.HMS CTIME.DAYS CTIME.HOURS CTIME.MINUTES CTIME.SECONDS XDATE.DATE XDATE.HOUR XDATE.JDAY
  XDATE.MDAY XDATE.MINUTE XDATE.MONTH XDATE.QUARTER XDATE.SECOND XDATE.TDAY XDATE.TIME
  XDATE.WEEK XDATE.WKDAY XDATE.YEAR DATEDIFF DATESUM (units years quarters months weeks days
  hours minutes seconds; DATESUM methods 'closest', the default, and 'rollover').
  Distributions: CDFNORM PROBIT, and CDF.d IDF.d PDF.d NCDF.d NPDF.d SIG.d RV.d where the
  distribution d has them: BERNOULLI BETA BINOM BVNOR CAUCHY CHISQ EXP F GAMMA GEOM HALFNRM HYPER
  IGAUSS LAPLACE LNORMAL LOGISTIC NEGBIN NORMAL PARETO POISSON SMOD SRANGE T UNIFORM WEIBULL
  (NCDF and NPDF: BETA CHISQ F T; SIG: CHISQ F). UNIFORM(max), NORMAL(sd) and the RV functions
  draw random numbers. An argument out of range gives system-missing.
```
