<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# RATIO STATISTICS

Statistics of the ratio of two positive variables, overall and within groups.

## Syntax

```text
RATIO STATISTICS numerator WITH denominator [BY group [(ASCENDING | DESCENDING | NOSORT)]]
                 [/MISSING=EXCLUDE | INCLUDE] [/PRINT=statistic ...]
                 [/OUTFILE('file or dataset')=statistic ...].
```

statistic: MEAN MEDIAN WGTMEAN MIN MAX RANGE STDDEV AAD COD PRD MNCOV MDCOV CIN\[(95)\] BCOC((low, high) ...) WCOC(percent ...) A case counts when both variables are valid and above 0 and its weight, rounded to a whole number, is above 0. WGTMEAN is the sum of numerators over the sum of denominators, PRD the mean over WGTMEAN. AAD is the mean distance from the median and COD that in percent of the median; MNCOV is the standard deviation in percent of the mean, MDCOV the root mean square distance from the median (over N - 1) in percent of the median. BCOC is the percent of ratios from low to high, ends included; WCOC the percent within that many percent of the median. CIN adds intervals to MEAN and WGTMEAN (t with N - 1 df, a delta method variance for WGTMEAN) and to MEDIAN (from order statistics, with the actual coverage, at least the level asked). Without PRINT only the case processing summary shows. The groups may include a missing value. OUTFILE writes the overall record first, the group variable empty, then one record per group; it is not available under SPLIT FILE. A subcommand without content is ignored.

## Example

```text
RATIO STATISTICS rent WITH income BY district (DESCENDING)
  /PRINT=MEDIAN COD PRD WCOC(10 25) CIN(90).
```

`statscraft help RATIO STATISTICS` prints the same text in the terminal. [All commands](README.md)
