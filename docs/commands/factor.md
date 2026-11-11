<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# FACTOR

Factor analysis: principal components and six factoring methods, rotations, factor scores.

## Syntax

```text
FACTOR VARIABLES=varlist | MATRIX=IN(COR=* | 'file' | COV=* | 'file')
       [/MISSING=[LISTWISE | PAIRWISE | MEANSUB] [INCLUDE]] [/SELECT=var(value)]
       [/ANALYSIS=varlist] [/METHOD=CORRELATION]
       [/PRINT=[DEFAULT] [INITIAL] [EXTRACTION] [ROTATION] [UNIVARIATE] [CORRELATION] [COVARIANCE]
               [SIG] [DET] [INV] [REPR] [AIC] [KMO] [FSCORE] [ALL]]
       [/FORMAT=[SORT] [BLANK(n)] [DEFAULT]] [/DIAGONAL=values | DEFAULT] [/PLOT=...]
       [/CRITERIA=[FACTORS(n)] [MINEIGEN(1)] [ITERATE(25)] [ECONVERGE(.001)] [RCONVERGE(.0001)]
                  [KAISER | NOKAISER] [DELTA(0)] [KAPPA(4)] [DEFAULT]]
       [/EXTRACTION=PC | PAF | ALPHA | IMAGE | ULS | GLS | ML]
       [/ROTATION=VARIMAX | EQUAMAX | QUARTIMAX | OBLIMIN[(delta)] | PROMAX[(kappa)] | NOROTATE]
       [/SAVE=[REG | BART | AR] [(n | ALL [rootname])]].
```

The analysis is of the correlation matrix of the ANALYSIS variables (default all of VARIABLES).

LISTWISE (default) uses the cases valid on every variable of VARIABLES; PAIRWISE each pair's own cases; MEANSUB puts the mean in place of a missing value. SELECT computes the matrix from the cases with that value, while scores are saved for all cases. CRITERIA applies to the EXTRACTION and ROTATION that follow it. The number of factors is FACTORS(n), else the count of eigenvalues above MINEIGEN. Without EXTRACTION the extraction is PC with VARIMAX; with EXTRACTION but no ROTATION the factors are not rotated. A later ANALYSIS cancels the PRINT, DIAGONAL, EXTRACTION, ROTATION and SAVE before it, with a warning. Tables: Descriptive Statistics (UNIVARIATE), Correlation Matrix (CORRELATION, SIG, DET), Inverse of Correlation Matrix (INV), Covariance Matrix, KMO and Bartlett's Test (KMO), Anti-image Matrices (AIC), Communalities, Total Variance Explained, Component or Factor Matrix, Goodness-of-fit Test (ML, GLS), Reproduced Correlations (REPR), the rotated matrix with its transformation matrix, or Pattern, Structure and factor correlation matrices for OBLIMIN and PROMAX, Score Coefficient and Score Covariance Matrix (FSCORE). SORT orders the variables by the factor of their largest loading; BLANK(n) leaves out loadings below n in absolute value. SAVE adds scores by regression, Bartlett or Anderson-Rubin, named FAC1\_1, FAC2\_1 and so on, or rootname1, rootname2. MATRIX=IN reads a matrix file (ROWTYPE\_ N, MEAN, STDDEV, CORR or COV, VARNAME\_) from the active dataset or a file. Weighted when a weight variable is set. PLOT is accepted with a warning that charts are not drawn yet. Not supported yet: METHOD=COVARIANCE, MATRIX=OUT, MATRIX=IN(FAC=...).

## Example

```text
FACTOR VARIABLES=q1 TO q10 /PRINT=INITIAL EXTRACTION ROTATION KMO /FORMAT=SORT BLANK(.3) /CRITERIA=MINEIGEN(1) /EXTRACTION=PC /ROTATION=VARIMAX /SAVE=REG(ALL).
```

`statscraft help FACTOR` prints the same text in the terminal. [All commands](README.md)
