<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# CLUSTER

Hierarchical clustering of cases by one of 37 proximity measures and seven methods.

## Syntax

```text
CLUSTER varlist | /MATRIX=IN('file' | dataset | *)
        [/MEASURE=SEUCLID | EUCLID | COSINE | CORRELATION | BLOCK | CHEBYCHEV | MINKOWSKI(p) |
                  POWER(p,r) | CHISQ | PH2 | binary[(p[,np])]]
        [/METHOD=BAVERAGE | WAVERAGE | SINGLE | COMPLETE | CENTROID | MEDIAN | WARD [(rootname)] ...]
        [/PRINT=[SCHEDULE] [CLUSTER(min[,max])] [DISTANCE] [NONE]] [/SAVE=CLUSTER(min[,max])]
        [/ID=var] [/MISSING=EXCLUDE | INCLUDE] [/PLOT=[VICICLE | HICICLE[(min,max,inc)]] [DENDROGRAM] [NONE]].
```

binary: RR SM JACCARD DICE SS1 RT SS2 K1 SS3 K2 SS4 HAMANN OCHIAI SS5 PHI LAMBDA D Y Q BEUCLID SIZE PATTERN BSEUCLID BSHAPE DISPER VARIANCE BLWMN, from the 2 x 2 table of presence (p, default 1) and absence (np, default 0; with p alone every other value is absence). The items are the cases valid on every variable; the weight variable is not used. Each step joins the most similar pair of clusters (smallest distance or largest similarity) into the lower-numbered one and updates the proximities by the formula of the method; BAVERAGE (default) and WAVERAGE choose by the mean proximity between or within the clusters. Tables: Case Processing Summary, Proximity Matrix (DISTANCE), and for each method the Agglomeration Schedule (default; for WARD the coefficient is the within-cluster sum of squares) and Cluster Membership (CLUSTER), the clusters of a solution numbered in the order of their first case. ID labels the cases. SAVE adds the membership of each solution, named rootname plus the number of clusters, or CLUn\_m. MATRIX=IN reads the rows of type PROX of a matrix file, a dissimilarity matrix unless the value label of PROX says SIMILARITY. Plots come as chart data for each method; a vertical icicle plot unless PLOT names others. The icicle plot shows the cluster of every case at each number of clusters, from min to max by inc (default 1 to the number of cases minus 1). The dendrogram holds every join with its coefficient, rescaled from 1 at the first join to 25 at the last. Not supported yet: MATRIX=OUT, SAVE with MATRIX=IN.

## Example

```text
CLUSTER q1 TO q5 /MEASURE=SEUCLID /METHOD=WARD /PRINT=SCHEDULE CLUSTER(2,4) /SAVE=CLUSTER(3).
```

`statscraft help CLUSTER` prints the same text in the terminal. [All commands](README.md)
