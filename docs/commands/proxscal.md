<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# PROXSCAL

Multidimensional scaling of one or more proximity matrices by majorization, with optimal transformations.

## Syntax

```text
PROXSCAL varlist | /MATRIX=IN('file' | dataset | *)
   [/TABLE=rowvar BY colvar [BY sourcevar] | sourcevar] [/SHAPE=LOWER | UPPER | BOTH]
   [/INITIAL=SIMPLEX | TORGERSON | RANDOM[(n)] | ('file' | dataset) [varlist]] [/WEIGHTS=varlist] [/CONDITION=MATRIX | UNCONDITIONAL]
   [/TRANSFORMATION=RATIO | INTERVAL | ORDINAL[(KEEPTIES | UNTIE)] | SPLINE [DEGREE=2] [INKNOT=1]]
   [/PROXIMITIES=DISSIMILARITIES | SIMILARITIES] [/MODEL=IDENTITY | WEIGHTED | GENERALIZED | REDUCED[(2)]]
   [/RESTRICTIONS=COORDINATES('file' | dataset) [ALL | varlist]
      | VARIABLES('file' | dataset) [ALL | FIRST(n) | varlist] [(kind ...)]]
   [/ACCELERATION=NONE] [/CRITERIA=[DIMENSIONS(2[,max])] [MAXITER(100)] [DIFFSTRESS(.0001)] [MINSTRESS(.0001)]]
   [/PRINT=[NONE] [INPUT] [RANDOM] [HISTORY] [STRESS] [DECOMPOSITION] [COMMON] [DISTANCES] [WEIGHTS]
           [INDIVIDUAL] [TRANSFORMATIONS] [VARIABLES] [CORRELATIONS]]
   [/PLOT=[NONE] [STRESS] [COMMON] [WEIGHTS] [INDIVIDUAL[(sources | ALL)]] [TRANSFORMATIONS[(sources | ALL)]]
          [RESIDUALS[(sources | ALL)]] [VARIABLES[(variables | ALL)]] [CORRELATIONS]]
   [/OUTFILE=[COMMON('file' | dataset)] [WEIGHTS(...)] [DISTANCES(...)] [TRANSFORMATIONS(...)] [VARIABLES(...)]].
```

The command places objects in a space so that their distances fit the transformed proximities in least squares (normalized raw stress). The proximities are dissimilarities unless PROXIMITIES=SIMILARITIES. Without TABLE each case is a row of a square matrix and each variable a column; more cases than variables hold several matrices one after the other, each a source. TABLE row BY column \[BY source\] reads one proximity per case, with each variable a source or the source in a third variable; TABLE=source names the matrix of each case. MATRIX=IN reads a matrix file from PROXIMITIES or CLUSTER as one source.

SHAPE=LOWER (the default) or UPPER picks a triangle; BOTH takes the weighted mean of the two. The diagonal is not used. WEIGHTS gives each proximity variable a weight variable; a missing proximity or weight counts as weight 0.

TRANSFORMATION=RATIO (the default) keeps the proximities as they are; INTERVAL is used instead, with a warning, for similarities or negative values. INTERVAL adds an intercept, ORDINAL keeps only the order (UNTIE lets ties part), and SPLINE fits a smooth rising curve of DEGREE (1 to 3) with INKNOT inner knots at quantiles, plus an intercept. CONDITION=MATRIX (the default) fits one transformation per source, UNCONDITIONAL one for all.

MODEL=IDENTITY (the default) uses one common space. WEIGHTED gives each source its own weight per dimension, GENERALIZED its own linear map of the common space and REDUCED(r) a map of rank r; one source always uses IDENTITY. INITIAL is SIMPLEX (the default), TORGERSON (classical scaling), RANDOM(n) (n random starts from the session's generator, the best kept), or a file or dataset with one case per object holding the start coordinates (the variables named, or its first numeric ones, one per dimension, none missing). DIMENSIONS(min,max) solves from max down to min, each solution starting from the one before. Iterations stop after MAXITER, below MINSTRESS or when stress gains no more than DIFFSTRESS.

PRINT shows COMMON, WEIGHTS and STRESS by default (and VARIABLES and CORRELATIONS with RESTRICTIONS=VARIABLES). Tables: Case Processing Summary; Stress and Fit Measures (normalized raw stress, Stress-I, Stress-II, S-Stress, dispersion accounted for and Tucker's coefficient; without STRESS only the first and the last two); Final Coordinates (COMMON); Dimension or Space Weights (WEIGHTS); Individual Space Coordinates; Distances; Transformed Proximities; Iteration History; Decomposition of Normalized Raw Stress; the proximities read (INPUT); and the Random Starts. The identity common space is turned to principal axes, and every dimension is signed so that the first object is positive.

PLOT shows COMMON and WEIGHTS by default: Common Space (a scatterplot matrix from three dimensions), Scree Plot of stress by dimensionality (STRESS), Dimension Weights, Individual Space, Transformation Plot and Residuals Plot. OUTFILE writes DIM\_1, DIM\_2 and so on for the common space or the weights, and OBJ\_1, OBJ\_2 and so on for the distances and transformed proximities, the sources one below the other.

RESTRICTIONS=COORDINATES fixes coordinates of the common space to those of a file with one case per object (the variables named, or its first numeric ones; smaller solutions use the first columns). A missing value leaves that coordinate free. The common space is not turned, so the fixed coordinates come out exactly as given, and transformed proximities and distances are shown at the scale of the file.

RESTRICTIONS=VARIABLES makes the common space a weighted sum of independent variables from a file with one case per object: the variables named, the first n, or ALL. Each variable has a kind in parentheses, one per variable or one for all: INTERVAL (the default), NOMINAL, ORDINAL with KEEPTIES or UNTIE, or SPLINE with DEGREE and INKNOT. A missing value takes the fitted value. With fewer variables than dimensions the rest are free. PRINT VARIABLES gives the transformed variables and their regression weights, CORRELATIONS their correlations with the dimensions, and OUTFILE VARIABLES writes them as VAR\_1, VAR\_2 and so on. PLOT CORRELATIONS (also on by default) draws each variable at its correlations with the dimensions, and PLOT VARIABLES a Transformation Plot of each variable named. Both restrictions need the identity model.

## Example

```text
PROXSCAL brand1 TO brand6 /TRANSFORMATION=ORDINAL /CRITERIA=DIMENSIONS(1,3) /PRINT=COMMON STRESS HISTORY /PLOT=COMMON STRESS.
```

`statscraft help PROXSCAL` prints the same text in the terminal. [All commands](README.md)
