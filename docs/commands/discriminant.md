<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# DISCRIMINANT

Linear discriminant analysis with direct or stepwise entry and classification of cases.

## Syntax

```text
DISCRIMINANT GROUPS=var(min,max) /VARIABLES=varlist [/SELECT=var(value)]
   [/ANALYSIS=varlist[(level)] ...] [/METHOD=DIRECT | WILKS | MAHAL | MAXMINF | MINRESID | RAO]
   [/TOLERANCE=0.001] [/MAXSTEPS=n] [/FIN=3.84] [/FOUT=2.71] [/PIN=p] [/POUT=p] [/VIN=0]
   [/FUNCTIONS=n1[,pct[,sig]]] [/PRIORS=EQUAL | SIZE | values] [/SAVE=[CLASS[=name]] [SCORES[=root]] [PROBS[=root]]]
   [/ANALYSIS=... another block] [/MISSING=EXCLUDE | INCLUDE] [/HISTORY=STEP | NONE]
   [/CLASSIFY=[NONMISSING | UNSELECTED | UNCLASSIFIED] [POOLED | SEPARATE] [MEANSUB]]
   [/STATISTICS=[MEAN] [STDDEV] [UNIVF] [COV] [CORR] [FPAIR] [BOXM] [GCOV] [TCOV] [RAW] [COEFF] [TABLE]
                [CROSSVALID] [ALL]] [/ROTATE=NONE | COEFF | STRUCTURE] [/MATRIX=IN(file | dataset | *)] [/MATRIX=OUT(file | dataset | *)] [/PLOT=[COMBINED] [SEPARATE] [MAP] [CASES[(n)]] [ALL]].
```

The analysis uses the cases with a whole group value in the range, no missing value on VARIABLES and, with SELECT, the selected value; groups without cases are ignored. DIRECT enters the variables that pass the tolerance; the stepwise methods enter the variable with the best criterion (smallest Wilks' lambda, largest smallest Mahalanobis distance or F between groups, smallest sum of unexplained variation, largest increase in Rao's V) among those passing FIN or PIN, higher inclusion levels first (even levels together, level 0 never), and remove a level 1 variable below FOUT or above POUT.

Tables: Analysis Case Processing Summary, Group Statistics, Tests of Equality of Group Means (UNIVF), Pooled Within-Groups Matrices (COV, CORR), Covariance Matrices (GCOV, TCOV), Log Determinants and Box's M (BOXM), the stepwise tables (HISTORY), Pairwise Group Comparisons (FPAIR), Eigenvalues, Wilks' Lambda, Standardized Canonical Discriminant Function Coefficients, Structure Matrix, Canonical Discriminant Function Coefficients (RAW), Functions at Group Centroids, Classification Processing Summary, Prior Probabilities for Groups, Classification Function Coefficients (COEFF) and Classification Results (TABLE, CROSSVALID leaving each case out). SAVE adds the predicted group, the function scores and the posterior probabilities, named DSC\_m and DSCn\_m unless given. Weighted when a weight variable is set. PLOT alone means COMBINED and CASES. COMBINED plots the classified cases on the first two functions by group, with the group centroids; SEPARATE makes one such plot per group; with a single function both give histograms of the scores. CASES lists every classified case (the first n) in a Casewise Statistics table: actual group, the two most likely groups with their posterior probabilities and squared distances to the centroid, the chi-square significance of the first one's distance, and the scores (with CROSSVALID a second block repeats the cases in the analysis for the rule refitted without each). MAP splits the plane of the first two functions into the territory of each group, a convex polygon, under the pooled rule with the priors, drawn as tinted polygons with the centroids named; there is no map with one function or CLASSIFY=SEPARATE. ROTATE=COEFF or STRUCTURE adds a varimax rotation of the functions: the transformation matrix with the share of the between-groups variance of each rotated function, the rotated standardized coefficients and the rotated structure matrix; scores and classification stay as they are. MATRIX=OUT writes the group counts and means with the pooled deviations and correlations of the first analysis. MATRIX=IN reads such materials (MEAN, N or COUNT, STDDEV and CORR records): with IN(\*), the active dataset being the matrix file, the analysis is made from them alone, so the tables that need cases (the case and classification summaries, classification results, CASES, PLOT of cases, SAVE, CROSSVALID) are not made, and with the pooled records the group deviations shown are the pooled ones; with the name of a file or dataset, the cases of the active dataset are classified by the functions of the matrix. With BOXM, GCOV or CLASSIFY=SEPARATE, MATRIX=OUT writes each group's own STDDEV and CORR records (with the group value), and MATRIX=IN needs such records when one of them is given, and pooled ones when none is.

## Example

```text
DISCRIMINANT GROUPS=segment(1,3) /VARIABLES=q1 TO q6 /METHOD=WILKS /PRIORS=SIZE /STATISTICS=MEAN STDDEV UNIVF BOXM TABLE /SAVE=CLASS SCORES PROBS.
```

`statscraft help DISCRIMINANT` prints the same text in the terminal. [All commands](README.md)
