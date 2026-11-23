<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# GLM

General linear model: one dependent variable as in UNIANOVA, several (multivariate analysis of variance and covariance), or repeated measures with WSFACTOR.

## Syntax

```text
GLM dependent [BY factors] [WITH covariates]
   [/WSFACTOR=name levels [POLYNOMIAL | contrast] ...] [/WSDESIGN=effects] [/MEASURE=names] (GLM)
   [/METHOD=SSTYPE(1 | 2 | 3 | 4)] [/INTERCEPT=INCLUDE | EXCLUDE] [/MISSING=EXCLUDE | INCLUDE]
   [/CRITERIA=[EPS(1E-8)] [ALPHA(0.05)]] [/REGWGT=var]
   [/PRINT=[DESCRIPTIVE] [HOMOGENEITY] [PARAMETER] [ETASQ] [OPOWER] [LOF] [RSSCP] [GEF] [MBP] [BP] [F] [WHITE]
          [TEST([SSCP] [LMATRIX | ESTIMABLE] [MMATRIX | TRANSFORM])]]
   [/EMMEANS=TABLES(OVERALL | factor[*factor...]) [WITH(covariate=value | MEAN ...)]
             [COMPARE[(factor)] [ADJ(LSD | BONFERRONI | SIDAK)]]] ...
   [/POSTHOC=factor ... (tests) [VS effect]] ... [/CONTRAST(factor)=DEVIATION[(ref)] | SIMPLE[(ref)] |
   DIFFERENCE | HELMERT | REPEATED | POLYNOMIAL[(metric)] | SPECIAL(matrix)] ...
   [/LMATRIX=['label'] effect coefficients ... [; ...] | ALL coefficients] ...
   [/TEST=effect VS coefficient*effect +/- ... [DF(df)] | value DF(df)] ... (one dependent)
   [/MMATRIX=['label'] dependent value ... | ALL values [; ...]] (GLM) [/KMATRIX=values [; ...]]
   [/SAVE=tempvar[(name)] ...] [/DESIGN=effects]
   [/ROBUST[=HC0 | HC1 | HC2 | HC3 | HC4]] [/RANDOM=factors]
   [/MBPDESIGN[=effects]] [/BPDESIGN[=effects]] [/FDESIGN[=effects]]
   [/PLOT=[SPREADLEVEL] [RESIDUALS] [PROFILE(factor[*factor[*factor]] ...) [WITH(covariate=value | MEAN ...)]
          [TYPE=LINE | BAR] [MEANREFERENCE=NO | YES] [ERRORBAR=NO | CI | SE[(2)]] [YAXIS=AUTO | 0]]] ...
```

effects: INTERCEPT, a factor or covariate, a\*b or a BY b, a(b) or a WITHIN b, a(b(c)), x\*x, x\*a.

tempvar: PRED WPRED RESID WRESID DRESID ZRESID SRESID SEPRED COOK LEVER. tests: as in ONEWAY /POSTHOC (LSD BONFERRONI SIDAK TUKEY SCHEFFE GH T2 SNK BTUKEY DUNCAN GT2 GABRIEL FREGW QREGW T3 C DUNNETT DUNNETTL DUNNETTR). VS names another effect of the design (not the intercept, and not a main effect named on the same POSTHOC) whose mean square and df are the error term of the pairwise and subset tests, which the tables then say; the tests of separate variances do not use an error term. Without DESIGN the model is the intercept, the covariates and the full factorial of the factors. A case missing on any variable is left out; the weight is rounded to a whole number. The design matrix has one column per effect level (only combinations that occur); sweeping it gives a generalized inverse, and a column that adds nothing (EPS) is redundant, its parameter 0. Tables: Between-Subjects Factors, Descriptive Statistics, Levene's Test of Equality of Error Variances (absolute residuals by cell; based on the mean, the median, the median with adjusted df and the trimmed mean for a full factorial without covariates), Tests of Between-Subjects Effects (Type I sequential, II, III or IV sums of squares, with partial eta squared, noncentrality and observed power), Parameter Estimates, Lack of Fit Tests, Contrast Results and Test Results for each CONTRAST and LMATRIX, Estimated Marginal Means (cells weighted equally, covariates at their means or the WITH values) with Pairwise Comparisons and Univariate Tests, and the post hoc tables of ONEWAY on the observed means with the model's error. T2, T3, GH and C need a model with one factor. SAVE adds PRE\_1, RES\_1 and so on, or the names given.

