<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# UNIANOVA

General linear model of one dependent variable: factorial ANOVA, ANCOVA and regression.

## Syntax

```text
UNIANOVA dependent [BY factors] [WITH covariates]
   [/WSFACTOR=name levels [POLYNOMIAL | contrast] ...] [/WSDESIGN=effects] (GLM)
   [/METHOD=SSTYPE(1 | 2 | 3 | 4)] [/INTERCEPT=INCLUDE | EXCLUDE] [/MISSING=EXCLUDE | INCLUDE]
   [/CRITERIA=[EPS(1E-8)] [ALPHA(0.05)]] [/REGWGT=var]
   [/PRINT=[DESCRIPTIVE] [HOMOGENEITY] [PARAMETER] [ETASQ] [OPOWER] [LOF] [RSSCP]
          [TEST([SSCP] [MMATRIX | TRANSFORM])]]
   [/EMMEANS=TABLES(OVERALL | factor[*factor...]) [WITH(covariate=value | MEAN ...)]
             [COMPARE[(factor)] [ADJ(LSD | BONFERRONI | SIDAK)]]] ...
   [/POSTHOC=factor ... (tests)] ... [/CONTRAST(factor)=DEVIATION[(ref)] | SIMPLE[(ref)] |
   DIFFERENCE | HELMERT | REPEATED | POLYNOMIAL[(metric)]] ...
   [/LMATRIX=['label'] effect coefficients ... [; ...] | ALL coefficients] ...
   [/MMATRIX=['label'] dependent value ... | ALL values [; ...]] (GLM) [/KMATRIX=values [; ...]]
   [/SAVE=tempvar[(name)] ...] [/DESIGN=effects]
   [/PLOT=[SPREADLEVEL] [RESIDUALS] [PROFILE(factor[*factor[*factor]] ...) [WITH(covariate=value | MEAN ...)]
          [TYPE=LINE | BAR] [MEANREFERENCE=NO | YES] [ERRORBAR=NO | CI | SE[(2)]] [YAXIS=AUTO | 0]]] ...
```

effects: INTERCEPT, a factor or covariate, a\*b or a BY b, a(b) or a WITHIN b, a(b(c)), x\*x, x\*a.

tempvar: PRED WPRED RESID WRESID DRESID ZRESID SRESID SEPRED COOK LEVER. tests: as in ONEWAY /POSTHOC (LSD BONFERRONI SIDAK TUKEY SCHEFFE GH T2 SNK BTUKEY DUNCAN GT2 GABRIEL FREGW QREGW T3 C DUNNETT DUNNETTL DUNNETTR). Without DESIGN the model is the intercept, the covariates and the full factorial of the factors. A case missing on any variable is left out; the weight is rounded to a whole number. The design matrix has one column per effect level (only combinations that occur); sweeping it gives a generalized inverse, and a column that adds nothing (EPS) is redundant, its parameter 0. Tables: Between-Subjects Factors, Descriptive Statistics, Levene's Test of Equality of Error Variances (absolute residuals by cell; based on the mean, the median, the median with adjusted df and the trimmed mean for a full factorial without covariates), Tests of Between-Subjects Effects (Type I sequential, II, III or IV sums of squares, with partial eta squared, noncentrality and observed power), Parameter Estimates, Lack of Fit Tests, Contrast Results and Test Results for each CONTRAST and LMATRIX, Estimated Marginal Means (cells weighted equally, covariates at their means or the WITH values) with Pairwise Comparisons and Univariate Tests, and the post hoc tables of ONEWAY on the observed means with the model's error. T2, T3, GH and C need a model with one factor. SAVE adds PRE\_1, RES\_1 and so on, or the names given.

GLM with WSFACTOR: the dependents are the levels of the within-subjects factors (the first factor's levels change slowest); Multivariate Tests, Mauchly's Test of Sphericity, Tests of Within-Subjects Effects (sphericity assumed, Greenhouse-Geisser, Huynh-Feldt, lower bound), Tests of Within-Subjects Contrasts and Tests of Between-Subjects Effects on the average.

GLM with several dependents and no WSFACTOR is the multivariate model: Multivariate Tests of every effect (Pillai's trace, Wilks' lambda, Hotelling's trace, Roy's largest root, each with F, df and Sig., and with ETASQ and OPOWER partial eta squared, noncentrality and power), then the univariate tables with a row per dependent. HOMOGENEITY adds Box's M test of equal covariance matrices in the cells to Levene's test of each dependent; RSSCP gives the residual SSCP, covariance and correlation matrices and Bartlett's test of sphericity; TEST(SSCP) the SSCP of each effect and of the error. A custom test L B M = K takes L from LMATRIX (or the intercept's test when only MMATRIX or KMATRIX is given), M from MMATRIX (the identity by default; columns end with ';' and are named by their label or T1, T2, ...) and K from KMATRIX (rows end with ';', zero by default); CONTRAST always uses the identity. Each test gives Contrast Results (K Matrix), Multivariate Test Results and Univariate Test Results; TEST(MMATRIX) shows M. EMMEANS, POSTHOC, PLOT and SAVE work on each dependent in turn (SAVE without names).

PLOT gives chart data. PROFILE draws the estimated marginal means as a line chart (bars with TYPE=BAR) over the levels of the first factor, one line per level of a second factor and one chart per level of a third; MEANREFERENCE adds the grand mean as a flat line. ERRORBAR=CI spans each mean's confidence interval at 1 minus CRITERIA ALPHA, ERRORBAR=SE(n) n standard errors on each side (2 by default); YAXIS is left to the drawing. SPREADLEVEL plots the mean of every cell with two cases or more against its standard deviation, then against its variance. RESIDUALS gives every case's predicted value, observed value and standardized residual. With WSFACTOR, PLOT is ignored with a warning. Not supported yet: RANDOM, TEST, ROBUST, OUTFILE, MEASURE, POSTHOC VS, CONTRAST SPECIAL, PRINT GEF, TEST(LMATRIX), MBP, BP, F and WHITE; in the multivariate model the multivariate tests of EMMEANS COMPARE; with WSFACTOR also EMMEANS (left out with a warning), POSTHOC, CONTRAST, LMATRIX, MMATRIX, SAVE and PRINT PARAMETER, HOMOGENEITY, LOF, RSSCP and TEST(SSCP).

## Example

```text
UNIANOVA score BY method school WITH pretest /PRINT=DESCRIPTIVE ETASQ PARAMETER /EMMEANS=TABLES(method) COMPARE ADJ(BONFERRONI) /DESIGN=pretest method school method*school.
```

`statscraft help UNIANOVA` prints the same text in the terminal. [All commands](README.md)
