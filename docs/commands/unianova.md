<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# UNIANOVA

General linear model of one dependent variable: factorial ANOVA, ANCOVA and regression.

## Syntax

```text
UNIANOVA dependent [BY factors] [WITH covariates]
   [/DESIGN=effects] [/METHOD=SSTYPE(1 | 2 | 3 | 4)] [/INTERCEPT=INCLUDE | EXCLUDE]
   [/MISSING=EXCLUDE | INCLUDE] [/CRITERIA=[EPS(1E-8)] [ALPHA(0.05)]] [/REGWGT=var]
   [/PRINT=[DESCRIPTIVE] [HOMOGENEITY] [PARAMETER] [ETASQ] [OPOWER] [LOF] [GEF] [MBP] [BP] [F] [WHITE]
          [TEST(LMATRIX | ESTIMABLE)]]
   [/EMMEANS=TABLES(OVERALL | factor[*factor...]) [WITH(covariate=value | MEAN ...)]
             [COMPARE[(factor)] [ADJ(LSD | BONFERRONI | SIDAK)]]] ...
   [/POSTHOC=factor ... (tests) [VS effect]] ...
   [/CONTRAST(factor)=DEVIATION[(ref)] | SIMPLE[(ref)] | DIFFERENCE | HELMERT | REPEATED |
                      POLYNOMIAL[(metric)] | SPECIAL(matrix)] ...
   [/LMATRIX=['label'] effect coefficients ... [; ...] | ALL coefficients] ... [/KMATRIX=values [; ...]]
   [/TEST=effect VS coefficient*effect +/- ... [DF(df)] | value DF(df)] ...
   [/ROBUST[=HC0 | HC1 | HC2 | HC3 | HC4]] [/RANDOM=factors]
   [/MBPDESIGN[=effects]] [/BPDESIGN[=effects]] [/FDESIGN[=effects]]
   [/SAVE=tempvar[(name)] ...]
   [/OUTFILE=[COVB('file' | dataset)] [CORB('file' | dataset)] [HCCOVB('file' | dataset)] [EFFECT('file' | dataset)] [DESIGN('file' | dataset)]]
   [/PLOT=[SPREADLEVEL] [RESIDUALS] [PROFILE(factor[*factor[*factor]] ...) [WITH(covariate=value | MEAN ...)]
          [TYPE=LINE | BAR] [MEANREFERENCE=NO | YES] [ERRORBAR=NO | CI | SE[(2)]] [YAXIS=AUTO | 0]]] ...
```

The command fits a general linear model to one dependent variable: analysis of variance with factors after BY, analysis of covariance with covariates after WITH, or regression. Without DESIGN the model is the intercept, the covariates and the full factorial of the factors. Effects on DESIGN: INTERCEPT, a factor or covariate, a\*b or a BY b (interaction), a(b) or a WITHIN b (nesting, also a(b(c))), x\*x and x\*a. Sums of squares are Type III unless METHOD says otherwise (Type I is sequential). A case missing on any variable is left out (MISSING=INCLUDE keeps user-missing values), and the weight is rounded to a whole number. A design column that adds nothing (CRITERIA EPS) is redundant and its parameter is 0. For several dependent variables or repeated measures use GLM.

The main table is Tests of Between-Subjects Effects; ETASQ adds partial eta squared and OPOWER the noncentrality and observed power. DESCRIPTIVE gives cell means, PARAMETER the parameter estimates, LOF a lack of fit test. HOMOGENEITY gives Levene's test on the absolute residuals by cell; for a full factorial without covariates also its versions based on the median, the median with adjusted df and the trimmed mean.

