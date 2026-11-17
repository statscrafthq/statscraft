<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# PRINCALS

Nonlinear principal components in the older syntax: CATPCA with variables read as categories 1 to a stated largest value.

## Syntax

```text
PRINCALS [/VARIABLES=] varlist(max) [varlist(max) ...]
   [/ANALYSIS=varlist [(ORDI | SNOM | MNOM | NUME)] [varlist [(...)] ...]]
   [/NOBSERVATIONS=n] [/DIMENSION=2 | n] [/MAXITER=100 | n] [/CONVERGENCE=.00001 | value]
   [/PRINT=[FREQ] [EIGEN] [LOADINGS] [QUANT] [HISTORY] [CORRELATION] [OBJECT] [DEFAULT] [ALL] [NONE]]
   [/PLOT=[NDIM(first | ALL, last | MAX)] [QUANT[(varlist)][(len)]] [OBJECT[(varlist)][(len)]]
         [LOADINGS[(len)]] [DEFAULT[(len)]] [ALL[(len)]] [NONE]]
   [/SAVE=[root][(n)]] [/MATRIX=OUT(* | 'file' | dataset)]
```

Variables are read as for HOMALS: whole values 1 to the stated largest are the categories, all else is missing. The level after a list on ANALYSIS covers the variables since the previous parenthesis: ORDI (the default) keeps the order of the categories, SNOM any values, NUME equal steps, MNOM a free point per category and dimension. The fit is that of CATPCA with these levels and variable principal normalization; messages name PRINCALS. Tables: Case Processing Summary, Iteration History and Model Summary always (the eigenvalue there is the sum over the variables; divided by their number it is the mean that older output showed); FREQ the frequencies; LOADINGS the component loadings; QUANT the quantifications with centroid and vector coordinates; CORRELATION the correlations of the quantified variables; OBJECT the object scores; HISTORY every iteration. Without PRINT: FREQ EIGEN LOADINGS; DEFAULT adds QUANT; ALL gives every one. Charts (QUANT and OBJECT by default, none for one dimension): QUANT the categories of each variable, OBJECT the objects labelled by each named variable, LOADINGS the loadings; label lengths and NDIM as for HOMALS. SAVE adds rootD\_S (PRI by default). MATRIX=OUT writes ROWTYPE\_ (QUANT, SCOOR\_ for single and MCOOR\_ for multiple coordinates, LOADING\_), LEVEL, VARNAME\_, VARTYPE\_ (MULTIPLE, SINGLE N, ORDINAL or NUMERICAL) and DIM1 to DIMn; the quantification of a single variable is in DIM1, the other columns 0. SPLIT FILE does not apply.

## Example

```text
PRINCALS VARIABLES=taste smell(5) price(4) /ANALYSIS=taste smell(SNOM) price(NUME) /PRINT=QUANT LOADINGS /PLOT=LOADINGS.
```

`statscraft help PRINCALS` prints the same text in the terminal. [All commands](README.md)
