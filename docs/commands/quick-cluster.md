<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# QUICK CLUSTER

K-means clustering of cases into a given number of clusters.

## Syntax

```text
QUICK CLUSTER varlist | ALL [/MISSING=[LISTWISE | PAIRWISE | DEFAULT] [INCLUDE]]
              [/FILE='file' | dataset] [/INITIAL=(values)]
              [/CRITERIA=[CLUSTER(2)] [NOINITIAL] [MXITER(10)] [CONVERGE(0)]]
              [/METHOD=KMEANS[(NOUPDATE | UPDATE)] | CLASSIFY]
              [/PRINT=[INITIAL] [CLUSTER] [ID(var)] [DISTANCE] [ANOVA] [NONE]]
              [/OUTFILE='file' | dataset] [/SAVE=[CLUSTER[(name)]] [DISTANCE[(name)]]].
```

Distances are Euclidean. Initial centers: by default the first k complete cases, then a case replaces a center when it lies farther from the centers than the two closest centers lie from each other (NOINITIAL keeps the first k cases); INITIAL gives them cluster by cluster, FILE reads them from the clustering variables of a data file. KMEANS moves each center to the mean of its cases after each pass (UPDATE after each case) until no center moves more than CONVERGE times the smallest distance between the initial centers, or MXITER passes; CLASSIFY keeps the initial centers. Then every case goes to its nearest center, and the final centers are the means of their cases. Tables: Initial Cluster Centers, Iteration History, Cluster Membership (CLUSTER, with ID), Final Cluster Centers, Distances between Final Cluster Centers (DISTANCE), ANOVA and Number of Cases in each Cluster. LISTWISE (default) leaves out a case missing on any variable; PAIRWISE uses the variables a case has. SAVE adds the cluster number and the distance to the classification center, named QCL\_1, QCL\_2 and so on unless given. OUTFILE writes the final centers with CLUSTER\_. Weighted when a weight variable is set.

## Example

```text
QUICK CLUSTER q1 TO q5 /CRITERIA=CLUSTER(3) MXITER(20) /PRINT=INITIAL ANOVA /SAVE=CLUSTER(segment) DISTANCE.
```

`statscraft help QUICK CLUSTER` prints the same text in the terminal. [All commands](README.md)