GLM with WSFACTOR: the dependents are the levels of the within-subjects factors (the first factor's levels change slowest); Multivariate Tests, Mauchly's Test of Sphericity, Tests of Within-Subjects Effects (sphericity assumed, Greenhouse-Geisser, Huynh-Feldt, lower bound), Tests of Within-Subjects Contrasts and Tests of Between-Subjects Effects on the average.

GLM with several dependents and no WSFACTOR is the multivariate model: Multivariate Tests of every effect (Pillai's trace, Wilks' lambda, Hotelling's trace, Roy's largest root, each with F, df and Sig., and with ETASQ and OPOWER partial eta squared, noncentrality and power), then the univariate tables with a row per dependent. HOMOGENEITY adds Box's M test of equal covariance matrices in the cells to Levene's test of each dependent; RSSCP gives the residual SSCP, covariance and correlation matrices and Bartlett's test of sphericity; TEST(SSCP) the SSCP of each effect and of the error. A custom test L B M = K takes L from LMATRIX (or the intercept's test when only MMATRIX or KMATRIX is given), M from MMATRIX (the identity by default; columns end with ';' and are named by their label or T1, T2, ...) and K from KMATRIX (rows end with ';', zero by default); CONTRAST always uses the identity; SPECIAL(matrix) takes the k x k matrix of a factor with k levels column by column, the first column the constant and the others one contrast each (the contrasts alone, k - 1 columns, also work), without fractions. Each test gives Contrast Results (K Matrix), Multivariate Test Results and Univariate Test Results; TEST(MMATRIX) shows M. EMMEANS, POSTHOC, PLOT and SAVE work on each dependent in turn (SAVE without names).

PLOT gives chart data. PROFILE draws the estimated marginal means as a line chart (bars with TYPE=BAR) over the levels of the first factor, one line per level of a second factor and one chart per level of a third; MEANREFERENCE adds the grand mean as a flat line. ERRORBAR=CI spans each mean's confidence interval at 1 minus CRITERIA ALPHA, ERRORBAR=SE(n) n standard errors on each side (2 by default); YAXIS is left to the drawing. SPREADLEVEL plots the mean of every cell with two cases or more against its standard deviation, then against its variance. RESIDUALS gives every case's predicted value, observed value and standardized residual. With WSFACTOR, PROFILE plots the means of the dependent variables over between-subjects and within-subjects factors and SPREADLEVEL and RESIDUALS are left out with a warning. GEF gives the general form of the estimable functions: a row per parameter, a column per free coefficient L1, L2, ... (a redundant parameter is a combination of them).

TEST(LMATRIX) (or ESTIMABLE) gives the L' matrix of every effect of the design for the sum of squares type in use (a row per parameter, a column per row of L), and of every LMATRIX and CONTRAST. TEST tests an effect of the design against an error term, either a linear combination of the mean squares of other effects (coefficients may be fractions; coefficients of one effect add up) with Satterthwaite's degrees of freedom unless DF(df) gives them, or a mean square with its DF(df); one table of Hypothesis and Error rows each. It is for one dependent variable and without WSFACTOR. ROBUST\[=HC0 \| HC1 \| HC2 \| HC3 \| HC4\] (HC3 when none is named) adds Robust Parameter Estimates and Robust Tests of Between-Subjects Effects (a Wald F of each effect's hypothesis rows, by the sum of squares type, with the error df) with the heteroskedasticity-consistent covariance, and a robust set of Contrast Results and Test Results for CONTRAST and LMATRIX; it is for one dependent variable and is ignored with a warning otherwise. RANDOM=factors makes those factors random, and every effect containing one (and no covariate): the Tests of Between-Subjects Effects then give each effect a Hypothesis and an Error row, the error term being the combination of the mean squares of the random effects and the residual that has the effect's expected mean square under its null hypothesis (Satterthwaite's df), and an Expected Mean Squares table gives the coefficient of each variance and the fixed effects of the quadratic term; not with WSFACTOR, a univariate analysis for each dependent, and the other tables (parameters, means, contrasts) keep the fixed-effects error with a warning. PRINT=MBP, BP, F and WHITE test for heteroskedasticity: the squared residuals (times the regression weight) are regressed on an auxiliary model, by default the constant, the fitted values and their squares, or the terms of MBPDESIGN, BPDESIGN and FDESIGN (a bare one names the model of DESIGN; an intercept is always in); White's uses every level, square and cross product of the design columns. One Heteroskedasticity Tests table (for each dependent variable of GLM) gives the chi-square or F, df and Sig. MEASURE=names (GLM) names the measures of a doubly multivariate model: the dependents are the cells of the within-subjects design for the first measure, then the second, and so on, the last WSFACTOR varying fastest; each measure has its own sphericity, averaged, contrast and between-subjects tests and the multivariate tests take the measures together.

## Example

```text
GLM t1 t2 t3 BY group /WSFACTOR=time 3 POLYNOMIAL /WSDESIGN=time /DESIGN=group.
```

`statscraft help GLM` prints the same text in the terminal. [All commands](README.md)
