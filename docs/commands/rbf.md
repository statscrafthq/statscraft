<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# RBF

Radial basis function network: Gaussian hidden units at two-step clusters and least squares output weights.

## Syntax

```text
RBF dependent [(MLEVEL=S | O | N)] [dependent ...] [BY factors] [WITH covariates]
   [/EXCEPT VARIABLES=varlist]
   [/RESCALE [COVARIATE=STANDARDIZED | NORMALIZED | ADJNORMALIZED | NONE]
             [DEPENDENT=STANDARDIZED | NORMALIZED | ADJNORMALIZED | NONE]]
   [/PARTITION TRAINING=70 TESTING=30 HOLDOUT=0 | VARIABLE=var]
   [/ARCHITECTURE [MINUNITS=AUTO | n MAXUNITS=AUTO | n | NUMUNITS=n] [HIDDENFUNCTION=NRBF | ORBF]]
   [/CRITERIA OVERLAP=AUTO | number] [/MISSING USERMISSING=EXCLUDE | INCLUDE]
   [/PRINT [CPS] [NETWORKINFO] [SUMMARY] [CLASSIFICATION] [SOLUTION] [IMPORTANCE] [NONE]]
   [/PLOT [NETWORK] [PREDICTED] [RESIDUAL] [ROC] [GAIN] [LIFT] [NONE]]
   [/SAVE [PREDVAL[(names)]] [PSEUDOPROB[(root[:25] ...)]]] [/OUTFILE MODEL='file' ...].
```

Variables, missing values, partitions, rescaling (STANDARDIZED by default for scale dependents too), tables, charts, SAVE (RBF\_PredictedValue, RBF\_PseudoProbability) and OUTFILE work as in MLP. The training cases are clustered by the two-step method (covariates as continuous, factors as categorical variables). Each hidden unit is a Gaussian at a cluster's means and category shares, its width the square root of OVERLAP (AUTO: 1 + 0.1 per input unit) times the root mean square of the cluster's standard deviations and of p(1 - p) for categories. NRBF (default) divides the units by their sum; ORBF keeps them and adds a bias. The output weights are least squares, with identity outputs and the sum of squares error for every dependent. Without NUMUNITS the number of units is searched between MINUNITS and MAXUNITS (AUTO: from the two-step cluster count K and the R output units, min(K, R), at least 2 for NRBF, to max(10, 2K, R)): the lowest error of the testing cases wins, or without them the lowest BIC of the training cases. The weight variable is not used.

## Example

```text
SET SEED=4417.
RBF spend BY plan WITH tenure visits /ARCHITECTURE HIDDENFUNCTION=ORBF
  /PRINT CPS NETWORKINFO SUMMARY SOLUTION /PLOT PREDICTED RESIDUAL /SAVE PREDVAL(spend_hat).
```

`statscraft help RBF` prints the same text in the terminal. [All commands](README.md)