EMMEANS gives estimated marginal means with the cells weighted equally and the covariates at their means or the WITH values; COMPARE adds pairwise comparisons and univariate tests, adjusted by ADJ. POSTHOC runs the post hoc tests of ONEWAY on the observed means with the model's error: LSD BONFERRONI SIDAK TUKEY SCHEFFE GH T2 SNK BTUKEY DUNCAN GT2 GABRIEL FREGW QREGW T3 C DUNNETT DUNNETTL DUNNETTR. T2, T3, GH and C need a model with one factor. VS names another effect of the design (not the intercept, nor a main effect on the same POSTHOC) as the error term of the pairwise and subset tests.

CONTRAST(factor) tests the factor's levels by a contrast type (DEVIATION by default); SPECIAL(matrix) takes a k x k matrix for k levels, column by column, the first column the constant (the k - 1 contrast columns alone also work), without fractions. LMATRIX gives custom contrasts of the parameters, rows ending with ';', and KMATRIX their hypothesized values (0 by default). Each gives Contrast Results and Test Results. TEST(LMATRIX) shows the L matrix of every effect, LMATRIX and CONTRAST; GEF gives the general form of the estimable functions.

TEST=effect VS ... tests an effect against an error term: a combination of the mean squares of other effects (coefficients may be fractions) with Satterthwaite's degrees of freedom unless DF(df) gives them, or a value with its DF(df). RANDOM=factors makes those factors random, with every effect that holds one: each effect is tested against the combination of mean squares that matches its expected mean square (Satterthwaite's df), and an Expected Mean Squares table is added. The other tables keep the fixed-effects error, with a warning.

ROBUST (HC3 when no type is named) adds Robust Parameter Estimates and Robust Tests of Between-Subjects Effects with a heteroskedasticity-consistent covariance, and robust results for CONTRAST and LMATRIX. PRINT=MBP, BP, F and WHITE test for heteroskedasticity by regressing the squared residuals on the fitted values and their squares, or on the effects of MBPDESIGN, BPDESIGN or FDESIGN (bare: the effects of DESIGN); WHITE uses every column, square and cross product of the design.

SAVE adds variables to the data: PRED WPRED RESID WRESID DRESID ZRESID SRESID SEPRED COOK LEVER, named PRE\_1, RES\_1 and so on unless a name is given. OUTFILE writes files for one dependent variable without RANDOM, each to a .sav file or a dataset made with DATASET DECLARE (a different one for each keyword; under SPLIT FILE the split variables come first and the groups follow one another): COVB the parameter covariance matrix and CORB their correlation matrix, as RowType\_ (COV or CORR), VarName\_ and a column P1, P2, ... for every parameter in the order of the Parameter Estimates (a redundant one blank), with a record per estimated parameter and then the records EST (the estimates, a redundant one 0), SE, DF (the error degrees of freedom) and SIG (the significance of t), named by the dependent variable; HCCOVB the covariance matrix of the robust estimator of ROBUST (needed) in the layout of COVB but with its diagonal terms only, and the robust standard errors and the significances they give; EFFECT the Tests of Between-Subjects Effects, a record per row of that table with SOURCE\_ and SS, DF, MS, F and SIG (and ETASQ, NONCENT and POWER when PRINT asks for them), a value the table leaves empty being missing; DESIGN the design matrix of the cases used, one variable DES\_1, DES\_2, ... for each parameter. PLOT gives chart data. PROFILE draws the estimated marginal means over the levels of the first factor, one line per level of a second factor and one chart per level of a third (TYPE=BAR for bars); MEANREFERENCE adds the grand mean, ERRORBAR=CI the confidence intervals at 1 minus ALPHA, and SE(n) n standard errors (2 by default). SPREADLEVEL plots each cell's mean against its standard deviation and variance; RESIDUALS plots the observed, predicted and standardized residual values.

## Example

```text
UNIANOVA score BY method school WITH pretest /PRINT=DESCRIPTIVE ETASQ PARAMETER /EMMEANS=TABLES(method) COMPARE ADJ(BONFERRONI) /DESIGN=pretest method school method*school.
```

`statscraft help UNIANOVA` prints the same text in the terminal. [All commands](README.md)
