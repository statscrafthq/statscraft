<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# TREE

A decision tree grown by CHAID, exhaustive CHAID, CRT or QUEST, with gain tables, rules and saved predictions.

## Syntax

```text
TREE dependent [level] BY predictor [level] ... [FORCE = var [level]]
     [/METHOD TYPE=CHAID | EXHAUSTIVECHAID | CRT | QUEST  MAXSURROGATES=AUTO | n  PRUNE=NONE | SE[(b)]]
     [/GROWTHLIMIT MAXDEPTH=AUTO | n  MINPARENTSIZE=100  MINCHILDSIZE=50  MINIMPROVEMENT=0.0001]
     [/CHAID ALPHASPLIT=.05 ALPHAMERGE=.05 CHISQUARE=PEARSON | LR CONVERGE=.001
             MAXITERATIONS=100 ADJUST=BONFERRONI | NONE INTERVALS=10 | varlist(n) ...]
     [/CRT IMPURITY=GINI | TWOING | ORDEREDTWOING  MINIMPROVEMENT=0.0001]  [/QUEST ALPHASPLIT=.05]
     [/PRIORS FROMDATA | EQUAL | CUSTOM=value [prior] ...  ADJUST=NO | YES]
     [/COSTS EQUAL | CUSTOM=actual predicted [cost] ...]  [/SCORES EQUALINCREMENTS | CUSTOM=value [score] ...]
     [/PROFITS CUSTOM=category [revenue expense] ...]
     [/DEPCATEGORIES USEVALUES=[VALID MISSING value ...] TARGET=[value ...]]
     [/VALIDATION TYPE=NONE | SPLITSAMPLE(percent | var) | CROSSVALIDATION(folds)  OUTPUT=BOTHSAMPLES | TESTSAMPLE]
     [/MISSING NOMINALMISSING=MISSING | VALID]
     [/TREE DISPLAY=TOPDOWN | NONE  BRANCHSTATISTICS=YES | NO  NODEDEFS=YES | NO]
     [/PRINT=MODELSUMMARY CLASSIFICATION RISK CPS TREETABLE IMPORTANCE SURROGATES | NONE]
     [/GAIN SUMMARYTABLE=YES | NO  CATEGORYTABLE=YES | NO  TYPE=[NODE PTILE]  SORT=DESCENDING | ASCENDING
            INCREMENT=10  CUMULATIVE=YES | NO]
     [/PLOT GAIN RESPONSE INDEX MEAN PROFIT ROI IMPORTANCE INCREMENT=10]
     [/RULES NODES=TERMINAL | ALL | TOPN(n) | TOPPCT(p) | MININDEX(v) | NONE  SYNTAX=INTERNAL | SQL | GENERIC
             TYPE=SCORING | SELECTION  SURROGATES=INCLUDE | EXCLUDE  LABELS=YES | NO  OUTFILE='file']
     [/SAVE NODEID[(name)] PREDVAL[(name)] PREDPROB[(root)]].
```

The command splits the cases, again and again, by the predictor that best separates the values of the dependent, then shows the tree, its risk and how well it classifies. METHOD TYPE picks the method: CHAID (the default) and EXHAUSTIVECHAID may split a node into more than two children, CRT and QUEST always split in two. A level after a variable sets how it is read: \[S\] scale, \[O\] ordinal, \[N\] or \[C\] nominal. Without one the dictionary's level holds, and a variable with no level gets one from its values. The default PRINT is MODELSUMMARY CLASSIFICATION RISK CPS.

CHAID takes a nominal dependent (Pearson chi-square, or the likelihood ratio with CHISQUARE=LR), an ordinal one (row effects model, scored 1, 2, ... or by SCORES CUSTOM) or a scale one (F test). Scale predictors are cut into INTERVALS groups of about equal size (at most 64; varlist(n) sets n per variable). Categories of a predictor that do not differ at ALPHAMERGE are merged, ordinal ones only with a neighbor; exhaustive CHAID merges down to two and keeps the most significant grouping. A missing value is a category that may join any other. A category smaller than MINCHILDSIZE joins its closest neighbor. The predictor with the smallest Bonferroni-adjusted p-value (ADJUST=NONE: unadjusted) splits the node if that is at most ALPHASPLIT. FORCE=var splits the root by var.

A node is not split when it is pure, holds fewer than MINPARENTSIZE cases or is at MAXDEPTH (AUTO: 3 for CHAID, 5 for CRT and QUEST). MINCHILDSIZE must be less than MINPARENTSIZE. A weight counts each case as often as its rounded value. SPLIT FILE is ignored.

