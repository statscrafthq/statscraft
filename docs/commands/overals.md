<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# OVERALS

Nonlinear canonical correlation analysis: what two or more sets of variables, each quantified at its own level, have in common.

## Syntax

```text
OVERALS [/VARIABLES=] varlist(max) [varlist(max) ...]
   /ANALYSIS=varlist [(ORDI | SNOM | MNOM | NUME)] [varlist [(...)] ...]
   /SETS=n(count, count, ...)
   [/NOBSERVATIONS=n] [/DIMENSION=2 | n] [/INITIAL=NUMERICAL | RANDOM]
   [/MAXITER=100 | n] [/CONVERGENCE=.00001 | value]
   [/PRINT=[FREQ] [QUANT] [CENTROID] [HISTORY] [WEIGHTS] [OBJECT] [FIT] [DEFAULT] [NONE]]
   [/PLOT=[NDIM(first | ALL, last | MAX)] [OBJECT[(varlist)][(len)]] [QUANT[(varlist)][(len)]]
         [LOADINGS[(len)]] [TRANS[(varlist)]] [CENTROID[(varlist)][(len)]] [DEFAULT[(len)]] [NONE]]
   [/SAVE=[root][(n)]] [/MATRIX=OUT(* | 'file' | dataset)]
```

Variables are read as for HOMALS and quantified at the level ANALYSIS gives (ORDI by default): MNOM a free point per category and dimension; SNOM, ORDI and NUME one value per category (free, in the order of the codes, or in equal steps) times a weight per dimension. SETS splits the ANALYSIS variables, in order, into at least two sets. A case missing on any variable of a set leaves that set out; a case in no set takes no part; each set needs three cases. The object scores and quantifications make the sum of each set's quantified variables as close as possible to the object scores, summed over the sets; scores are centered and uncorrelated with sum of squares K times n in the metric of the number of sets each case is in. WEIGHT cases count as replications. NUMERICAL (the default) first fits every single variable as numerical and then continues at the stated levels; RANDOM starts from random scores of the session's generator. The fit stops after MAXITER iterations or when it gains less than CONVERGENCE, and is turned to its principal axes; in each dimension the case with the largest absolute score is positive.

DIMENSION is lowered with a warning to what the sets allow. One variable per set gives the solution of HOMALS (all MNOM) or PRINCALS (all single); two sets of numerical variables give eigenvalues (1 + r)/2 for the canonical correlations r.

Tables: Variables by Set and Summary of Analysis (loss of each set per dimension over n, the mean, and the eigenvalue 1 minus the mean; the fit is their sum) always; FREQ the frequencies with the cases missing from the set; HISTORY the fit per iteration; WEIGHTS the weights and component loadings of the single variables; FIT multiple fit, single fit (squared weights) and single loss; CENTROID per variable the quantification, projected centroids and centroids; QUANT the quantification, single and multiple coordinates; OBJECT the object scores. Without PRINT, or with DEFAULT: FREQ CENTROID WEIGHTS; NONE leaves the summary. Charts (OBJECT and LOADINGS by default): object scores, labelled by each named variable; loadings; TRANS the quantification against the category (one chart per dimension for MNOM); QUANT the category coordinates; CENTROID the centroids. Label lengths and NDIM as for HOMALS. SAVE adds the object scores rootD\_S (OVE by default). MATRIX=OUT writes ROWTYPE\_ (QUANT, SCOOR\_, MCOOR\_, CENTRO\_, PCENTRO\_, WEIGHT\_, LOADING\_), LEVEL, VARNAME\_, VARTYPE\_, SET\_ and DIM1 to DIMn; values that do not apply are 0. SPLIT FILE does not apply.

## Example

```text
OVERALS VARIABLES=habit1 habit2(4) score1 score2(10) /ANALYSIS=habit1 habit2(SNOM) score1 score2(NUME) /SETS=2(2,2) /PRINT=FIT QUANT /PLOT=OBJECT CENTROID(habit1).
```

`statscraft help OVERALS` prints the same text in the terminal. [All commands](README.md)
