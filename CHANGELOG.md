# Changelog

<!-- For writers: write for a user. Each release gets one or two plain sentences on what it brings,
then at most five one-line bullets of the main visible changes. No option-by-option lists,
no test or verification notes, no comparisons with other software, no internal history. -->

## Unreleased

- LEAVE takes a TO list of variables that the commands before it create (LEAVE total1 TO total3); before, such a list was refused as unknown variables.
- EXPORT keeps in portable files, and IMPORT reads from them by the file's own table, the symbols the format has a code for (plus-minus, degree, superscript digits, less-or-equal, greater-or-equal, not-equal, dagger, dashes, cent, bullet and a few more) in text; before, they became ?.
- DATA LIST, PRINT and WRITE refuse a format list with groups nested more than 32 deep or with more than a million formats, with an error at the place; before, a long run of parentheses overflowed the stack and repeats of repeats filled the memory. A group in DATA LIST repeated more than 10000 times is no longer cut at 10000.
- EXPORT writes text by the character table of the portable file: a letter with an accent or a symbol the table has no code for becomes ? with a warning (before, its UTF-8 bytes went in as they were and other programs could not read them), and # now has its code. Files written before still open.
- GET DATA /TYPE=TXT: an AUTO variable is numeric when DATATYPEMIN PERCENTAGE (95 unless given) of its non-blank values are numbers, and the others become system-missing; before, every value had to be a number and DATATYPEMIN was ignored for text files. In fixed columns a number format with decimals implies them in a value without a decimal point, as DATA LIST does (F5.2 reads 12345 as 123.45).
- NOMREG with both DELTA and BIAS gives an empty cell both, as PLUM does; before, DELTA had no effect once BIAS was above 0.
- DATA LIST reads the Z (zoned decimal) and PIBHEX (hexadecimal) formats, and STRING, PRINT and WRITE write PIBHEX; NUMBER reads JDATE, QYR, MOYR, WKYR, DATETIME, YMDHMS, TIME, MTIME, Z and PIBHEX.
- DATA LIST, GET DATA and ALTER TYPE read the date formats JDATE, QYR, MOYR and WKYR; DATE, ADATE, EDATE and SDATE dates and TIME, MTIME and YMDHMS values with no delimiters (28OCT1990, 10281990, 0530, 20110317140645); a plus sign on a time; a month spelled out or in Roman numerals after any delimiter in DATETIME, which reads a slash date with a first number above 12 day first; and they refuse minutes of 60 or more and seconds of 60 or more in a time. A bare four-digit MTIME value is now minutes and seconds (0719 is 7:19), as the format's form without a colon; other bare numbers stay minutes.
- NPTESTS Kolmogorov-Smirnov tests (one sample, and two independent samples) work out the significance with pi itself in the Smirnov series, as the formula is written, instead of the seven-digit constants of NPAR TESTS; the p-value changes by about 1e-7 for statistics between 0.27 and 1, NPAR TESTS is unchanged.
- AGGREGATE accepts a period after a function name (`SUM.(x)`, `PGT.(x, 30)`, `FIRST.(x)`, `N.(x)`, `NMISS.(x)`), which counts user-missing values as valid; before, such a call was refused with an unexpected character.
- MEAN and MEDIAN in COMPUTE no longer give system-missing for values near the largest number a computer holds, whose sum overflows but whose mean does not.
- The distribution functions of COMPUTE keep their digits far into the tails: CDF.GAMMA, CDF.CHISQ and the noncentral chi-square no longer lose the lower tail, SIG.F no longer gives 0 far out, and IDF.EXP, IDF.GAMMA, IDF.CHISQ, IDF.F, IDF.T, IDF.CAUCHY, IDF.WEIBULL and IDF.HALFNRM are exact for probabilities near 0 and 1.
- DISCRIMINANT says so when it ignores a subcommand of an analysis block that is written after a global one (CLASSIFY, STATISTICS and the others); before, a SAVE written after CLASSIFY was dropped without a message.
- [VERIFY](docs/commands/verify.md) reports the first and last case of the file and of the USE and PREDICT periods, flags date variables that are missing, out of range, not nested or stepping oddly, and lists one numeric series at those cases with its missing count.
- CATPCA with `/RESAMPLE=BOOTSTRAP` also draws joint plots of the data's points together with the points the bootstrap samples gave for the loadings, objects and categories.
- [KEYED DATA LIST](docs/commands/keyed-data-list.md) reads variables from the record of a text file that a key variable picks for each case: the record number for a number, the record that begins with the key for a string, with a variable that says whether a record was found, also inside an input program.
- UNIANOVA and GLM `/OUTFILE` also works under `SPLIT FILE`: each file holds the groups one after the other, the split variables in front.
- [GENLIN](docs/commands/genlin.md) `/MODEL DISTRIBUTION=TWEEDIE(q)` fits the compound Poisson-gamma distribution (q above 1 and below 2) for zero or positive responses, with the scale by maximum likelihood, deviance, Pearson or a fixed number and the Lagrange test.
- [ONEWAY](docs/commands/oneway.md) `/POSTHOC=WALLER(kratio)` gives the homogeneous subsets of the Waller-Duncan t test.
- [GENLIN](docs/commands/genlin.md) `/REPEATED` also takes the ordinal `MULTINOMIAL` distribution with the independent working correlation: the model's estimates with model-based or robust (sandwich over subjects) covariances.

## 0.53.0 (2026-11-26)

The window starts new data and takes new variables and cases where you type them, CATPCA gives bootstrap intervals and confidence ellipses, and SAVE CODEPAGE writes data files in a code page.

