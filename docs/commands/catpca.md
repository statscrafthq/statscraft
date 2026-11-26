<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# CATPCA

Categorical principal components: quantify each variable by optimal scaling (spline, ordinal, nominal, multiple nominal or numeric) and reduce them to a few components.

## Syntax

```text
CATPCA [/VARIABLES=] varlist
   /ANALYSIS=varlist [([WEIGHT=n] [LEVEL=SPORD | SPNOM | ORDI | NOMI | MNOM | NUME]
                      [DEGREE=2 | n] [INKNOT=2 | n])] [varlist [(...)] ...]
   [/DISCRETIZATION=[varlist [([GROUPING] [NCAT=7 | n] [DISTR=NORMAL | UNIFORM] | [EQINTV=width]
                               | RANKING | MULTIPLYING)] ...]]
   [/MISSING=[varlist | ALL [([PASSIVE] [MODEIMPU | EXTRACAT | RANDIMPU]
                               | ACTIVE [MODEIMPU | EXTRACAT | RANDIMPU] | LISTWISE)] ...]]
   [/SUPPLEMENTARY=[OBJECT(case numbers)] [VARIABLE(varlist)]]
   [/CONFIGURATION=[INITIAL | FIXED]('file' | dataset)] [/DIMENSION=2 | n]
   [/NORMALIZATION=VPRINCIPAL | OPRINCIPAL | SYMMETRICAL | INDEPENDENT | q]
   [/MAXITER=100 | n] [/CRITITER=.00001 | value]
   [/ROTATION=[NOROTATE | VARIMAX | EQUAMAX | QUARTIMAX | PROMAX[(4 | kappa)] | OBLIMIN[(0 | delta)]]
              [KAISER | NOKAISER]]
   [/RESAMPLE=NONE | BOOTSTRAP[(samples=1000, percent=95, BALANCED | UNBALANCED, PROCRU | REFLEC)]]
   [/PRINT=[DESCRIP[(varlist)]] [LOADING[(SORT | NOSORT)]] [CORR] [VAF] [OCORR] [QUANT[(varlist)]]
          [HISTORY] [OBJECT[([(labelvar)] varlist)]] [NONE]]
   [/PLOT=[OBJECT[(varlist)][(len)]] [LOADING[(varlist [(CENTR[(varlist)])])][(len)]]
         [LDELLAREA[((GT|LT) (AREA|STDEV) value)]] [OBELLAREA[(...)]] [CTELLAREA[(...)]] [NELLPNT(40)]
         [CATEGORY(varlist)[(len)]] [JOINTCAT[(varlist)][(len)]] [TRANS(var[(n)] ...)[(len)]]
         [RESID(var[(n)] ...)[(len)]] [BIPLOT[(LOADING | CENTR[(varlist)])][(varlist)][(len)]]
         [TRIPLOT[(varlist[(varlist)])][(len)]] [PROJCENTR(var, varlist)[(len)]] [VAF]
         [NDIM(first, last)] [NONE]]
   [/SAVE=[TRDATA[(root[(n)])]] [OBJECT[(root[(n)])]] [APPROX[(root)]] [LDELLAREA] [CTELLAREA]
        [OBELLAREA | ELLAREAOBJ]]
   [/OUTFILE=[DISCRDATA('file' | dataset)] [TRDATA('file' | dataset)] [OBJECT('file' | dataset)]
            [APPROX('file' | dataset)] [ELLCOORD('file' | dataset)]].
```

The command gives each category of each variable a value (its quantification) so that a few principal components explain the quantified variables as well as they can. ANALYSIS is required and names the active variables with their options. Variables are coded as for MULTIPLE CORRESPONDENCE: whole codes from 1, lower codes and missing values missing, strings coded in sorted order, fractions cut into 7 groups unless DISCRETIZATION says otherwise. DISCRETIZATION, MISSING, SUPPLEMENTARY, CONFIGURATION, DIMENSION, MAXITER and CRITITER work as there. SPLIT FILE does not apply.

LEVEL says what the values must respect. NUME keeps equal steps between codes (all numeric gives plain principal components). ORDI keeps the order of the codes and NOMI only the grouping. SPORD (the default) and SPNOM fit a smooth spline of DEGREE pieces joined at INKNOT interior knots, placed at quantiles of the codes; SPORD keeps it rising. MNOM gives a separate value per component, the centroid of the category's cases. WEIGHT gives a variable more say.

The fit starts from the numeric solution (iteration 0) and iterates until the variance accounted for gains less than CRITITER, at most MAXITER times. An extra category from ACTIVE EXTRACAT is free of the level's restriction after iteration 0. The components come out on principal axes, each signed so that its positive loadings have the larger mean square. Supplementary variables are fitted to the final scores.

NORMALIZATION shares the eigenvalues between scores and loadings, so that scores times loadings stay the same. VPRINCIPAL (the default, q = -1) gives standard scores and loadings that are correlations. OPRINCIPAL (q = 1) puts the eigenvalues in the scores, SYMMETRICAL (q = 0) splits them evenly, and a number q from -1 to 1 lies between. INDEPENDENT scales the scores only. ROTATION turns the loadings of the variables that have them, and the scores and centroids with them, with Kaiser normalization unless NOKAISER.

