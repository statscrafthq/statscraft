<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# TWOSTEP CLUSTER

Clusters of cases from continuous and categorical variables, the number of clusters chosen by BIC or AIC.

## Syntax

```text
TWOSTEP CLUSTER [/CONTINUOUS VARIABLES=varlist] [/CATEGORICAL VARIABLES=varlist]
                [/DISTANCE=LIKELIHOOD | EUCLIDEAN] [/NUMCLUSTERS=AUTO [15] [BIC | AIC] | FIXED=n]
                [/CRITERIA=[INITHRESHOLD(0)] [MXBRANCH(8)] [MXLEVEL(3)]] [/HANDLENOISE=0]
                [/NOSTANDARDIZE [VARIABLES=varlist]] [/MISSING=EXCLUDE | INCLUDE] [/MEMALLOCATE=64]
                [/PRINT=[IC] [COUNT] [SUMMARY]] [/VIEWMODEL [DISPLAY=YES | NO] [EVALUATIONFIELDS=varlist]]
                [/SAVE CLUSTER [VARIABLE=name]] [/OUTFILE [MODEL='file'] [STATE='file']].
```

Step one reads the cases in file order into a tree of small subclusters: a case joins the nearest subcluster when it lies within the threshold, else it starts its own; a node holds at most MXBRANCH entries and the tree at most MXLEVEL levels below its root. A full tree is rebuilt with a larger threshold; with HANDLENOISE p it first puts aside the subclusters smaller than p percent of the largest, takes back those that later fit and calls the rest noise. Step two joins the subclusters, nearest pair first. LIKELIHOOD (default) measures how much the log-likelihood drops when two groups join, with normal continuous and multinomial categorical variables; EUCLIDEAN, the distance between group means, takes continuous variables only. AUTO compares 1 to 15 clusters (or the maximum given): BIC (default) or AIC gives a first count, at the first step whose change is less than 0.04 of the change from 1 to 2 clusters, and the ratios of the join distances settle it. Then every case goes to its nearest cluster; when the tree filled under HANDLENOISE, a case too far from every cluster goes to cluster -1. Continuous variables are standardized unless NOSTANDARDIZE names them; a constant one is left out.

Tables: Model Summary (unless DISPLAY=NO), Auto-Clustering (IC), Cluster Distribution (COUNT), Centroids and one frequency table per categorical variable (SUMMARY). Cases missing on any variable are left out; the weight variable is not used. SAVE adds the cluster numbers, named TSC\_1, TSC\_2 and so on unless given. The result depends on the order of the cases. DMCLUSTER is another name for the command. Not supported yet: INFILE; OUTFILE writes no file.

## Example

```text
TWOSTEP CLUSTER /CONTINUOUS VARIABLES=spend visits /CATEGORICAL VARIABLES=region plan
  /NUMCLUSTERS=AUTO 8 BIC /PRINT=COUNT SUMMARY IC /SAVE CLUSTER VARIABLE=segment.
```

`statscraft help TWOSTEP CLUSTER` prints the same text in the terminal. [All commands](README.md)
