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
   [/RESAMPLE=NONE | BOOTSTRAP[(samples, percent, BALANCED | UNBALANCED, PROCRU | REFLEC)]]
   [/PRINT=[DESCRIP[(varlist)]] [LOADING[(SORT | NOSORT)]] [CORR] [VAF] [OCORR] [QUANT[(varlist)]]
          [HISTORY] [OBJECT[([(labelvar)] varlist)]] [NONE]]
   [/PLOT=[OBJECT[(varlist)][(len)]] [LOADING[(varlist [(CENTR[(varlist)])])][(len)]]
         [CATEGORY(varlist)[(len)]] [JOINTCAT[(varlist)][(len)]] [TRANS(var[(n)] ...)[(len)]]
         [RESID(var[(n)] ...)[(len)]] [BIPLOT[(LOADING | CENTR[(varlist)])][(varlist)][(len)]]
         [TRIPLOT[(varlist[(varlist)])][(len)]] [PROJCENTR(var, varlist)[(len)]] [VAF]
         [NDIM(first, last)] [NONE]]
   [/SAVE=[TRDATA[(root[(n)])]] [OBJECT[(root[(n)])]] [APPROX[(root)]]]
   [/OUTFILE=[DISCRDATA('file' | dataset)] [TRDATA('file' | dataset)] [OBJECT('file' | dataset)]
            [APPROX('file' | dataset)]].
```

Variables are coded as for MULTIPLE CORRESPONDENCE: whole codes from 1, lower codes and missing values missing, strings coded in sorted order, fractions cut into 7 groups unless DISCRETIZATION says otherwise; DISCRETIZATION, MISSING, SUPPLEMENTARY, CONFIGURATION, DIMENSION, MAXITER and CRITITER work as there. Each variable gets a value for each category, chosen so that the components explain the quantified variables as well as they can. LEVEL says what the values must respect: NUME equal steps between codes (plain principal components when every variable is numeric), ORDI the order of the codes, NOMI nothing but the grouping, SPORD (the default) and SPNOM a smooth curve through the codes, a spline of DEGREE pieces joined at INKNOT interior knots placed at quantiles of the cases' codes (SPORD keeps it rising), and MNOM a separate value per component, the centroid of the category's cases. WEIGHT gives a variable more say. The fit starts from the exact numeric solution (iteration 0), then alternates quantifications and component scores until the variance accounted for gains less than CRITITER. An extra category from ACTIVE EXTRACAT is left out of the level's restriction after iteration 0. The components are then turned to principal axes, and a component is reflected when its negative loadings have the larger mean square. Supplementary variables are fitted to the final scores. NORMALIZATION q gives the scores the power (1 + q)/2 of the root of each eigenvalue and the loadings and multiple nominal centroids the power (1 - q)/2 less one, so that scores times loadings stay the same: VPRINCIPAL -1 (standard scores, loadings are correlations), OPRINCIPAL 1, SYMMETRICAL 0; INDEPENDENT scales the scores only. ROTATION turns the loadings of the variables that have loadings, with Kaiser normalization unless NOKAISER, and the scores and centroids with them. SPLIT FILE does not apply.

Tables: Case Processing Summary, Iteration History (iteration 0 and the last, or all with HISTORY: variance accounted for, its increase, and the loss split into the centroid part and the part from holding single variables to a line) and Model Summary (Cronbach's alpha, eigenvalue and percent of variance per dimension and in total) always; DESCRIP frequencies, missing count and mode; QUANT per variable the quantification, centroid and vector coordinates; VAF the variance each variable accounts for, by centroids and by vectors; OCORR and CORR the correlations of the original and of the quantified variables (one matrix per dimension when a variable is MNOM), with eigenvalues; OBJECT the component scores; LOADING the component loadings (SORT groups them by their largest loading), with the rotated loadings and transformation matrix, or pattern, structure and component correlations after an oblique rotation. Without PRINT, or with PRINT alone: DESCRIP LOADING CORR.

Charts (OBJECT and LOADING by default): component scores, colored by the categories of each named variable; loadings, with MNOM centroids after CENTR; category centroids and vectors of each variable; all centroids together (JOINTCAT); quantification by code (TRANS); quantified value against its approximation (RESID); scores with loadings or centroids (BIPLOT) or with both (TRIPLOT), scores and centroids shrunk to the loadings, neither with INDEPENDENT; the centroids of one variable projected on the loading direction of others (PROJCENTR, also as a table); bars of each variable's variance accounted for per dimension and in total (VAF). Two dimensions give one scatter, more a scatterplot matrix, NDIM(a, b) one scatter of a against each of a+1 to b. A label length of 0 shows values and names instead of labels.

SAVE adds the quantified variables rootK\_S (TRA, K the place on ANALYSIS, S the first free set number; an MNOM variable gives rootK\_D\_S per dimension D with the root cut to 3 characters), the scores rootD\_S (OBSCO) and the approximations rootK\_S (APP: scores times loadings, the first score for an MNOM variable). Excluded cases and passive missing values are system-missing. OUTFILE writes the codes (DISCRDATA), TRAK or TRAK\_D, OBSCOD and APPK; a file with no keyword means TRDATA.

## Example

```text
CATPCA pace crowd parking shade /ANALYSIS=pace crowd(LEVEL=ORDI) parking(LEVEL=NOMI) shade(LEVEL=SPNOM INKNOT=1) /MISSING=shade(ACTIVE EXTRACAT) /ROTATION=VARIMAX /PRINT=LOADING(SORT) VAF QUANT(parking) /PLOT=BIPLOT TRANS(shade) /SAVE=OBJECT.
```

`statscraft help CATPCA` prints the same text in the terminal. [All commands](README.md)
