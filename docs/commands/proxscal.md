<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# PROXSCAL

Multidimensional scaling of one or more proximity matrices by majorization, with optimal transformations.

## Syntax

```text
PROXSCAL varlist | /MATRIX=IN('file' | dataset | *)
   [/TABLE=rowvar BY colvar [BY sourcevar] | sourcevar] [/SHAPE=LOWER | UPPER | BOTH]
   [/INITIAL=SIMPLEX | TORGERSON | RANDOM[(n)]] [/WEIGHTS=varlist] [/CONDITION=MATRIX | UNCONDITIONAL]
   [/TRANSFORMATION=RATIO | INTERVAL | ORDINAL[(KEEPTIES | UNTIE)] | SPLINE [DEGREE=2] [INKNOT=1]]
   [/PROXIMITIES=DISSIMILARITIES | SIMILARITIES] [/MODEL=IDENTITY | WEIGHTED | GENERALIZED | REDUCED[(2)]]
   [/ACCELERATION=NONE] [/CRITERIA=[DIMENSIONS(2[,max])] [MAXITER(100)] [DIFFSTRESS(.0001)] [MINSTRESS(.0001)]]
   [/PRINT=[NONE] [INPUT] [RANDOM] [HISTORY] [STRESS] [DECOMPOSITION] [COMMON] [DISTANCES] [WEIGHTS]
           [INDIVIDUAL] [TRANSFORMATIONS]]
   [/PLOT=[NONE] [STRESS] [COMMON] [WEIGHTS] [INDIVIDUAL[(sources | ALL)]] [TRANSFORMATIONS[(sources | ALL)]]
          [RESIDUALS[(sources | ALL)]]]
   [/OUTFILE=[COMMON('file' | dataset)] [WEIGHTS(...)] [DISTANCES(...)] [TRANSFORMATIONS(...)]].
```

The proximities are dissimilarities unless PROXIMITIES=SIMILARITIES. Without TABLE each case is a row of a square matrix and each variable a column; more cases than variables hold several matrices, one after the other, each a source. TABLE row BY column \[BY source\] takes one proximity per case, each variable a source or the source in a third variable; TABLE=source names the matrix of each case.

SHAPE picks the triangle below or above the diagonal, or BOTH for the weighted mean of the two; the diagonal is not used. WEIGHTS gives each proximity its weight; a missing proximity or weight counts as weight 0. MATRIX=IN reads a matrix file of PROXIMITIES or CLUSTER as one source.

The scaling makes the distances fit the transformed proximities in least squares (normalized raw stress) by majorization. RATIO keeps the proximities as they are (INTERVAL is used, with a warning, for similarities or negative values), INTERVAL adds an intercept, ORDINAL keeps only their order (UNTIE lets tied values part) and SPLINE fits a smooth rising curve of the given degree with INKNOT inner knots at quantiles, plus an intercept. CONDITION=MATRIX fits one transformation per source, UNCONDITIONAL one for all. MODEL=WEIGHTED gives every source its own weight per dimension, GENERALIZED its own linear map of the common space and REDUCED(r) a map of rank r; one source always uses IDENTITY. The start is SIMPLEX, TORGERSON (classical scaling) or RANDOM(n): n random starts from the session's generator, the best one kept. DIMENSIONS(min,max) solves from max down to min, each solution starting from the main axes of the one before. Iterations stop after MAXITER, below MINSTRESS or when the stress improves by no more than DIFFSTRESS.

Tables: Case Processing Summary; Stress and Fit Measures (normalized raw stress, Stress-I, Stress-II, S-Stress, dispersion accounted for and Tucker's coefficient; without STRESS among PRINT keywords only the first and the last two); Final Coordinates (COMMON), Dimension Weights or Space Weights (WEIGHTS), Individual Space Coordinates, Distances, Transformed Proximities, Iteration History, Decomposition of Normalized Raw Stress, the proximities read (INPUT) and the Random Starts. The identity model's common space is turned to principal axes; the weighted one has sum of squares n per dimension, the generalized one Z'Z = nI; every dimension is signed so that the first object is positive. Charts: Common Space (a scatterplot matrix from three dimensions), Scree Plot of stress by dimensionality (STRESS), Dimension Weights, Individual Space, Transformation Plot and Residuals Plot. OUTFILE writes DIM\_1, DIM\_2, ... for the common space or the weights and OBJ\_1, OBJ\_2, ... for the distances and the transformed proximities, the sources one below the other. Not supported yet: RESTRICTIONS, a start from a file, and the VARIABLES and CORRELATIONS keywords of PRINT and PLOT.

## Example

```text
PROXSCAL brand1 TO brand6 /TRANSFORMATION=ORDINAL /CRITERIA=DIMENSIONS(1,3) /PRINT=COMMON STRESS HISTORY /PLOT=COMMON STRESS.
```

`statscraft help PROXSCAL` prints the same text in the terminal. [All commands](README.md)
