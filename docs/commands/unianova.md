<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# UNIANOVA

General linear model of one dependent variable: factorial ANOVA, ANCOVA and regression.

## Syntax

```text
UNIANOVA dependent [BY factors] [WITH covariates]
   [/WSFACTOR=name levels [POLYNOMIAL | contrast] ...] [/WSDESIGN=effects] (GLM)
   [/METHOD=SSTYPE(1 | 2 | 3 | 4)] [/INTERCEPT=INCLUDE | EXCLUDE] [/MISSING=EXCLUDE | INCLUDE]
   [/CRITERIA=[EPS(1E-8)] [ALPHA(0.05)]] [/REGWGT=var]
   [/PRINT=[DESCRIPTIVE] [HOMOGENEITY] [PARAMETER] [ETASQ] [OPOWER] [LOF]]
   [/EMMEANS=TABLES(OVERALL | factor[*factor...]) [WITH(covariate=value | MEAN ...)]
             [COMPARE[(factor)] [ADJ(LSD | BONFERRONI | SIDAK)]]] ...
   [/POSTHOC=factor ... (tests)] ... [/CONTRAST(factor)=DEVIATION[(ref)] | SIMPLE[(ref)] |
   DIFFERENCE | HELMERT | REPEATED | POLYNOMIAL[(metric)]] ...
   [/LMATRIX=['label'] effect coefficients ... [; ...] | ALL coefficients] ... [/KMATRIX=values]
   [/SAVE=tempvar[(name)] ...] [/PLOT=...] [/DESIGN=effects].
```

effects: INTERCEPT, a factor or covariate, a\*b or a BY b, a(b) or a WITHIN b, a(b(c)), x\*x, x\*a.

tempvar: PRED WPRED RESID WRESID DRESID ZRESID SRESID SEPRED COOK LEVER. tests: as in ONEWAY /POSTHOC (LSD BONFERRONI SIDAK TUKEY SCHEFFE GH T2 SNK BTUKEY DUNCAN GT2 GABRIEL FREGW QREGW T3 C DUNNETT DUNNETTL DUNNETTR). Without DESIGN the model is the intercept, the covariates and the full factorial of the factors. A case missing on any variable is left out; the weight is rounded to a whole number. The design matrix has one column per effect level (only combinations that occur); sweeping it gives a generalized inverse, and a column that adds nothing (EPS) is redundant, its parameter 0. Tables: Between-Subjects Factors, Descriptive Statistics, Levene's Test of Equality of Error Variances (absolute residuals by cell; based on the mean, the median, the median with adjusted df and the trimmed mean for a full factorial without covariates), Tests of Between-Subjects Effects (Type I sequential, II, III or IV sums of squares, with partial eta squared, noncentrality and observed power), Parameter Estimates, Lack of Fit Tests, Contrast Results and Test Results for each CONTRAST and LMATRIX, Estimated Marginal Means (cells weighted equally, covariates at their means or the WITH values) with Pairwise Comparisons and Univariate Tests, and the post hoc tables of ONEWAY on the observed means with the model's error. T2, T3, GH and C need a model with one factor. SAVE adds PRE\_1, RES\_1 and so on, or the names given.

GLM with WSFACTOR: the dependents are the levels of the within-subjects factors (the first factor's levels change slowest); Multivariate Tests, Mauchly's Test of Sphericity, Tests of Within-Subjects Effects (sphericity assumed, Greenhouse-Geisser, Huynh-Feldt, lower bound), Tests of Within-Subjects Contrasts and Tests of Between-Subjects Effects on the average.

PLOT is accepted with a warning that charts are not drawn yet. Not supported yet: GLM with several dependents without WSFACTOR (the multivariate model), RANDOM, TEST, ROBUST, OUTFILE, MMATRIX, MEASURE, POSTHOC VS, CONTRAST SPECIAL, PRINT GEF, TEST(LMATRIX), MBP, BP, F and WHITE; with WSFACTOR also EMMEANS (left out with a warning), POSTHOC, CONTRAST, LMATRIX, SAVE and PRINT PARAMETER, HOMOGENEITY and LOF.

## Example

```text
UNIANOVA score BY method school WITH pretest /PRINT=DESCRIPTIVE ETASQ PARAMETER /EMMEANS=TABLES(method) COMPARE ADJ(BONFERRONI) /DESIGN=pretest method school method*school.
```

`statscraft help UNIANOVA` prints the same text in the terminal. [All commands](README.md)
