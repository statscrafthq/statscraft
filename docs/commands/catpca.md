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

The command gives each category of each variable a value (its quantification) so that a few principal components explain the quantified variables as well as they can. ANALYSIS is required and names the active variables with their options. Variables are coded as for MULTIPLE CORRESPONDENCE: whole codes from 1, lower codes and missing values missing, strings coded in sorted order, fractions cut into 7 groups unless DISCRETIZATION says otherwise. DISCRETIZATION, MISSING, SUPPLEMENTARY, CONFIGURATION, DIMENSION, MAXITER and CRITITER work as there. SPLIT FILE does not apply.

LEVEL says what the values must respect. NUME keeps equal steps between codes (all numeric gives plain principal components). ORDI keeps the order of the codes and NOMI only the grouping. SPORD (the default) and SPNOM fit a smooth spline of DEGREE pieces joined at INKNOT interior knots, placed at quantiles of the codes; SPORD keeps it rising. MNOM gives a separate value per component, the centroid of the category's cases. WEIGHT gives a variable more say.

The fit starts from the numeric solution (iteration 0) and iterates until the variance accounted for gains less than CRITITER, at most MAXITER times. An extra category from ACTIVE EXTRACAT is free of the level's restriction after iteration 0. The components come out on principal axes, each signed so that its positive loadings have the larger mean square. Supplementary variables are fitted to the final scores.

NORMALIZATION shares the eigenvalues between scores and loadings, so that scores times loadings stay the same. VPRINCIPAL (the default, q = -1) gives standard scores and loadings that are correlations. OPRINCIPAL (q = 1) puts the eigenvalues in the scores, SYMMETRICAL (q = 0) splits them evenly, and a number q from -1 to 1 lies between. INDEPENDENT scales the scores only. ROTATION turns the loadings of the variables that have them, and the scores and centroids with them, with Kaiser normalization unless NOKAISER.

Always shown: Case Processing Summary, Iteration History (iteration 0 and the last, or all with HISTORY) and Model Summary (Cronbach's alpha, eigenvalue and percent of variance per dimension and in total). PRINT adds: DESCRIP frequencies, missing count and mode; QUANT each variable's quantifications, centroids and vector coordinates; VAF each variable's variance accounted for, by centroids and vectors; OCORR and CORR the correlations of the original and of the quantified variables with eigenvalues (one matrix per dimension when a variable is MNOM); OBJECT the component scores; LOADING the component loadings (SORT groups them by largest loading), with the rotated loadings, or pattern, structure and component correlations after an oblique rotation. Without PRINT, or with PRINT alone: DESCRIP LOADING CORR.

Charts (OBJECT and LOADING by default): OBJECT the component scores, colored by the categories of each named variable; LOADING the loadings, with MNOM centroids after CENTR; CATEGORY the centroids and vectors of each variable; JOINTCAT all centroids together; TRANS the quantification by code; RESID the quantified value against its approximation; BIPLOT scores with loadings or centroids and TRIPLOT with both, scaled to the loadings (not with INDEPENDENT); PROJCENTR the centroids of one variable projected on the loading directions of others (also a table); VAF bars of each variable's variance accounted for. Two dimensions give one scatter, more give a scatterplot matrix, and NDIM(a, b) plots a against each of a+1 to b. A label length of 0 shows values and names instead of labels.

SAVE adds the quantified variables rootK\_S (root TRA, K the place on ANALYSIS, S the first free set number; an MNOM variable gives rootK\_D\_S per dimension D, with the root cut to 3 characters), the scores rootD\_S (OBSCO) and the approximations rootK\_S (APP, the first score for an MNOM variable). Excluded cases and passive missing values are system-missing. OUTFILE writes the codes (DISCRDATA), TRAK or TRAK\_D, OBSCOD and APPK; a file with no keyword means TRDATA.

## Example

```text
CATPCA pace crowd parking shade /ANALYSIS=pace crowd(LEVEL=ORDI) parking(LEVEL=NOMI) shade(LEVEL=SPNOM INKNOT=1) /MISSING=shade(ACTIVE EXTRACAT) /ROTATION=VARIMAX /PRINT=LOADING(SORT) VAF QUANT(parking) /PLOT=BIPLOT TRANS(shade) /SAVE=OBJECT.
```

`statscraft help CATPCA` prints the same text in the terminal. [All commands](README.md)