- File > New > Data opens an empty data window. A name typed in the empty row under the last variable of the Variables tab makes a new numeric variable, and a value typed in the empty row under the last case of the Data tab adds a case ([ADD CASE](docs/commands/add-case.md), a Statscraft extension). NUMERIC and STRING after NEW FILE start the blank dataset.
- [CATPCA](docs/commands/catpca.md) `/RESAMPLE=BOOTSTRAP` analyses balanced or unbalanced bootstrap samples of the cases, turned to the data's solution by a Procrustes rotation, with percentile intervals for the eigenvalues, loadings and category values, interval bands on the transformation plots and, for two dimensions, confidence ellipses (`LDELLAREA`, `OBELLAREA`, `CTELLAREA`, `NELLPNT`); `/SAVE` saves the ellipse areas and `/OUTFILE` takes `ELLCOORD` for their coordinates.
- [SAVE CODEPAGE](docs/commands/save-codepage.md) writes a data file whose text (names, labels, string values, notes) is in a code page of your choice, such as `'1252'` or `'windows-1257'`, with `KEEP`, `DROP` and `UNSELECTED`.
- [ALSCAL](docs/commands/alscal.md) `/FILE` takes `(FIXED)` after CONFIG, ROWCONF, COLCONF, SUBJWGHT and STIMWGHT: those coordinates and weights stay as the file gives them and the rest is solved for them.

## 0.52.0 (2026-11-26)

ALSCAL scales asymmetric and rectangular data with all its models, PLS saves its datasets under SPLIT FILE, and TWOSTEP CLUSTER updates a saved model.

- [ALSCAL](docs/commands/alscal.md) takes `/SHAPE=ASYMMETRIC` and `RECTANGULAR` (unfolding, with `/INPUT=ROWS(n)`), `/CONDITION=ROW`, and the models `ASCAL`, `AINDS` (stimulus weights) and `GEMSCAL` (a metric of its own for every matrix, with `/CRITERIA=DIRECTIONS(q)`).
- [PLS](docs/commands/pls.md) `/OUTDATASET` works under SPLIT FILE: every group is fitted and the CASES, LATENTFACTORS and PREDICTORS datasets hold the groups one after the other, the split variables in front.
- [TWOSTEP CLUSTER](docs/commands/twostep-cluster.md) `/INFILE` updates a model saved with `/OUTFILE STATE`: the saved subclusters and the cases of the active file go into the tree together, with the saved distance, standardization and limits.

## 0.51.0 (2026-11-26)

LOGLINEAR fits general loglinear and logit models, SAVE TRANSLATE writes SAS transport and dBASE files, and stacked plugin tables line up their columns on one Excel sheet.

- Excel export on one sheet: tables in a plugin's stacked layout line up their data columns, so the total of a means table stands under the total of the percentage tables.
- [SAVE TRANSLATE](docs/commands/save-translate.md) writes SAS transport files with `/TYPE=SAS /VERSION=X`: labels, date, time and datetime formats, and one missing value.
- [SAVE TRANSLATE](docs/commands/save-translate.md) writes dBASE III and IV files with `/TYPE=DB3` and `/TYPE=DB4`: fields from the print formats, dates as date fields, a missing number as asterisks.
- [LOGLINEAR](docs/commands/loglinear.md) fits general loglinear and logit models of a table of counts with contrasts for every factor (DEVIATION, SIMPLE, DIFFERENCE, HELMERT, REPEATED, POLYNOMIAL, SPECIAL and BASIS SPECIAL), cell covariates, cell weights and structural zeros, several DESIGNs with settings that carry over, and generalized residuals: cell counts with raw, standardized and adjusted residuals, the likelihood-ratio and Pearson tests, the design matrix, parameter estimates with z values and intervals and their correlation, the analysis of dispersion for a logit model, and residual plots.
- [GRAPH](docs/commands/graph.md) accepts `/TEMPLATE`: it is read and ignored with a note, since charts come as data.
- [CLUSTER](docs/commands/cluster.md) `/SAVE` works with `/MATRIX=IN`: the matrix of the active dataset's cases gives the membership variables.

## 0.50.0 (2026-11-26)

Nonlinear regression arrives with NLR and CNLR, the time series commands gain RMV, FIT and SHIFT VALUES, MCONVERT converts matrix data, and the window speaks Spanish.

