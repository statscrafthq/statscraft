<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# GLM

General linear model: one dependent variable as in UNIANOVA, or repeated measures with WSFACTOR.

## Syntax

```text
GLM dependent [BY factors] [WITH covariates]
   [/WSFACTOR=name levels [POLYNOMIAL | contrast] ...] [/WSDESIGN=effects] (GLM)
   [/METHOD=SSTYPE(1 | 2 | 3 | 4)] [/INTERCEPT=INCLUDE | EXCLUDE] [/MISSING=EXCLUDE | INCLUDE]
   [/CRITERIA=[EPS(1E-8)] [ALPHA(0.05)]] [/REGWGT=var]
   [/PRINT=[DESCRIPTIVE] [HOMOGENEITY] [PARAMETER] [ETASQ] [OPOWER] [LOF]]
   [/EMMEANS=TABLES(OVERALL | factor[*factor...]) [WITH(covariate=value | MEAN ...)]
             [COMPARE[(factor)] [ADJ(LSD | BONFERRONI | SIDAK)]]] ...
   [/POSTHOC=factor ... (tests)] ... [/CONTRAST(factor)=DEVIATION[(ref)] | SIMPLE[(ref)] |
   DIFFERENCE | HELMERT | REPEATED | POLYNOMIAL[(metric)]] ...
   [/LMATRIX=['label'] effect coefficients ... [; ...] | ALL coefficients] ... [/KMATRIX=values]
   [/SAVE=tempvar[(name)] ...] [/DESIGN=effects]
   [/PLOT=[SPREADLEVEL] [RESIDUALS] [PROFILE(factor[*factor[*factor]] ...) [WITH(covariate=value | MEAN ...)]
          [TYPE=LINE | BAR] [MEANREFERENCE=NO | YES] [ERRORBAR=NO | CI | SE[(2)]] [YAXIS=AUTO | 0]]] ...
```

effects: INTERCEPT, a factor or covariate, a\*b or a BY b, a(b) or a WITHIN b, a(b(c)), x\*x, x\*a.

tempvar: PRED WPRED RESID WRESID DRESID ZRESID SRESID SEPRED COOK LEVER. tests: as in ONEWAY /POSTHOC (LSD BONFERRONI SIDAK TUKEY SCHEFFE GH T2 SNK BTUKEY DUNCAN GT2 GABRIEL FREGW QREGW T3 C DUNNETT DUNNETTL DUNNETTR). Without DESIGN the model is the intercept, the covariates and the full factorial of the factors. A case missing on any variable is left out; the weight is rounded to a whole number. The design matrix has one column per effect level (only combinations that occur); sweeping it gives a generalized inverse, and a column that adds nothing (EPS) is redundant, its parameter 0. Tables: Between-Subjects Factors, Descriptive Statistics, Levene's Test of Equality of Error Variances (absolute residuals by cell; based on the mean, the median, the median with adjusted df and the trimmed mean for a full factorial without covariates), Tests of Between-Subjects Effects (Type I sequential, II, III or IV sums of squares, with partial eta squared, noncentrality and observed power), Parameter Estimates, Lack of Fit Tests, Contrast Results and Test Results for each CONTRAST and LMATRIX, Estimated Marginal Means (cells weighted equally, covariates at their means or the WITH values) with Pairwise Comparisons and Univariate Tests, and the post hoc tables of ONEWAY on the observed means with the model's error. T2, T3, GH and C need a model with one factor. SAVE adds PRE\_1, RES\_1 and so on, or the names given.

GLM with WSFACTOR: the dependents are the levels of the within-subjects factors (the first factor's levels change slowest); Multivariate Tests, Mauchly's Test of Sphericity, Tests of Within-Subjects Effects (sphericity assumed, Greenhouse-Geisser, Huynh-Feldt, lower bound), Tests of Within-Subjects Contrasts and Tests of Between-Subjects Effects on the average.

PLOT gives chart data. PROFILE draws the estimated marginal means as a line chart (bars with TYPE=BAR) over the levels of the first factor, one line per level of a second factor and one chart per level of a third; MEANREFERENCE adds the grand mean as a flat line. ERRORBAR=CI spans each mean's confidence interval at 1 minus CRITERIA ALPHA, ERRORBAR=SE(n) n standard errors on each side (2 by default); YAXIS is left to the drawing. SPREADLEVEL plots the mean of every cell with two cases or more against its standard deviation, then against its variance. RESIDUALS gives every case's predicted value, observed value and standardized residual. With WSFACTOR, PLOT is ignored with a warning. Not supported yet: GLM with several dependents without WSFACTOR (the multivariate model), RANDOM, TEST, ROBUST, OUTFILE, MMATRIX, MEASURE, POSTHOC VS, CONTRAST SPECIAL, PRINT GEF, TEST(LMATRIX), MBP, BP, F and WHITE; with WSFACTOR also EMMEANS (left out with a warning), POSTHOC, CONTRAST, LMATRIX, SAVE and PRINT PARAMETER, HOMOGENEITY and LOF.

## Example

```text
GLM t1 t2 t3 BY group /WSFACTOR=time 3 POLYNOMIAL /WSDESIGN=time /DESIGN=group.
```

`statscraft help GLM` prints the same text in the terminal. [All commands](README.md)