RESAMPLE=BOOTSTRAP shows how stable the solution is: it analyses that many samples of the cases drawn with replacement (BALANCED gives every case exactly as many appearances as there are samples; UNBALANCED draws each case freely, from the generator SET SEED seeds), turns each sample's component loadings to those of the data (PROCRU by the closest rotation, REFLEC by reflecting dimensions) and gives, for the eigenvalues, the loadings and the category values of the variables that have a single value, the value of the data, the mean and standard deviation of the samples and the percentile interval at the percent: tables Bootstrap Eigenvalues, Bootstrap Component Loadings and Bootstrap Quantifications. It cannot be combined with ROTATION or a FIXED configuration, and at least one active variable must not be MNOM. Samples that cannot be analysed are left out and counted.

The bootstrap also draws charts (unless PLOT=NONE): the eigenvalues of the data with their interval, and for a solution of 2 dimensions the confidence ellipses of the eigenvalues, of the component loadings (with LOADING), of the object points (with OBJECT) and of the category points of the variables of CATEGORY (and of all of them together with JOINTCAT); an ellipse is the region where (x - c)' S^-1 (x - c) is at most the chi-square value of 2 degrees of freedom at the percent, around the data's point c, S the covariance of the aligned points of the samples. LDELLAREA, OBELLAREA and CTELLAREA(GT\|LT AREA\|STDEV value) keep the ellipses whose area is above or below the value, or the mean area plus that many standard deviations (defaults: \> AREA 0, \> STDEV 2, \> AREA 2); NELLPNT(n) sets the points of a contour (40). Each of those plots (loadings, objects, categories) also comes as a joint plot: the data's labelled points together with the points the samples gave for them (at most about 200000 object points are kept, every k-th case). TRANS plots of variables with a single value show the interval of each category's value as Lower and Upper.

With a bootstrap of 2 dimensions, SAVE LDELLAREA, CTELLAREA and OBELLAREA (also ELLAREAOBJ) add the area of every confidence ellipse as a variable named LDELL\_n, CTELL\_n or OBELL\_n: for the objects one area per case, for the loadings the area of the k-th analysis variable in case k, for the categories the area of the k-th category (variable by variable) in case k, the other cases blank. OUTFILE ELLCOORD writes the coordinates of the ellipses drawn: Plot, Entity, Point (0 is the point of the data, then the closed contour), Dimension1 and Dimension2. Without the bootstrap the keywords warn.

Always shown: Case Processing Summary, Iteration History (iteration 0 and the last, or all with HISTORY) and Model Summary (Cronbach's alpha, eigenvalue and percent of variance per dimension and in total). PRINT adds: DESCRIP frequencies, missing count and mode; QUANT each variable's quantifications, centroids and vector coordinates; VAF each variable's variance accounted for, by centroids and vectors; OCORR and CORR the correlations of the original and of the quantified variables with eigenvalues (one matrix per dimension when a variable is MNOM); OBJECT the component scores; LOADING the component loadings (SORT groups them by largest loading), with the rotated loadings, or pattern, structure and component correlations after an oblique rotation. Without PRINT, or with PRINT alone: DESCRIP LOADING CORR.

Charts (OBJECT and LOADING by default): OBJECT the component scores, colored by the categories of each named variable; LOADING the loadings, with MNOM centroids after CENTR; CATEGORY the centroids and vectors of each variable; JOINTCAT all centroids together; TRANS the quantification by code; RESID the quantified value against its approximation; BIPLOT scores with loadings or centroids and TRIPLOT with both, scaled to the loadings (not with INDEPENDENT); PROJCENTR the centroids of one variable projected on the loading directions of others (also a table); VAF bars of each variable's variance accounted for. Two dimensions give one scatter, more give a scatterplot matrix, and NDIM(a, b) plots a against each of a+1 to b. A label length of 0 shows values and names instead of labels.

SAVE adds the quantified variables rootK\_S (root TRA, K the place on ANALYSIS, S the first free set number; an MNOM variable gives rootK\_D\_S per dimension D, with the root cut to 3 characters), the scores rootD\_S (OBSCO) and the approximations rootK\_S (APP, the first score for an MNOM variable). Excluded cases and passive missing values are system-missing. OUTFILE writes the codes (DISCRDATA), TRAK or TRAK\_D, OBSCOD and APPK; a file with no keyword means TRDATA.

## Example

```text
CATPCA pace crowd parking shade /ANALYSIS=pace crowd(LEVEL=ORDI) parking(LEVEL=NOMI) shade(LEVEL=SPNOM INKNOT=1) /MISSING=shade(ACTIVE EXTRACAT) /ROTATION=VARIMAX /PRINT=LOADING(SORT) VAF QUANT(parking) /PLOT=BIPLOT TRANS(shade) /SAVE=OBJECT.
```

`statscraft help CATPCA` prints the same text in the terminal. [All commands](README.md)
