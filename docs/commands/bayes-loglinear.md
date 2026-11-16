<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# BAYES LOGLINEAR

Bayesian test of independence in a two-way table, with simulated interaction terms.

## Syntax

```text
BAYES LOGLINEAR /TABLE rowvar BY colvar
   [/INFERENCE ANALYSIS=POSTERIOR | BAYESFACTOR | BOTH]
   [/ESTBF [MODEL=MULTINOMIAL | POISSON | NONPARAMETRIC] [FIX_MARGIN=TOTAL | ROW | COLUMN]
           [PRIOR_TYPE=CONJUGATE | MIXTURE_DIRICHLET | INTRINSIC] [SHAPEPARAMS=values]
           [SCALEPARAM=DATA | b] [LAMBDAS=values]]
   [/PRINT [SUPPRESS_TABLE=FALSE | TRUE] [CELLS=OBSERVED ROW COLUMN EXPECTED TOTAL ALL]
           [STATISTICS=CHISQ LRT]]
   [/CRITERIA [CILEVEL=95] [SEED=RANDOM | n] [TOL=0.000001] [MAXITER=2000]
              [POSTSAMPLES=10000] [ORDER=ASCENDING | DESCENDING]].
```

The table counts cases valid on both variables with rounded case weights; only categories that occur count, and ORDER sorts both of them. Every Bayes factor is BF01, for independence. CONJUGATE puts gamma priors with the shapes of SHAPEPARAMS (1 for every cell; one value, or one row of values per table row with ';' between rows) on the cells; FIX\_MARGIN says whether the grand total, the row totals or the column totals are fixed by the design. POISSON fixes nothing and uses the gamma scale SCALEPARAM (DATA: rows times columns times the smallest shape over N). MIXTURE\_DIRICHLET mixes symmetric Dirichlet priors over a log-Cauchy weight, by quadrature with TOL and MAXITER. INTRINSIC needs a 2 x 2 table (others fall back to the mixture with a warning). NONPARAMETRIC needs FIX\_MARGIN=ROW with two rows or COLUMN with two columns; LAMBDAS holds one Dirichlet parameter, or one per free category.

The posterior part draws POSTSAMPLES cell probability sets from SEED and reports each log odds ratio against the last row and column, with a simultaneous credible region for all of them. PRINT shows the crosstab (observed counts by default) and, for CHISQ, Pearson's chi-square, the continuity correction of a 2 x 2 table and Fisher's test when an expected count is below 5; LRT adds the likelihood ratio. Keywords with an underscore may be written without it or as two words.

## Example

```text
BAYES LOGLINEAR /TABLE ferry_route BY ticket_class
  /INFERENCE ANALYSIS=BOTH /ESTBF MODEL=MULTINOMIAL FIX_MARGIN=ROW PRIOR_TYPE=CONJUGATE
  /PRINT CELLS=OBSERVED ROW STATISTICS=CHISQ LRT /CRITERIA SEED=4417.
```

`statscraft help BAYES LOGLINEAR` prints the same text in the terminal. [All commands](README.md)
