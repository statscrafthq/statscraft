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
                [CROSSVALID] [ALL]] [/ROTATE=NONE] [/PLOT=[COMBINED] [SEPARATE] [MAP] [CASES[(n)]] [ALL]].
```

The analysis uses the cases with a whole group value in the range, no missing value on VARIABLES and, with SELECT, the selected value; groups without cases are ignored. DIRECT enters the variables that pass the tolerance; the stepwise methods enter the variable with the best criterion (smallest Wilks' lambda, largest smallest Mahalanobis distance or F between groups, smallest sum of unexplained variation, largest increase in Rao's V) among those passing FIN or PIN, higher inclusion levels first (even levels together, level 0 never), and remove a level 1 variable below FOUT or above POUT.

Tables: Analysis Case Processing Summary, Group Statistics, Tests of Equality of Group Means (UNIVF), Pooled Within-Groups Matrices (COV, CORR), Covariance Matrices (GCOV, TCOV), Log Determinants and Box's M (BOXM), the stepwise tables (HISTORY), Pairwise Group Comparisons (FPAIR), Eigenvalues, Wilks' Lambda, Standardized Canonical Discriminant Function Coefficients, Structure Matrix, Canonical Discriminant Function Coefficients (RAW), Functions at Group Centroids, Classification Processing Summary, Prior Probabilities for Groups, Classification Function Coefficients (COEFF) and Classification Results (TABLE, CROSSVALID leaving each case out). SAVE adds the predicted group, the function scores and the posterior probabilities, named DSC\_m and DSCn\_m unless given. Weighted when a weight variable is set. PLOT alone means COMBINED and CASES. COMBINED plots the classified cases on the first two functions by group, with the group centroids; SEPARATE makes one such plot per group; with a single function both give histograms of the scores. CASES lists every classified case (the first n) in a Casewise Statistics table: actual group, the two most likely groups with their posterior probabilities and squared distances to the centroid, the chi-square significance of the first one's distance, and the scores. MAP splits the plane of the first two functions into the territory of each group, a convex polygon, under the pooled rule with the priors, drawn as tinted polygons with the centroids named; there is no map with one function or CLASSIFY=SEPARATE. Not supported yet: ROTATE=COEFF and STRUCTURE, MATRIX, OUTFILE, cross-validated rows in the casewise listing.

## Example

```text
DISCRIMINANT GROUPS=segment(1,3) /VARIABLES=q1 TO q6 /METHOD=WILKS /PRIORS=SIZE /STATISTICS=MEAN STDDEV UNIVF BOXM TABLE /SAVE=CLASS SCORES PROBS.
```

`statscraft help DISCRIMINANT` prints the same text in the terminal. [All commands](README.md)
