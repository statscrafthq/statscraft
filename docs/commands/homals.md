<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# HOMALS

Homogeneity analysis in the older syntax: MULTIPLE CORRESPONDENCE with variables read as categories 1 to a stated largest value.

## Syntax

```text
HOMALS [/VARIABLES=] varlist(max) [varlist(max) ...]
   [/ANALYSIS=varlist] [/NOBSERVATIONS=n] [/DIMENSION=2 | n]
   [/MAXITER=100 | n] [/CONVERGENCE=.00001 | value]
   [/PRINT=[FREQ] [EIGEN] [DISCRIM] [QUANT] [OBJECT] [HISTORY] [DEFAULT] [ALL] [NONE]]
   [/PLOT=[NDIM(first | ALL, last | MAX)] [QUANT[(varlist)][(len)]] [OBJECT[(varlist)][(len)]]
         [DISCRIM[(len)]] [DEFAULT[(len)]] [ALL[(len)]] [NONE]]
   [/SAVE=[root][(n)]] [/MATRIX=OUT(* | 'file' | dataset)]
```

The number after a list is the largest category of its variables: every whole value from 1 up to it is a category, a fraction is cut to its whole part, user-missing codes count as values, and anything else (system-missing, 0, negatives, above the largest) is missing and left out of the fit for that variable. String variables are refused. ANALYSIS picks the variables of the fit (all of VARIABLES by default; named twice it is an error); the others can still label object plots.

NOBSERVATIONS uses only the first n cases. Every other subcommand counts once, the last one.

The model, the fit and the numbers are those of MULTIPLE CORRESPONDENCE with variable principal normalization, run on these categories; messages name HOMALS. Tables: Case Processing Summary, Iteration History and Model Summary (eigenvalue and inertia per dimension; the inertia is the mean discrimination measure) always; FREQ the frequencies of each variable; DISCRIM the discrimination measures; QUANT the category quantifications; OBJECT the object scores; HISTORY every iteration. Without PRINT, or with DEFAULT: FREQ EIGEN DISCRIM QUANT; ALL adds OBJECT and HISTORY. Charts (QUANT and OBJECT by default, none for one dimension): QUANT all categories together, OBJECT the objects labelled by each named variable, DISCRIM the discrimination measures. The label length lies from 1 to 20 (12 by default). NDIM(a, b) draws dimension a against a+1 to b, MAX the last dimension, ALL every pair; without it dimension 1 against 2.

SAVE adds the object scores rootD\_S (HOM by default, the root cut to 6 characters, D the dimension, S the first free set number) for the first n dimensions (all by default). MATRIX=OUT writes ROWTYPE\_ (QUANT), LEVEL, VARNAME\_ and DIM1 to DIMn, one case per category; an asterisk replaces the active dataset. SPLIT FILE does not apply.

## Example

```text
HOMALS VARIABLES=brand(3) shop(4) payment(2) /PRINT=FREQ DISCRIM QUANT /PLOT=QUANT(5) /SAVE=hs(2).
```

`statscraft help HOMALS` prints the same text in the terminal. [All commands](README.md)