CRT takes a nominal, ordinal or scale dependent. It tries every cut of a scale or ordinal predictor (x \<= v goes left) and every split of a nominal one's categories in two, and keeps the split that lowers the impurity most. IMPURITY is GINI, TWOING or ORDEREDTWOING (ordinal dependent only). A node also stops when the best improvement is below MINIMPROVEMENT. For a scale dependent the impurity is the variance within the node, a node predicts its mean, the risk is the mean squared deviation, and PRIORS, COSTS and the class tables do not apply.

QUEST needs a nominal dependent and reads scale and ordinal predictors as numbers. A node splits on the predictor whose test against the classes (F or chi-square) has the smallest p-value, if that is below ALPHASPLIT divided by the number of predictors; otherwise a test of spread may still split it. The cut comes from a discriminant analysis of the classes grouped in two. The tree table shows each split's test, p-value and cut.

For CRT and QUEST, PRIORS FROMDATA (the default), EQUAL or CUSTOM=value \[prior\] ... gives the class probabilities that decide each node's class and weigh the risk. A case missing the split variable follows the first surrogate it has a value for (a split of another predictor that mimics the main one), else the larger child. MAXSURROGATES caps the surrogates (AUTO: one fewer than the predictors; 0: none), and PRINT SURROGATES lists them. PRUNE=SE(b) (b is 1 when left out) cuts the tree back to the smallest subtree whose risk is within b standard errors of the lowest. PRINT and PLOT IMPORTANCE (CRT only) show each predictor's improvement relative to the largest.

COSTS CUSTOM=actual predicted \[cost\] ... sets the cost of predicting one category for a case of another: 1 when not given, and a correct prediction always costs 0. A node gets the category with the least expected cost, and the risk is the mean cost. CRT with GINI also grows the tree by the costs. PRIORS ADJUST=YES instead folds the costs into the priors and then ignores them. A setting that does not apply to the method or the dependent, such as PRIORS under CHAID or SCORES under CRT, is ignored with a warning.

VALIDATION TYPE=SPLITSAMPLE(percent) grows the tree on a random share of the cases (50 when no number is given; SET SEED repeats it) and tests it on the rest; SPLITSAMPLE(var) trains on the cases where var is 1. Risk, classification and gains are shown for both samples (OUTPUT=TESTSAMPLE: the test sample only), and PRUNE uses the test sample's risk. CROSSVALIDATION(folds) (2 to 25, default 10) adds a cross-validated risk without changing the tree; it is ignored with FORCE.

The tree comes as chart data: each node with its parent, the condition that leads to it, its counts, its predicted value and its split. DISPLAY=NONE leaves it out, and TREETABLE lists the nodes as a table. USEVALUES lists the classes of the dependent (VALID: every valid value; user-missing values of a nominal dependent may be named). NOMINALMISSING=VALID makes user-missing values of nominal predictors valid categories.

Gain tables, one per TARGET category, list the terminal nodes by index: size, target count, response (the target share in the node) and index (response over the overall share), cumulative unless CUMULATIVE=NO. TYPE=\[NODE PTILE\] adds percentile groups INCREMENT percent wide (1, 2, 5, 10, 20 or 25; default 10). For a scale dependent SUMMARYTABLE lists the nodes by mean. PLOT GAIN, RESPONSE and INDEX chart the percentile groups. PROFITS CUSTOM=category \[revenue expense\] ... adds the average profit and the return on investment to the summary, charted by PLOT PROFIT and ROI. PLOT MEAN charts a scale dependent.

RULES writes rules for the terminal nodes (NODES=ALL: every node, parents first). SYNTAX=INTERNAL gives DO IF blocks that compute NodeID, PredictedValue and PredictedProbability, SQL gives CASE expressions, GENERIC gives sentences with labels (LABELS=NO: names and values). TYPE=SELECTION gives one rule that selects the cases of the nodes. A missing value follows the surrogates (SURROGATES=INCLUDE), then the larger child. TOPN(n), TOPPCT(p) and MININDEX(v) keep the terminal nodes with the highest index. OUTFILE writes the rules to a text file.

SAVE adds the terminal node, the predicted value and one probability per category (default names NodeID, PredictedValue and PredictedProbability\_1, ...), for test cases too. A scale dependent saves the node mean as PREDVAL. DMTREE is another name for the command.

## Example

```text
GET FILE='survey.sav'.
TREE recommend [n] BY satisfaction [o] region [n] age_group [o] contacted [n]
  /GROWTHLIMIT MAXDEPTH=2 MINPARENTSIZE=40 MINCHILDSIZE=20 /SAVE NODEID PREDVAL.
TREE switching [n] BY satisfaction [o] region age_group spend [s]
  /METHOD TYPE=CRT /GROWTHLIMIT MAXDEPTH=3 /PRINT=MODELSUMMARY RISK IMPORTANCE.
```

`statscraft help TREE` prints the same text in the terminal. [All commands](README.md)
