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

PLOT GAIN, RESPONSE and INDEX give line charts with one line per TARGET category over cumulative percentile groups INCREMENT percent wide (1, 2, 5, 10, 20 or 25): the terminal nodes in the order of the gain tables, the node that holds a cut counted in proportion. PLOT MEAN, PROFIT and ROI, the QUEST subcommand and PRINT CATEGORYSPECS are accepted and ignored with a warning. So are the settings of CRT (CRT, PRIORS, MAXSURROGATES, PRUNE, PRINT IMPORTANCE and SURROGATES, PLOT IMPORTANCE) under another method, and CHAID, SCORES and FORCE under CRT.

METHOD TYPE=CRT grows a binary tree for a nominal or ordinal dependent: every cut of a scale or ordinal predictor (a value of the data, x \<= v goes left) and every subset of the categories of a nominal one is tried (for more than two classes up to 20 categories present in a node), and the split with the largest improvement wins. /CRT IMPURITY is GINI (the default), TWOING or ORDEREDTWOING (ordinal dependent only); /PRIORS FROMDATA, EQUAL or CUSTOM=value \[prior\] ... sets the class probabilities, which also decide each node's category and the risk. A node stops at MAXDEPTH (AUTO is 5), under MINPARENTSIZE, when no cut leaves MINCHILDSIZE cases on each side or when the best improvement is below MINIMPROVEMENT (default 0.0001). A case with no value of the split variable follows the first surrogate (a split of another predictor that imitates the main one, kept when its association is positive; METHOD MAXSURROGATES gives the most kept, AUTO is one fewer than the predictors, 0 none) it has a value for, and goes to the child with more cases when there is none. PRINT SURROGATES lists them with their association. METHOD PRUNE=SE(b) (b is 1 when omitted) cuts the grown tree back by repeatedly turning its weakest links into terminal nodes and keeps the smallest subtree whose risk is within b standard errors of the lowest risk, all from the risk of the cases analyzed (there is no validation sample yet). PRINT IMPORTANCE and PLOT IMPORTANCE give each predictor's importance: the improvements of the splits of the final tree where it is the main splitter or a surrogate, added up and shown relative to the largest. A scale dependent is not built yet. Not supported yet: METHOD TYPE=QUEST, a scale dependent, VALIDATION SPLITSAMPLE and CROSSVALIDATION, SPLITMERGED=YES, COSTS CUSTOM, GAIN TYPE PTILE, RULES, PROFITS, INFLUENCE, OUTFILE, TARGETRESPONSE.

DMTREE is another name for the command.

## Example

```text
TREE churn [n] BY region [n] age_group [o] income [s] /GROWTHLIMIT MAXDEPTH=2 /SAVE NODEID PREDVAL.
```

`statscraft help TREE` prints the same text in the terminal. [All commands](README.md)
