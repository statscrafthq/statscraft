<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# TREE

A classification tree grown by CHAID or exhaustive CHAID.

## Syntax

```text
TREE dependent [level] BY predictor [level] ... [FORCE = var [level]]
     [/TREE DISPLAY=TOPDOWN | LEFTTORIGHT | RIGHTTOLEFT | NONE  NODES=STATISTICS | CHART | BOTH
            BRANCHSTATISTICS=YES | NO  NODEDEFS=YES | NO  SCALE=AUTO | n]
     [/DEPCATEGORIES USEVALUES=[VALID MISSING value ...] TARGET=[value ...]]
     [/PRINT=MODELSUMMARY CLASSIFICATION RISK CPS TREETABLE | NONE]
     [/GAIN CATEGORYTABLE=YES | NO  TYPE=[NODE]  SORT=DESCENDING | ASCENDING  CUMULATIVE=YES | NO]
     [/SAVE NODEID[(name)] PREDVAL[(name)] PREDPROB[(root)]]
     [/METHOD TYPE=CHAID | EXHAUSTIVECHAID]  [/VALIDATION TYPE=NONE]
     [/GROWTHLIMIT MAXDEPTH=AUTO | n  MINPARENTSIZE=100  MINCHILDSIZE=50]
     [/CHAID ALPHASPLIT=.05 ALPHAMERGE=.05 SPLITMERGED=NO CHISQUARE=PEARSON | LR CONVERGE=.001
             MAXITERATIONS=100 ADJUST=BONFERRONI | NONE INTERVALS=10 | varlist(n) ...]
     [/COSTS EQUAL] [/SCORES EQUALINCREMENTS | CUSTOM=value [score] ...]
     [/MISSING NOMINALMISSING=MISSING | VALID]
     [/PLOT GAIN RESPONSE INDEX INCREMENT=10].
```

Levels: \[S\] scale, \[O\] ordinal, \[N\] or \[C\] nominal; without one the dictionary's level holds, and an unknown level comes from the data as in a data pass. The dependent is nominal (Pearson or likelihood-ratio chi-square) or ordinal (likelihood ratio against Goodman's row effects model, scored 1, 2, ... or by SCORES). Scale predictors are cut into INTERVALS groups of about equal size. Predictor categories that do not differ at ALPHAMERGE are merged (ordinal ones only when adjacent); a missing value is a category of its own that may merge with any. A category smaller than MINCHILDSIZE is merged with its most similar neighbor. The predictor with the smallest Bonferroni-adjusted significance splits the node when that is at most ALPHASPLIT. A node is not split when it is pure, smaller than MINPARENTSIZE or at MAXDEPTH (AUTO is 3). Exhaustive CHAID merges down to two categories and keeps the most significant set. The tree diagram comes as chart data: every node with its parent, the condition that leads to it, its category counts and percentages, its share of all cases, its predicted category and the split below it. TREETABLE lists the nodes in order with their parents. Gain tables (one per TARGET category) show terminal nodes by index.

SAVE adds the terminal node, the predicted category and one probability per category for the cases in the analysis (default names NodeID, PredictedValue, PredictedProbability\_1, ...).

A weight counts each case as often as its rounded value. SPLIT FILE is ignored.

PLOT GAIN, RESPONSE and INDEX give line charts with one line per TARGET category over cumulative percentile groups INCREMENT percent wide (1, 2, 5, 10, 20 or 25): the terminal nodes in the order of the gain tables, the node that holds a cut counted in proportion. PLOT MEAN, PROFIT, ROI and IMPORTANCE, PRIORS, CRT, QUEST, the settings of CRT and QUEST and PRINT IMPORTANCE, SURROGATES and CATEGORYSPECS are accepted and ignored with a warning. Not supported yet: METHOD TYPE=CRT and QUEST, a scale dependent, VALIDATION SPLITSAMPLE and CROSSVALIDATION, SPLITMERGED=YES, COSTS CUSTOM, GAIN TYPE PTILE, RULES, PROFITS, INFLUENCE, OUTFILE, TARGETRESPONSE.

## Example

```text
TREE churn [n] BY region [n] age_group [o] income [s] /GROWTHLIMIT MAXDEPTH=2 /SAVE NODEID PREDVAL.
```

`statscraft help TREE` prints the same text in the terminal. [All commands](README.md)
