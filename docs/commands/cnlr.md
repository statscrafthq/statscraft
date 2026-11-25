<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# CNLR

Constrained nonlinear regression: least squares or a loss function of your own, under bounds, linear constraints and constrained functions.

## Syntax

```text
CNLR dependent
   [/FILE=file] [/OUTFILE=file] [/PRED=variable]
   [/SAVE=[PRED] [RESID[(variable)]] [DERIVATIVES] [LOSS]]
   [/BOUNDS=bound; bound; ...] [/LOSS=variable] [/BOOTSTRAP[(samples)]]
   [/CRITERIA=[ITER(n)] [MINORITERATION(n)] [CRSHTOL(0.01)] [STEPLIMIT(2)] [FTOLERANCE(n)]
   [LFTOLERANCE(n)] [NFTOLERANCE(n)] [LSTOLERANCE(0.9)] [OPTOLERANCE(n)] [FPRECISION(n)]
   [ISTEP(1E20)] [CKDER(0.5)]]
```

Fits the model of MODEL PROGRAM (and DERIVATIVES) like NLR, but by a method that allows constraints. FILE, OUTFILE, PRED, SAVE and the check of the derivatives work as in NLR; SAVE LOSS also saves the loss variable. A bound is two or three linear forms (numbers, parameters and number \* parameter, joined by + and -) with =, EQ, \<=, LE, \>= or GE between them; two relations point the same way, as in 2 \<= b \<= 5. A bound on one parameter limits it, a bound on several is a linear constraint; both hold at every trial value of the parameters, which start at the nearest point that meets them. A bound that names the variable of a CONSTRAINED FUNCTIONS program is nonlinear: the variable stands alone against a number, and the bound is met at the solution. Without LOSS the sum of weighted squared residuals is minimized and the tables of NLR follow (their standard errors take no account of the bounds). LOSS names a variable of the model program that holds the loss of a case; the weighted sum of it is minimized, derivatives in the DERIVATIVES program are then those of the loss, and only the iteration history with the final estimates is shown. BOOTSTRAP re-estimates the model on samples drawn from the cases with replacement (10 p (p + 1) / 2 of them unless a number is given, p the parameters) and shows the bootstrap standard errors, the 95% trimmed range and the correlations of the estimates; OUTFILE then gets the case of the estimates followed by one case per sample, NCASES counting the distinct cases drawn. ITER limits the iterations in all, MINORITERATION those of one inner minimization; CRSHTOL moves a starting value that lies within that relative distance of a bound onto it; STEPLIMIT limits the length of a step to that multiple of the length of the parameters (at least 1); FTOLERANCE, LFTOLERANCE and NFTOLERANCE say how far a constraint may be missed (all, linear and nonlinear ones); LSTOLERANCE is the accuracy of the line search, a smaller value being more exact; OPTOLERANCE and FPRECISION end the iterations when the gradient on the free parameters is at most the square root of the larger of the two times one plus the objective; a step longer than ISTEP stops the run as unbounded.

## Example

```text
MODEL PROGRAM b0=2 b1=0.3.
COMPUTE pred = b0 * EXP(b1 * dose).
CNLR response /BOUNDS 0 <= b1 <= 1; b0 + b1 <= 5.
```

`statscraft help CNLR` prints the same text in the terminal. [All commands](README.md)
