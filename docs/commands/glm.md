<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# GLM

General linear model: one dependent variable as in UNIANOVA, several (multivariate analysis of variance and covariance), or repeated measures with WSFACTOR.

## Syntax

```text
GLM dependents [BY factors] [WITH covariates]
   [/WSFACTOR=name levels [POLYNOMIAL | contrast] ...] [/WSDESIGN=effects] [/MEASURE=names]
   [/MMATRIX=['label'] dependent value ... | ALL values [; ...]] [/KMATRIX=values [; ...]]
   [/PRINT=... [RSSCP] [TEST([SSCP] [LMATRIX | ESTIMABLE] [MMATRIX | TRANSFORM])]]
   [/every other subcommand of UNIANOVA]
```

The command is the general linear model of UNIANOVA extended to several dependent variables and to repeated measures. With one dependent and no WSFACTOR it is UNIANOVA: the same subcommands, defaults, rules and tables, all described under UNIANOVA. This page gives only what is added.

Several dependents without WSFACTOR make the multivariate model, a multivariate analysis of variance or covariance. Every effect gets Multivariate Tests (Pillai's trace, Wilks' lambda, Hotelling's trace and Roy's largest root, each with F, df and Sig.; with ETASQ and OPOWER also partial eta squared, noncentrality and power), then the univariate tables with a row per dependent. HOMOGENEITY adds Box's M test of equal covariance matrices to Levene's test of each dependent. RSSCP gives the residual SSCP, covariance and correlation matrices and Bartlett's test of sphericity; TEST(SSCP) gives the SSCP of each effect and of the error. EMMEANS, POSTHOC, PLOT and SAVE work on each dependent in turn (SAVE without names).

MMATRIX transforms the dependents for a custom test L B M = K: each column ends with ';' and is named by its label or T1, T2, ... (the identity by default). L comes from LMATRIX (the intercept's test when only MMATRIX or KMATRIX is given) and K from KMATRIX (rows end with ';', zero by default). CONTRAST always uses the identity. Each test gives Contrast Results (K Matrix), Multivariate Test Results and Univariate Test Results; TEST(MMATRIX) shows M.

WSFACTOR makes the dependents the levels of within-subjects factors, for example GLM t1 t2 t3 t4 t5 t6 /WSFACTOR=time 3 task 2: the first factor's levels change slowest, and the number of dependents must be the product of the levels (times the number of measures). The factor's contrast (POLYNOMIAL by default) gives the within-subjects contrasts. WSDESIGN lists the within-subjects effects (by default their full factorial). The tables are Multivariate Tests, Mauchly's Test of Sphericity, Tests of Within-Subjects Effects (sphericity assumed, Greenhouse-Geisser, Huynh-Feldt and lower bound), Tests of Within-Subjects Contrasts, and Tests of Between-Subjects Effects on the average of the levels. PROFILE may plot within-subjects factors too; SPREADLEVEL and RESIDUALS are left out with a warning.

MEASURE=names makes a doubly multivariate model: the dependents are the cells of the within-subjects design for the first measure, then for the second, and so on. Each measure gets its own sphericity, within-subjects, contrast and between-subjects tests, and the multivariate tests take the measures together. TEST, ROBUST and RANDOM are for one dependent without WSFACTOR; ROBUST is ignored with a warning otherwise, and RANDOM runs a univariate analysis of each dependent.

## Example

```text
GLM t1 t2 t3 BY group /WSFACTOR=time 3 POLYNOMIAL /WSDESIGN=time /DESIGN=group.
```

`statscraft help GLM` prints the same text in the terminal. [All commands](README.md)
