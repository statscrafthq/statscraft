<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# ALSCAL

Multidimensional scaling of proximity matrices, symmetric or asymmetric, by alternating least squares on squared distances.

## Syntax

```text
ALSCAL VARIABLES=varlist | /MATRIX=IN('file' | dataset | *)
   [/INPUT=ROWS(ALL | n)] [/SHAPE=SYMMETRIC | ASYMMETRIC]
   [/LEVEL=ORDINAL[([UNTIE] [SIMILAR])] | INTERVAL[(1..4)] | RATIO[(1..4)] | NOMINAL]
   [/CONDITION=MATRIX | UNCONDITIONAL] [/MODEL=EUCLID | INDSCAL]
   [/CRITERIA=[NEGATIVE] [CUTOFF(0)] [CONVERGE(.001)] [ITER(30)] [STRESSMIN(.005)] [DIMENS(2[,max])]]
   [/PRINT=[DATA] [HEADER]] [/PLOT[=DEFAULT | ALL]] [/OUTFILE='file' | dataset]
   [/FILE='file' | dataset [CONFIG | ROWCONF] [SUBJWGHT] [(INITIAL)]].
```

Each case is a row of a square matrix of dissimilarities and each variable a column; only the triangle below the diagonal is read (SHAPE=ASYMMETRIC reads both), and more cases than variables hold one matrix per subject. With SHAPE=ASYMMETRIC the distances stay symmetric and each ordered pair of stimuli is a datum of its own: the disparities, S-stress, stress and RSQ run over all of them, a pair given in one triangle only counts once, and the start averages the two triangles.

MATRIX=IN reads a matrix file of PROXIMITIES or CLUSTER instead, VARIABLES picking its items; a similarity file is scaled as SIMILAR, or turned into dissimilarities from its largest value for INTERVAL and RATIO. A missing value or one below CUTOFF is left out; the weight variable is not used. The model fits squared distances to squared disparities and reports S-stress, the root of the mean over the matrices of their squared misfit relative to the fourth powers of the disparities. ORDINAL keeps the order of the data (UNTIE lets ties part, SIMILAR turns the order), NOMINAL only which values are equal; INTERVAL(n) and RATIO(n) make the squared disparities a polynomial of degree n of the squared data, with and without an intercept. CONDITION=MATRIX fits each matrix on its own, UNCONDITIONAL all at once. MODEL=INDSCAL weights the dimensions per matrix (no negative weights unless NEGATIVE). The start is classical scaling of the mean scalar products of all matrices, after each got the smallest constant that makes it a metric. Iterations stop when S-stress improves by less than CONVERGE, falls to STRESSMIN or after ITER; DIMENS(min,max) solves from max down to min (1 only for EUCLID), each smaller solution from the first dimensions of the one before.

Tables per dimensionality: Iteration History (S-stress and its improvement), Stress and Squared Correlation (RSQ) in Distances per matrix (Kruskal's stress formula 1 and the squared correlation of disparities and distances, with their average), Stimulus Coordinates and for INDSCAL Subject Weights; PRINT=DATA adds the data and the optimally scaled data, HEADER the options. The coordinates are centered; EUCLID turns them to principal axes with a mean square of 1, INDSCAL scales each dimension to a sum of squares equal to the number of stimuli (the weights the other way) and orders the dimensions by their weights; every dimension is signed so that the first stimulus is positive. PLOT gives the configuration, the subject weights, the linear fit (disparities against distances), the nonlinear fit (data against distances) for ORDINAL and NOMINAL and the transformation (data against disparities), with ALL one per matrix. OUTFILE writes TYPE\_ (CON for coordinates, SUB for subject weights), DIMENS, MATNUM (0 for coordinates) and DIM1, DIM2, ... for every dimensionality. FILE reads such a file (or any with TYPE\_, DIM1, DIM2, ...) to start the iterations from: the first rows of TYPE\_ CON or ROW with a value in the last dimension, one for each stimulus, are the starting coordinates of the largest dimensionality, and with MODEL=INDSCAL the TYPE\_ SUB rows, one for each matrix, the starting weights (they are read when SUBJWGHT is listed, or when nothing is listed). The words may be cut to three letters; only INITIAL is supported, not FIXED.

## Example

```text
ALSCAL VARIABLES=s1 TO s9 /LEVEL=ORDINAL /CRITERIA=DIMENS(2,3) /PLOT.
```

`statscraft help ALSCAL` prints the same text in the terminal. [All commands](README.md)
