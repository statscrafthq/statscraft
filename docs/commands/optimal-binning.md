<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# OPTIMAL BINNING

Bins of scale variables: cuts chosen by MDLP against a guide variable, or by equal frequency.

## Syntax

```text
OPTIMAL BINNING /VARIABLES [GUIDE=var] BIN=varlist [SAVE=NO | YES [(INTO=new varlist)]]
                [/CRITERIA [PREPROCESS=EQUALFREQ[(BINS=1000)] | NONE] [METHOD=MDLP | EQUALFREQ[(BINS=10)]]
                           [LOWEREND=UNBOUNDED | OBSERVED] [UPPEREND=UNBOUNDED | OBSERVED]
                           [LOWERLIMIT=INCLUSIVE | EXCLUSIVE] [FORCEMERGE=0]]
                [/MISSING SCOPE=PAIRWISE | LISTWISE] [/OUTFILE RULES='file']
                [/PRINT [ENDPOINTS] [DESCRIPTIVES] [ENTROPY] [NONE]].
```

Every word is written in full and each subcommand given once. MDLP, the default when GUIDE names a numeric or string variable (at most 256 values), splits a range of values in two at the point that best separates the guide's values, keeps the split when its information gain beats the minimum description length cost, and goes on in both halves. PREPROCESS first groups a variable with more distinct values than BINS into that many equal frequency groups, and MDLP splits only between groups. EQUALFREQ, the default without GUIDE, cuts at the 1/n, 2/n and further percentiles (empirical distribution with averaging); a variable with at most n distinct values gets a bin for each. FORCEMERGE r joins an inner bin smaller than r times its smaller neighbour to the neighbour that leaves the lower class entropy, in one pass. End points are data values: with LOWERLIMIT=INCLUSIVE a bin runs from its lower end point up to but not including its upper one, with EXCLUSIVE from above its lower one up to and including its upper one; the outer ends are unbounded, or the observed minimum and maximum with OBSERVED.

PAIRWISE uses each variable's valid cases (with a valid guide for MDLP), LISTWISE the cases valid on all of them; user-missing values are missing. Weights are rounded to whole numbers.

Tables: End Points with the cases of each bin (ENDPOINTS, the default), Descriptive Statistics and Guide Variable Distribution (DESCRIPTIVES), Model Entropy (ENTROPY, MDLP only). SAVE adds the bin numbers of every case with a valid value as guide\_var\_bin (var\_bin without a guide) or the INTO names. RULES writes syntax with the same bins for other data. SPLIT FILE is not used.

## Example

```text
OPTIMAL BINNING /VARIABLES GUIDE=churned BIN=tenure spend SAVE=YES
  /CRITERIA FORCEMERGE=0.05 /OUTFILE RULES='bins.sps' /PRINT ENDPOINTS ENTROPY.
```

`statscraft help OPTIMAL BINNING` prints the same text in the terminal. [All commands](README.md)
