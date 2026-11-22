<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# PREFSCAL

Multidimensional unfolding of preferences or proximities between row and column objects, with penalized stress.

## Syntax

```text
PREFSCAL [VARIABLES=]varlist
   [/INPUT=[ROWS(n | rowvar)] [SOURCES(n | sourcevar)]] [/PROXIMITIES=DISSIMILARITIES | SIMILARITIES]
   [/WEIGHTS=varlist] [/INITIAL=CLASSICAL[(TRIANGLE | SPEARMAN)] | CORRESPONDENCE | ROSSCLIFF
             | CENTROIDS[(n)] | RANDOM[(n)] | ('file' | dataset) [varlist]]
   [/CONDITION=ROW | MATRIX | UNCONDITIONAL]
   [/TRANSFORMATION=NONE[(INTERCEPT)] | LINEAR[(INTERCEPT)] | ORDINAL[(KEEPTIES | UNTIE)]
                    | SPLINE[([INTERCEPT] [ORDER=2] [INKNOT=1])]]
   [/MODEL=IDENTITY | WEIGHTED | GENERALIZED]
   [/RESTRICTIONS=[ROW(NONE | COORDINATES ('file' | dataset) [varlist])]
                  [COLUMN(NONE | COORDINATES ('file' | dataset) [varlist])]]
   [/PENALTY=[LAMBDA(0.5)] [OMEGA(1)]]
   [/CRITERIA=[DIMENSIONS(2[,max])] [MAXITER(5000)] [DIFFSTRESS(.000001)] [MINSTRESS(.0001)]]
   [/PRINT=[NONE] [INPUT] [MULTIPLE] [INITIAL] [HISTORY] [MEASURES] [DECOMPOSITION] [COMMON]
           [DISTANCES] [WEIGHTS] [INDIVIDUAL] [TRANSFORMATION]]
   [/PLOT=[NONE] [MULTIPLE] [INITIAL] [STRESS] [COMMON] [WEIGHTS] [INDIVIDUAL[(values)...]]
          [TRANSFORMATIONS[(values)...]] [SHEPARD[(values)...]] [FIT[(values)...]] [RESIDUALS[(values)...]]]
   [/OPTIONS=[MARKERS(var)] [COLORS(var)]]
   [/OUTFILE=[COMMON('file' | dataset)] [WEIGHTS(...)] [DISTANCES(...)] [TRANSFORMATIONS(...)]].
```

Each listed variable is a column object and each case a row object of a source. INPUT says how the cases form sources: ROWS(n) cases per source or SOURCES(n) sources (the cases divide evenly; both numbers take the first rows times sources cases); a row variable numbers the rows (a smaller value than the one before starts the next source), a source variable numbers the sources, which hold consecutive cases. Without INPUT there is one source. A missing proximity or a WEIGHTS value of 0 leaves the cell out; the weight variable of the data weights whole rows. SPLIT FILE is not used. Similarities are mirrored within each transformation list (largest and smallest swap).

The loss is penalized stress: the mean over transformation lists of the normalized stress, raised to LAMBDA, times the mean over lists of 1 + OMEGA v(p)^2 / v(t)^2, where v is the coefficient of variation and p and t are the proximities and transformed proximities of the list. A small LAMBDA makes the penalty stronger, OMEGA 0 turns it off. CONDITION picks the lists: every row of every source, every source, or all proximities. Each iteration moves the configuration by one majorization step and the transformation by one projected gradient step, so the penalized stress never rises. NONE keeps the proximities (INTERCEPT adds a constant), LINEAR fits a slope (INTERCEPT also a constant), ORDINAL keeps the order (UNTIE lets tied values part) and SPLINE fits a rising spline of the given degree with INKNOT inner knots; the transformed proximities are never negative. MODEL=WEIGHTED gives each source its own weight per dimension and GENERALIZED its own linear map; one source uses IDENTITY. Starts: CLASSICAL scales the full matrix after filling the row-row and column-column distances with the middle of their triangle bounds (SPEARMAN: root mean squared differences of the ranks within rows), ROSSCLIFF takes the singular vectors of the double centered squared proximities, CORRESPONDENCE the scores of a correspondence analysis of the mirrored proximities, CENTROIDS(n) the principal components of the rows with every column at the centroid of the rows that rank it among their first n, RANDOM(n) n random starts from the session's generator, the best kept, and a file or dataset gives the coordinates of the rows, then the columns, one case per object (no missing values); its variables are the ones named, else the first MAXDIM numeric ones. RESTRICTIONS fixes the coordinates of the rows or the columns to those of such a file, one case per object of the set; a missing value leaves that coordinate free. Fixing a whole set is external unfolding. The space is then not centered, scaled or rotated, so the fixed coordinates come out as given, and restrictions go with MODEL=IDENTITY only. DIMENSIONS solves from max down to min. Iterations stop after MAXITER, at MINSTRESS, or when the penalized stress falls by no more than DIFFSTRESS times its value.

Tables: Case Processing Summary; Measures (penalized stress with its stress and penalty parts; with MEASURES also normalized stress, Kruskal's Stress-I and II, Young's S-Stress-I and II, dispersion and variance accounted for, recovered preference orders, the mean Spearman rho and Kendall tau-b of the rows and the coefficients of variation); Final Common Space (rows, then columns); Dimension Weights or Space Weights; and on request the proximities, Random Starts, Initial Common Space, Iteration History, Decomposition of Normalized Stress, Individual Space Coordinates, Distances and Transformed Proximities. Charts: the joint plot of the common space (OPTIONS groups the rows by the values of a variable), Dimension Weights, Individual Space, Scree Plot, Initial Common Space, Random Starts, and Transformation, Shepard, Fit and Residuals plots; the values pick rows (CONDITION=ROW) or sources by their number or identifier, ALL for all. OUTFILE writes DIM\_1, DIM\_2, ... for the common space or the weights and ROW\_1, ..., COL\_1, ... for the distances and transformed proximities, the sources one below the other.

SMOOTH is read but runs as ORDINAL with a warning: the smoothing is not applied.

## Example

```text
PREFSCAL brand1 TO brand6 /INPUT=ROWS(person) /TRANSFORMATION=ORDINAL(UNTIE) /PRINT=MEASURES COMMON HISTORY /PLOT=COMMON TRANSFORMATIONS(1 TO 4).
```

`statscraft help PREFSCAL` prints the same text in the terminal. [All commands](README.md)
