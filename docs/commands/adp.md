<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# ADP

Prepares a target and its inputs for modelling and writes the preparation as syntax.

## Syntax

```text
ADP /FIELDS [TARGET=var] INPUT=varlist [ANALYSISWEIGHT=var]
    [/PREPDATETIME [DATEDURATION=YES[(REFERENCE=CURRENT | YMD('y-m-d') UNIT=AUTO | YEARS | MONTHS | DAYS[(SUFFIX='s')])] | NO]
                   [TIMEDURATION=YES[(REFERENCE=CURRENT | HMS('h:m:s') UNIT=AUTO | HOURS | MINUTES | SECONDS[(SUFFIX='s')])] | NO]
                   [EXTRACTYEAR | EXTRACTMONTH | EXTRACTDAY | EXTRACTHOUR | EXTRACTMINUTE | EXTRACTSECOND=NO | YES[(SUFFIX='s')]]]
    [/SCREENING [PCTMISSING=YES[(MAXPCT=50)] | NO] [UNIQUECAT=YES[(MAXCAT=100)] | NO] [SINGLECAT=NO | YES[(MAXPCT=95)]]]
    [/ADJUSTLEVEL [INPUT=YES | NO] [TARGET=YES | NO] [MAXVALORDINAL=10] [MINVALCONTINUOUS=5]]
    [/OUTLIERHANDLING [INPUT=YES | NO] [TARGET=YES | NO] [CUTOFF=SD(3)] [REPLACEWITH=CUTOFFVALUE | MISSING]]
    [/REPLACEMISSING [INPUT=YES[(EXCLUDE([CONTINUOUS] [NOMINAL] [ORDINAL]))] | NO] [TARGET=NO | YES[(EXCLUDE(...))]]]
    [/REORDERNOMINAL [INPUT=NO | YES] [TARGET=NO | YES]]
    [/RESCALE [INPUT=ZSCORE[(MEAN=0 SD=1)] | MINMAX[(MIN=0 MAX=100)] | NONE] [TARGET=BOXCOX[(MEAN=0 SD=1)] | NONE]]
    [/TRANSFORM [MERGESUPERVISED=YES[(PVALUE=0.05)] | NO] [MERGEUNSUPERVISED=NO | YES[([ORDINAL] [NOMINAL] [MINPCT=10])]]
                [BINNING=SUPERVISED[(PVALUE=0.05)] | NONE] [SELECTION=YES[(PVALUE=0.05)] | NO] [CONSTRUCTION=YES[(ROOT=feature)] | NO]]
    [/CRITERIA SUFFIX([TARGET='_transformed'] [INPUT='_transformed'])]
    /OUTFILE PREPXML='file'.
```

Every word is written in full and each subcommand given once. ADP looks at the data and writes syntax (RECODE, COMPUTE, IF and VARIABLE LEVEL) that makes a prepared copy of each field it changes, named with the suffix; run that file to add the fields. The data are not changed by ADP itself, and the file holds syntax rather than XML. The steps, in order: date inputs become the time since the reference date (AUTO: days below 31 days, months of 30.4375 days below 366, else years of 365.25 days) and time inputs the time since the reference time (seconds below 60, minutes below 3600, else hours), with the parts asked for as ordinal fields; the date or time field itself is dropped. Screening drops fields with too many missing values, a single value, one dominant category or too many nominal categories. Levels: continuous fields with few values become ordinal, ordinal fields with many continuous.

Outliers of continuous fields lie more than CUTOFF robust SDs (found after setting aside thin tails of at most 5% of the cases) from the robust mean. Missing values take the mean, median or mode by level. A nominal target can be renumbered by count; a continuous target goes through Box-Cox (lambda -3 to 3 by 0.5) and then to MEAN and SD. Continuous inputs get z scores or min-max. With a target, categories of an input whose relation to the target does not differ at PVALUE are merged (F test, chi-square or the row effects test by the target's level) and an input not related to the target is dropped; continuous inputs are binned at the Scheffe subsets of the target classes' means for a categorical target, or dropped when their correlation with a continuous target is not significant. Without a target, MERGEUNSUPERVISED joins small neighbouring categories until each holds MINPCT of the cases. With a continuous target and CONSTRUCTION=YES the continuous inputs left are grouped by correlation (level 0.9 down to 0.2, at most five to a group) and each group is replaced by its first principal component, written as a COMPUTE into the syntax file (ROOT names them ROOT1, ROOT2, ...). Frequency weights are rounded; analysis weights weigh means and spreads.

SPLIT FILE is not used. The table lists each field, its prepared field and the actions.

## Example

```text
ADP /FIELDS TARGET=churned INPUT=tenure plan region signup
  /PREPDATETIME DATEDURATION=YES(REFERENCE=YMD('2025-01-01') UNIT=MONTHS)
  /OUTFILE PREPXML='prepare.sps'.
```

`statscraft help ADP` prints the same text in the terminal. [All commands](README.md)