- The window speaks Spanish: Edit > Settings > Language chooses System (the default, Spanish when the computer is set to Spanish), English or Español, and the change applies at once. The menus, toolbars, status bar, grids, the Settings dialog and the buttons of the dialogs follow it; syntax, output tables and command messages stay the same in every language. See [The language of the window](docs/window.md#the-language-of-the-window).
- The README is also in Spanish and Simplified Chinese.
- [RMV](docs/commands/rmv.md) copies series with their missing values filled in: `LINT` interpolates, `MEAN` and `MEDIAN` use the nearest valid values on each side, `SMEAN` the series mean and `TREND` a fitted line.
- [NLR](docs/commands/nlr.md) fits nonlinear models by least squares (Levenberg-Marquardt) to the model that [MODEL PROGRAM](docs/commands/model-program.md) and [DERIVATIVES](docs/commands/derivatives.md) define: iteration history, analysis of variance with R squared, estimates with asymptotic standard errors and 95% intervals, parameter correlations, FILE and OUTFILE for starting values, SAVE of predicted values, residuals and derivatives, and a check of the derivatives given against numerical ones. CLEAR MODEL PROGRAMS drops the programs.
- [SHIFT VALUES](docs/commands/shift-values.md) adds a variable holding the value of another variable from an earlier or later case, within each split group, with the dictionary entry of the original.
- [MCONVERT](docs/commands/mconvert.md) turns matrix data of covariances into correlations with standard deviations, and back, set by set, in the active dataset or a file.
- [FIT](docs/commands/fit.md) gives goodness-of-fit statistics of residual series from the time series commands: mean, absolute and squared errors, the Durbin-Watson statistic, percent errors against the observed series, and separate columns for a PREDICT period inside the series.
- [CNLR](docs/commands/cnlr.md) fits the same model under constraints: simple bounds, linear constraints and bounds on [CONSTRAINED FUNCTIONS](docs/commands/constrained-functions.md) computed from the parameters, with an optional LOSS function to minimize instead of the squared residuals and BOOTSTRAP for standard errors, correlations and 95% trimmed ranges. It gives the iteration history and, for squared residuals, the tables of NLR; FILE, OUTFILE and SAVE work as in NLR.

## 0.49.0 (2026-11-25)

GRAPH draws grouped high-low charts, MVA writes its filled data under SPLIT FILE, CURVEFIT can apply an earlier fit as it is, and INCLUDE keeps inline data intact.

- [MVA](docs/commands/mva.md) writes its OUTFILE under SPLIT FILE: each group is filled from its own estimates, all in one file.
- [GRAPH](docs/commands/graph.md) `/HILO` draws grouped high-low and high-low-close charts: with more than three series, each group of two or three stands beside the next in a category.
- `INCLUDE` reads the inline data of a file as it is: a data line that starts with a minus sign or a period keeps it, and fixed-format records no longer get a blank record between them.
- [CURVEFIT](docs/commands/curvefit.md) `/APPLY FIT` keeps the estimates of the earlier fit: the tables are the original ones, and the plot and the saved variables put its coefficients to use on the current data.

## 0.48.0 (2026-11-25)

GGRAPH draws error bars, and plugin presets can be corrected in place.

- [GGRAPH](docs/commands/ggraph.md) reads the interval functions `MEANCI`, `MEDIANCI`, `MEANSD`, `MEANSE` and `COUNTCI`: each gives a value and its high and low end, `VARSTOCASES` takes them in `HIGH` and `LOW` columns, and an `interval(position(region.spread.range(x*(low+high))))` element draws them as error bars around the other element.
- Plugin Settings: choose a preset, change its value or name and click **Save** to keep the change in that preset.
- Whole numbers in the plugin settings show without a decimal point, for example a population of 1300000.
- Style samples are black on white as in Excel, whatever the window's theme.

## 0.47.0 (2026-11-25)

The syntax editor finds and replaces text.

- The syntax editor finds and replaces text: Ctrl+F opens a find bar and Ctrl+H a replace row, with Match case, Whole word, F3 for the next match and Replace all. See [the window](docs/window.md#find-and-replace).

## 0.46.0 (2026-11-25)

Expressions read today's date and the page size, GLM names its saved variables per dependent variable, and MANOVA shows the decomposition and collinearity of its design.

- [MANOVA](docs/commands/manova.md) `/PRINT=DESIGN(DECOMP)` shows the triangular decomposition of the design and `DESIGN(COLLINEARITY)` its collinearity diagnostics.
- [GLM](docs/commands/glm.md) `/SAVE` names the new variable of each dependent variable, for example `PRED(p1 p2)`, also in the multivariate and repeated measures models.
- Expressions read the system variables $JDATE, $DATE, $DATE11 and $TIME (today and now) and $LENGTH and $WIDTH (the SET page size).

## 0.45.0 (2026-11-25)

UNIANOVA and GLM write their effects table as a data file, APPLY can repeat any earlier model of the session, and GGRAPH computes the percentile of grouped values.

- GGRAPH and XGRAPH compute GPTILE, the percentile of grouped values.
- `APPLY 'MOD_2'` repeats any earlier model of the session, not only the previous one: ACF, PACF, CCF, SEASON, EXSMOOTH, ARIMA, AREG, SPECTRA, 2SLS, TSPLOT, CASEPLOT, CURVEFIT, WLS and PPLOT number their models MOD_1, MOD_2, ... in the order the session runs them.
- [UNIANOVA](docs/commands/unianova.md) and GLM write the Tests of Between-Subjects Effects as a data file with `/OUTFILE EFFECT`.

## 0.44.0 (2026-11-25)

COXREG writes its coefficients and survival tables to files, and the robust covariance file of GLM holds the diagonal only again.

- [COXREG](docs/commands/coxreg.md) writes the coefficients of the final model and the survival table of every stratum (at the baseline, at the means and for each PATTERN line) with `/OUTFILE COEFF` and `TABLE`.
- [UNIANOVA](docs/commands/unianova.md) and GLM: the file of `/OUTFILE HCCOVB` holds the diagonal terms of the robust covariance matrix only, blank off the diagonal (0.43.0 wrote the whole matrix).

## 0.43.0 (2026-11-25)

UNIANOVA and GLM save their robust covariance matrix, and a plugin setting can hold for every data file.

- [UNIANOVA](docs/commands/unianova.md) and GLM write the robust covariance matrix of `/ROBUST` with `/OUTFILE HCCOVB`.
- A plugin setting can hold for every data file: the window keeps the last value chosen on the computer, also with no data open, and marks it (all data files) in Plugin Settings.

## 0.42.0 (2026-11-25)

AUTORECODE keeps its codes from file to file, ALTER TYPE reads more string formats, and ALSCAL can start from a saved configuration.
- [AUTORECODE](docs/commands/autorecode.md) saves its scheme to a template file (`/SAVE TEMPLATE`) and applies one (`/APPLY TEMPLATE`), so another file is recoded with the same codes and its new values get the next ones.
- [ALTER TYPE](docs/commands/alter-type.md) turns a string into a date (DATE, ADATE, EDATE, SDATE) or a number in the E, DOLLAR, PCT and N formats, as well as F, COMMA and DOT.
- ALSCAL `/FILE` starts the iterations from the coordinates (and, for INDSCAL, the subject weights) of a configuration file, such as the one its own OUTFILE writes.
- ONEWAY, PPLOT and SPCHART accept `/TEMPLATE` and ignore it with a note, as the other commands with charts do, so scripts that name a chart template run.

## 0.41.0 (2026-11-25)

More procedures write their parameter matrices to a file or dataset, and two more commands repeat with /APPLY.

- [REGRESSION](docs/commands/regression.md) writes the covariance (`/OUTFILE COVB`) or correlation (`CORB`) matrix of the coefficients of its last equation, with the estimates, standard errors, significances and degrees of freedom, to a file or dataset.
- [UNIANOVA](docs/commands/unianova.md) and GLM with one dependent variable write the parameter covariance matrix (`/OUTFILE COVB`), their correlation matrix (`CORB`) and the design matrix (`DESIGN`) to files or datasets.
- [GENLIN](docs/commands/genlin.md) writes the covariance or correlation matrix of its parameters, with the estimates, standard errors, significances and degrees of freedom, to a file or dataset (`/OUTFILE COVB=` or `CORB=`), and `/CRITERIA INITIAL=` reads start values from such a file or dataset, group by group under SPLIT FILE.
- [STRING](docs/commands/string.md) also writes the custom currency formats CCA to CCE, as SET defines them.
- CURVEFIT and WLS take `/APPLY`: the previous command's specifications run again on the same or new variables, with the changes the new command gives.

## 0.40.0 (2026-11-25)

DISCRIMINANT can work from a matrix file instead of the cases.

- [DISCRIMINANT](docs/commands/discriminant.md) `/MATRIX=IN` reads the group counts, means and pooled deviations and correlations of a matrix file: with `IN(*)` the analysis is made from the matrix alone, with a file or dataset name the cases of the active dataset are classified by it.
- [DISCRIMINANT](docs/commands/discriminant.md) `/MATRIX=OUT` writes each group's own deviations and correlations when `BOXM`, `GCOV` or `CLASSIFY=SEPARATE` is given, and `/MATRIX=IN` reads them back.

## 0.39.0 (2026-11-25)

Case influence in TREE works with every dependent in CHAID and CRT.

- [TREE](docs/commands/tree.md) `/INFLUENCE` also works with CRT and a nominal or ordinal dependent.
- [TREE](docs/commands/tree.md) `/INFLUENCE` also works with a scale dependent, in CHAID and in CRT.

## 0.38.1 (2026-11-25)

Two fixes to CHAID trees.

- TREE: exhaustive CHAID no longer stops with an error when an ordinal predictor has only its missing category in a node.
- TREE /INFLUENCE: the risk of a CHAID tree is the share of the cases (their frequencies) that the tree misclassifies, as without an influence.

## 0.38.0 (2026-11-24)

CHAID trees can split merged categories again and weigh cases by their influence.

- [TREE](docs/commands/tree.md) `/CHAID SPLITMERGED=YES`: a merged group of three or more categories is split again where that split is significant at `ALPHAMERGE`.
- [TREE](docs/commands/tree.md) `/INFLUENCE`: with CHAID and a nominal or ordinal dependent, each case counts in growing the tree by the value of an influence variable.

## 0.37.0 (2026-11-24)

TREE finds the nodes that answer best for a target category.

- [TREE](docs/commands/tree.md) `/TARGETRESPONSE`: for each target category, the terminal nodes ranked by their response rate, with the rule that leads to each, the cumulative response and whether it reaches a minimum rate you set, and a chart of the cumulative response.

## 0.36.0 (2026-11-23)

Plugins lay out tables of means more compactly and can offer their settings as a list.

- In a plugin's compact layout, a table of means shows each question in its mean row, with no row of its own above it, in Excel and on web pages.
- A plugin setting can offer a fixed set of values, chosen from a list in the plugin's settings.

## 0.35.0 (2026-11-23)

Undo and redo go more than one step.

- Undo goes back more than one change: Edit > Undo takes back up to the last 10 runs that changed the data, one at a time, and the data file that was open comes back with them. New Edit > Redo (Ctrl+Y or Ctrl+Shift+Z) does a run again; a new change ends the redo. Both menu items name the run they act on. `statscraft redo` and the MCP tool `redo` do the same as Edit > Redo, and `undo` in both goes one change further back on each call.

## 0.34.0 (2026-11-23)

The command pages are easier to use: even in length, with examples on the sample file and their output.

- The 20 most used commands, such as [FREQUENCIES](docs/commands/frequencies.md), [CTABLES](docs/commands/ctables.md) and [COMPUTE](docs/commands/compute.md), have examples on `survey.sav` with the table they give.
- [TREE](docs/commands/tree.md) describes all four growing methods: CHAID, exhaustive CHAID, CRT and QUEST.
- Completion in the syntax editor offers a function with its parenthesis inside an expression.
## 0.33.0 (2026-11-23)

The documentation is rewritten for users, with a sample file to follow along.

- [Getting started](docs/getting-started.md) goes from installing to an Excel workbook with a custom table, in the window, on the sample file `survey.sav`.
- The window guide lists every menu item with the command it runs, and explains custom tables step by step.
- [MANOVA](docs/commands/manova.md) draws boxplots, cell plots, normal plots, predicted means and residual plots.

## 0.32.0 (2026-11-23)

MANOVA can now save its cell statistics to a matrix file and run from one, and the help and documentation are easier to read.

- [MANOVA](docs/commands/manova.md) /MATRIX writes cell counts, means and pooled correlations to a matrix file, and runs an analysis from such a file or from a CORRELATIONS matrix.
- MANOVA /ANALYSIS runs several groups of analyses in one command, also with repeated measures.
- The help and the command pages describe only what Statscraft does and are easier to read on the documentation site.

## 0.31.0 (2026-11-23)

Multidimensional scaling with PROXSCAL can now be tied to outside variables or fixed coordinates, and repeated measures in GLM get profile plots.

- [PROXSCAL](docs/commands/proxscal.md) can build the common space from independent variables, or hold chosen coordinates fixed.
- PROXSCAL can start from coordinates in a file and plots how the independent variables correlate with the dimensions.
- [GLM](docs/commands/glm.md) draws profile plots of estimated marginal means for repeated measures designs.
- [SURVIVAL](docs/commands/survival.md) can write its life tables to a file.

## 0.30.0 (2026-11-23)

Classification trees can now weigh costs and profits, check themselves on held-out data, and write their rules as syntax or SQL. GLM repeated measures gets the full set of tests and saved values.

- [TREE](docs/commands/tree.md) writes the tree as rules: syntax you can run, SQL, or plain sentences.
- TREE takes misclassification costs, profits and adjusted priors, and validates a tree by split sample or cross-validation.
- TREE CHAID handles a scale dependent, and gain tables come by percentile as well as by node.
- [GLM](docs/commands/glm.md) with repeated measures takes EMMEANS, POSTHOC, contrasts, custom hypotheses, SAVE and the extra printed tables.
- [GENLIN](docs/commands/genlin.md) gives score tests with repeated measures, [GENLINMIXED](docs/commands/genlinmixed.md) fits fixed effects alone, and [ADP](docs/commands/adp.md) can build new predictors from correlated inputs.

## 0.29.0 (2026-11-23)

TREE grows QUEST trees, GLM fits several measures over one repeated measures design, and post hoc tests are faster and more accurate.

- [TREE](docs/commands/tree.md) /METHOD TYPE=QUEST grows binary trees for a nominal dependent.
- [GLM](docs/commands/glm.md) /MEASURE fits doubly multivariate repeated measures.
- Post hoc tests in ONEWAY, UNIANOVA and GLM run 10 to 40 times faster, and their critical values are now accurate with very few degrees of freedom.
- [NPAR TESTS](docs/commands/npar-tests.md) says at once when an exact test would take too long, and the exact tests that fit run faster.

## 0.28.0 (2026-11-22)

TREE grows CRT trees, UNIANOVA and GLM take random factors, and more procedures write matrix files.

- [TREE](docs/commands/tree.md) CRT grows binary trees for categorical or scale dependents, with priors, surrogate splits, pruning and predictor importance.
- [UNIANOVA](docs/commands/unianova.md) and GLM /RANDOM handle random factors, testing each effect against the right error term; the Univariate dialog offers them.
- FACTOR, DISCRIMINANT and CLUSTER write matrix files, and FACTOR reads a loading matrix.
- GENLIN gives profile likelihood intervals for the scale and the negative binomial parameter.
- NPAR TESTS gives the exact significance of the marginal homogeneity test.

## 0.27.0 (2026-11-22)

Correlations get confidence intervals, GENLIN gets profile likelihood intervals, more nonparametric tests are exact, and GLM tests for heteroskedasticity. The dialogs offer all of these.

- [CORRELATIONS](docs/commands/correlations.md) and NONPAR CORR give confidence intervals for each coefficient and write matrix files.
- [GENLIN](docs/commands/genlin.md) gives profile likelihood intervals and takes starting values.
- [NPAR TESTS](docs/commands/npar-tests.md) gives exact significances for the two-sample Kolmogorov-Smirnov, median, Kruskal-Wallis, Jonckheere-Terpstra, Friedman, Kendall's W and Cochran tests.
- [FACTOR](docs/commands/factor.md) analyzes the covariance matrix and writes matrix files; [DISCRIMINANT](docs/commands/discriminant.md) rotates its functions.
- UNIANOVA and GLM test for heteroskedasticity (Breusch-Pagan, F and White tests).

## 0.26.0 (2026-11-22)

UNIANOVA and GLM give robust standard errors, and optimal scaling groups variables into many categories more precisely.

- [UNIANOVA](docs/commands/unianova.md) and GLM /ROBUST give robust estimates and tests (HC0 to HC4).
- CATREG, CATPCA and related commands find the cut points of many normal-shaped groups precisely.
- The Multidimensional Unfolding dialog starts from, or fixes, coordinates read from a file.

## 0.25.0 (2026-11-22)

More procedures read and write matrix data files, BOOTSTRAP gains two resampling methods built on residuals, and the DM command names work.

- [REGRESSION](docs/commands/regression.md), ONEWAY, RELIABILITY and PARTIAL CORR read and write matrix data files.
- [BOOTSTRAP](docs/commands/bootstrap.md) resamples residuals (RESIDUAL and WILD).
- [PREFSCAL](docs/commands/prefscal.md) fixes row or column coordinates and starts from given ones.
- [TSMODEL](docs/commands/tsmodel.md) and TSAPPLY draw charts that compare the models, in syntax and in the dialogs.
- DMLOGISTIC, DMROC, DMTREE and DMTABLES run LOGISTIC REGRESSION, ROC, TREE and CTABLES.

## 0.24.0 (2026-11-21)

GENLIN, NOMREG, GLM and UNIANOVA show more of how their tests are built, and GLM tests an effect against an error term you choose.

- [GENLIN](docs/commands/genlin.md) prints estimable functions, L matrices, the iteration history and Lagrange multiplier tests.
- [NOMREG](docs/commands/nomreg.md) /TEST tests linear combinations of the parameters.
- [GLM](docs/commands/glm.md) and UNIANOVA print estimable functions and L matrices, and /TEST tests an effect against a chosen error term.
- The Univariate, Multivariate and Generalized Linear Models dialogs offer these tables.

## 0.23.0 (2026-11-21)

REGRESSION can fit a subset of cases and report on the rest, NOMREG gives more tables, and GLM post hoc tests take a chosen error term.

- [REGRESSION](docs/commands/regression.md) /SELECT fits the selected cases and reports predictions and residuals for all; the dialog has a selection variable.
- [NOMREG](docs/commands/nomreg.md) adds cell probabilities, measures of association and the iteration history, also in the dialog.
- GLM and UNIANOVA /POSTHOC take an error term with VS.

## 0.22.0 (2026-11-21)

REGRESSION tests sets of predictors, CROSSTABS writes its cells to a file, and GET and SAVE list how variables map.

- [REGRESSION](docs/commands/regression.md) /METHOD=TEST tests each set of predictors by its change in R square.
- [CROSSTABS](docs/commands/crosstabs.md) /WRITE writes one record per cell to the file named by the new [PROCEDURE OUTPUT](docs/commands/procedure-output.md) command.
- GET and SAVE /MAP list the variables with their names on both sides.
- GLM and UNIANOVA read a SPECIAL contrast matrix column by column, the first column being the constant.

## 0.21.0 (2026-11-21)

ONEWAY gains trend tests, more Levene tests and effects rows, RELIABILITY gives McDonald's omega, and LOGISTIC REGRESSION can fit a subset of cases. The dialogs have the new choices.

- [ONEWAY](docs/commands/oneway.md) splits the between-groups sum of squares into trends (/POLYNOMIAL) and adds fixed and random effects rows.
- ONEWAY /STATISTICS=HOMOGENEITY gives four Levene tests; scripts that read that table by column position should note its new second column.
- [RELIABILITY](docs/commands/reliability.md) /MODEL=OMEGA gives McDonald's omega.
- [LOGISTIC REGRESSION](docs/commands/logistic-regression.md) /SELECT fits the selected cases and reports the others.
- [DESCRIPTIVES](docs/commands/descriptives.md) /SORT orders the variables by any statistic.

## 0.20.6 (2026-11-20)

The window's Excel export now puts all tables on one sheet by default, Windows shows Statscraft's own icons for its data, syntax and output files, and the status bar shows the version.

## 0.20.5 (2026-11-20)

Fixes for exact tests, updates on Windows and sessions.

- CROSSTABS exact tests use memory in proportion to what the computer has free, so larger tables get exact results.
- Windows: Update in the window now installs reliably; a window of 0.20.4 or older needs the installer run once by hand.
- A file session recovers its output after an interrupted save.
- GET DATA /TYPE=TXT accepts /MAP after /VARIABLES.

## 0.20.4 (2026-11-20)

Plugin settings travel between computers, and OUTPUT MODIFY can hide a heading alone.

- Edit > Plugin Settings can export and import templates; `--plugin-template` and the MCP server apply one for a run.
- [OUTPUT MODIFY](docs/commands/output-modify.md) VISIBLE=NO on HEADINGS hides only the heading and keeps its tables; use OUTLINEHEADERS to hide both.

## 0.20.3 (2026-11-20)

Command line and MCP sessions keep their output between runs, so several runs can be exported once at the end, and plugins can be checked for updates from the command line.

## 0.20.2 (2026-11-19)

The command line and MCP server reach further into the open window: undo the last change, read its output, close a syntax window, and apply plugins to its workbook.

## 0.20.1 (2026-11-19)

The command line and MCP server export the open window's output to Excel, a web page or PDF, write charts as pictures, and can leave table titles out of the workbook.

## 0.20.0 (2026-11-19)

One Export dialog for the output, and plugins for agents.

- File > Export... writes Excel, a web page or PDF, all items or only the selected tables.
- The MCP server applies plugins to an agent's workbook and lists a plugin's settings.
- A plugin's own settings decide whether it writes comments, in the window and in Excel.
- On Linux, Update installs the new .deb package itself.

## 0.19.1 (2026-11-18)

On Windows, running a plugin no longer flashes a console window.

## 0.19.0 (2026-11-18)

Plugins can update themselves: the window checks once a day and offers an Update button. See [plugins](docs/plugins.md).

## 0.18.0 (2026-11-18)

A large release: generalized linear mixed models, classic MANOVA, neural networks, simulation, time series models, survey sampling and many more commands, with dialogs for nearly all of them.

- Models: [GENLINMIXED](docs/commands/genlinmixed.md), [MANOVA](docs/commands/manova.md), [VARCOMP](docs/commands/varcomp.md), [QUANTILE REGRESSION](docs/commands/quantile-regression.md), [PLS](docs/commands/pls.md), [CATREG](docs/commands/catreg.md), [PREFSCAL](docs/commands/prefscal.md), OVERALS and the older optimal scaling commands.
- Prediction: [MLP](docs/commands/mlp.md), [RBF](docs/commands/rbf.md), [KNN](docs/commands/knn.md), [NAIVEBAYES](docs/commands/naivebayes.md), [ROC ANALYSIS](docs/commands/roc-analysis.md), [AIM](docs/commands/aim.md), [SELECTPRED](docs/commands/selectpred.md), [ADP](docs/commands/adp.md), [OPTIMAL BINNING](docs/commands/optimal-binning.md), [RESPONSE RATE](docs/commands/response-rate.md).
- [BOOTSTRAP](docs/commands/bootstrap.md) for many procedures, exact and Monte Carlo tests in CROSSTABS, simulation with [SIMPLAN](docs/commands/simplan.md) and SIMRUN, and [TSMODEL](docs/commands/tsmodel.md) time series models.
- Surveys and quality: [CSPLAN](docs/commands/csplan.md) sampling with [CSSELECT](docs/commands/csselect.md), [CSCOXREG](docs/commands/cscoxreg.md), [PROPORTIONS](docs/commands/proportions.md), [SPCHART](docs/commands/spchart.md), [OLAP CUBES](docs/commands/olap-cubes.md), and conjoint studies with [CONJOINT](docs/commands/conjoint.md).
- Charts and output: [XGRAPH](docs/commands/xgraph.md), [GGRAPH](docs/commands/ggraph.md), [IGRAPH](docs/commands/igraph.md), the [OMS](docs/commands/oms.md) commands, and several plugins at once.

## 0.17.0 (2026-11-17)

CROSSTABS gets its full set of statistics, and TWOSTEP CLUSTER, DETECTANOMALY and CSORDINAL are new. Plugins gain design templates.

- [CROSSTABS](docs/commands/crosstabs.md) gives every statistic (kappa, risk, McNemar, Cochran-Mantel-Haenszel and the rest), a case processing summary, and runs fast on large tables; the dialog offers them all.
- [TWOSTEP CLUSTER](docs/commands/twostep-cluster.md) clusters cases from mixed variables, and [DETECTANOMALY](docs/commands/detectanomaly.md) finds unusual cases.
- [CSORDINAL](docs/commands/csordinal.md) fits ordinal regression on a complex sample.
- [AGGREGATE](docs/commands/aggregate.md) has the percentage, fraction and count functions; DATA LIST reads date and time fields; SAVE writes the compression asked for.
- Plugins: save settings as named design templates, one look per client.

## 0.16.0 (2026-11-16)

Plugins arrive: a program of your own can colour, comment and lay out custom tables, in the window and in Excel. SPECTRA, AREG and 2SLS are new.

- Plugins colour cells, add comments and set fonts, borders and column widths. See [plugins](docs/plugins.md).
- `statscraft run --xlsx FILE --plugin PROGRAM` applies a plugin to the workbook.
- Every part of the output can be shown or hidden in the navigator. See [the window](docs/window.md#what-the-output-shows).
- [SPECTRA](docs/commands/spectra.md), [AREG](docs/commands/areg.md) and [2SLS](docs/commands/2sls.md) for spectral analysis and regression with autocorrelated errors or instruments.
- On Linux the window follows the GNOME light or dark setting.

## 0.15.0 (2026-11-16)

Time series, Bayesian statistics, power analysis, newer nonparametric tests and categorical principal components, each with its dialogs.

- Time series: [EXSMOOTH](docs/commands/exsmooth.md), [ARIMA](docs/commands/arima.md), [ACF](docs/commands/acf.md), PACF, CCF, [SEASON](docs/commands/season.md), [CREATE](docs/commands/create.md), [TSET](docs/commands/tset.md), [TSPLOT](docs/commands/tsplot.md), [USE](docs/commands/use.md) and [PREDICT](docs/commands/predict.md).
- Bayesian statistics: [BAYES ONESAMPLE](docs/commands/bayes-onesample.md) and the other BAYES commands, from correlations to log-linear models.
- Power analysis: every POWER command, such as [POWER MEANS ONESAMPLE](docs/commands/power-means-onesample.md), with power curves; no data needed.
- [NPTESTS](docs/commands/nptests.md), [CATPCA](docs/commands/catpca.md) and [MULTIPLE CORRESPONDENCE](docs/commands/multiple-correspondence.md).
- [COMPARE DATASETS](docs/commands/compare-datasets.md) and [VALIDATEDATA](docs/commands/validatedata.md) for checking survey data.

## 0.14.0 (2026-11-15)

Stata files, multiple imputation, loglinear models and a set of new procedures, each with its dialog.

- [GET STATA](docs/commands/get-stata.md) reads Stata files and [SAVE TRANSLATE](docs/commands/save-translate.md) /TYPE=STATA writes them.
- [MULTIPLE IMPUTATION](docs/commands/multiple-imputation.md) makes completed copies of the data, and many procedures pool their results over them.
- [HILOGLINEAR](docs/commands/hiloglinear.md) and [GENLOG](docs/commands/genlog.md) fit loglinear models.
- [PROBIT](docs/commands/probit.md), [CURVEFIT](docs/commands/curvefit.md), [WLS](docs/commands/wls.md), [WEIGHTED KAPPA](docs/commands/weighted-kappa.md), [FLEISS MULTIRATER KAPPA](docs/commands/fleiss-multirater-kappa.md) and [RATIO STATISTICS](docs/commands/ratio-statistics.md).
- [CODEBOOK](docs/commands/codebook.md) and [SUMMARIZE](docs/commands/summarize.md).

## 0.13.0 (2026-11-15)

Survival analysis, distances and multidimensional scaling, and missing value analysis, each with its dialog.

- [KM](docs/commands/km.md), [SURVIVAL](docs/commands/survival.md) and [COXREG](docs/commands/coxreg.md), with [TIME PROGRAM](docs/commands/time-program.md) for time-dependent covariates.
- [PROXIMITIES](docs/commands/proximities.md), [PROXSCAL](docs/commands/proxscal.md) and [ALSCAL](docs/commands/alscal.md).
- [MVA](docs/commands/mva.md) describes missing values and estimates the data under them.
- LOGISTIC REGRESSION handles constant covariates and stepwise removal better.
- With SPLIT FILE, saved variables keep the values of every group.

## 0.12.0 (2026-11-14)

GLM fits multivariate models, complex samples get regression, and MIXED and GENLIN grow.

- [GLM](docs/commands/glm.md) with several dependent variables: multivariate tests, SSCP matrices and custom tests, also in the Multivariate dialog.
- [CSGLM](docs/commands/csglm.md) and [CSLOGISTIC](docs/commands/cslogistic.md) fit linear and logistic models on a complex sample.
- [MIXED](docs/commands/mixed.md) gains eleven covariance structures, Kenward-Roger degrees of freedom and predicted random effects.
- [GENLIN](docs/commands/genlin.md) /REPEATED fits generalized estimating equations.
- Runs sent to the open window read relative file names from the caller's folder.

## 0.11.0 (2026-11-13)

Generalized linear models, correspondence analysis, complex sample descriptives and a Graphs menu in the window.

- [GENLIN](docs/commands/genlin.md) fits generalized linear models for many distributions and links.
- [CORRESPONDENCE](docs/commands/correspondence.md) analyzes two-way tables.
- [CSPLAN](docs/commands/csplan.md), [CSDESCRIPTIVES](docs/commands/csdescriptives.md) and [CSTABULATE](docs/commands/cstabulate.md) estimate from complex samples.
- NOMREG steps through effects, PLUM tests custom hypotheses, MIXED gives estimated marginal means and custom tests.
- The window has a Graphs menu and new dialogs for regression, mixed models and complex samples. See [the window](docs/window.md#analyze).

## 0.10.0 (2026-11-13)

The GRAPH command arrives, along with PDF export, ordinal and multinomial regression and mixed models.

- [GRAPH](docs/commands/graph.md) draws bar, line, area, pie, error bar, Pareto, high-low and scatter charts and histograms, with panels.
- [PPLOT](docs/commands/pplot.md) and [ROC](docs/commands/roc.md) give probability plots and ROC curves.
- [OUTPUT EXPORT](docs/commands/output-export.md) writes PDF and picture files.
- [NOMREG](docs/commands/nomreg.md), [PLUM](docs/commands/plum.md) and [MIXED](docs/commands/mixed.md).
- [OUTPUT MODIFY](docs/commands/output-modify.md) edits output, and agents can add charts given as JSON. See [charts as input](docs/command-line.md#charts-as-input).

## 0.9.0 (2026-11-12)

Charts are drawn and can be edited in office programs, output can be saved, and every step an agent takes leaves syntax.

- The output window draws charts, and saves them as PNG or SVG. See [the window](docs/window.md#charts).
- Charts open as editable charts in Excel, Word and PowerPoint, and OUTPUT EXPORT writes Word and PowerPoint files.
- Output saves as a `.scout` file and opens again (OUTPUT SAVE, OUTPUT OPEN).
- Each agent run is added to an Agent syntax window, and the MCP server can read and edit syntax windows. See [the MCP server](docs/mcp.md#every-step-leaves-syntax).
- AI apps are connected in Edit > Settings > AI agents, including agents inside WSL. See [Connect your AI agent](docs/connect.md).

## 0.8.1 (2026-11-12)

Help > Connect an AI agent connects Statscraft to AI apps with clicks, and the Data View stays evenly spaced in very large files.

## 0.8.0 (2026-11-12)

AI agents can now work with Statscraft through an MCP server.

- `statscraft mcp` runs the MCP server. See [the MCP server](docs/mcp.md).
- `statscraft connect` adds it to the AI apps on your computer. See [Connect your AI agent](docs/connect.md).
- Each release has a one-click bundle for Claude Desktop, a Claude Code plugin and the `statscraft-analysis` skill.
- Each dataset has its own data window and each output document its own output window.
- REGRESSION, FACTOR, MEANS, RECODE, COUNT and DESCRIPTIVES are faster on large data.

## 0.7.0 (2026-11-12)

Tables show variable and value labels, names or both, as SET TVARS and TNUMBERS choose, and every dialog ends with the same OK, Paste and Cancel buttons.

## 0.6.0 (2026-11-12)

The [MATRIX](docs/commands/matrix.md) language arrives, transformations know 185 functions, and SET BLANKS, EPOCH, FUZZBITS and SCALEMIN take effect.

## 0.5.0 (2026-11-12)

New procedures, input programs and output commands, with a portable Windows build and a macOS app.

- [FACTOR](docs/commands/factor.md), QUICK CLUSTER, [CLUSTER](docs/commands/cluster.md), [DISCRIMINANT](docs/commands/discriminant.md), [LOGISTIC REGRESSION](docs/commands/logistic-regression.md), [UNIANOVA](docs/commands/unianova.md), [GLM](docs/commands/glm.md) and ANOVA.
- [INPUT PROGRAM](docs/commands/input-program.md), FILE TYPE, REPEATING DATA and the other input program commands.
- MATRIX DATA, REPORT and the OUTPUT commands, which export to Excel, HTML, text and CSV.
- Dialogs for these procedures in the window.
- A portable Windows ZIP and a macOS app, both updating from the window.

## 0.4.0 (2026-11-11)

The window asks before closing unsaved work, has one Settings dialog, and gains grid selection and more dialogs.

- Nothing unsaved closes without asking.
- Edit > Settings holds the editor, grid, output and update settings.
- Cells, headers and row numbers can be selected with the mouse, and Delete clears them.
- Dialogs for Linear Regression, Reliability Analysis, the nonparametric tests and Partial Correlations.
- New commands: TITLE, SUBTITLE, PRINT, WRITE, XSAVE and more.

## 0.3.0 (2026-11-11)

More procedures, restructuring and merging, and Excel files in and out.

- [REGRESSION](docs/commands/regression.md), [RELIABILITY](docs/commands/reliability.md), [NPAR TESTS](docs/commands/npar-tests.md), PARTIAL CORR, [MULT RESPONSE](docs/commands/mult-response.md) and [TREE](docs/commands/tree.md).
- VARSTOCASES, CASESTOVARS, FLIP, UPDATE, ADD FILES and MATCH FILES.
- GET DATA reads Excel workbooks; SAVE TRANSLATE writes CSV, tab-delimited text and Excel.
- Dates, times and currencies show in their formats in tables, the Data tab and Excel.
- Command line sessions keep settings, the working folder and random numbers between runs.

## 0.2.0 (2026-11-11)

The Statscraft name and look everywhere, right-click menus in the grids, and more CTABLES formats.

- A new app icon, colours and bundled fonts.
- Right-click menus in the Data and Variables tabs.
- INCLUDE and INSERT read the file at once, so a macro library's macros can be called right after it.
- [CTABLES](docs/commands/ctables.md) has more summary formats and dates in titles.

## 0.1.1 (2026-11-10)

FREQUENCIES and RANK are complete, database connections are encrypted and faster, and the window gains the Variables tab.

- [FREQUENCIES](docs/commands/frequencies.md) and [RANK](docs/commands/rank.md) run with every subcommand.
- Database connections use TLS and store and read large datasets much faster.
- The Variables tab with its editors, and Rank, Automatic Recode and Split File dialogs.
- Syntax editor themes.
- Macros last across the runs of a session.

## 0.1.0 (2026-11-10, early release)

The first version for people who want to try Statscraft and tell us what breaks. Please report problems on the Issues page of the GitHub repository.

- Reads and writes .sav and .zsav files, with labels, missing values, multiple response sets and dates.
- Runs the statistics command syntax: macros, transformations, and procedures from FREQUENCIES to CTABLES.
- Custom tables with nesting, totals, multiple response sets and significance tests.
- A desktop window with data, syntax and output views, and a command line built for scripts and AI agents.
- Stores datasets in SQLite, PostgreSQL, MySQL, MariaDB or Dolt.
