<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# GENLINMIXED

Generalized linear mixed model: a target with a normal, binomial, gamma, inverse Gaussian or Poisson distribution and a link, fixed effects and random effects of subjects.

## Syntax

```text
GENLINMIXED [/DATA_STRUCTURE SUBJECTS=field[*field...]]
   /FIELDS TARGET=field [TRIALS=NONE | VALUE(n) | FIELD(field)] [ANALYSIS_WEIGHT=field]
          [OFFSET=NONE | VALUE(x) | FIELD(field)]
   [/TARGET_OPTIONS [REFERENCE=value] [DISTRIBUTION=NORMAL | BINOMIAL | GAMMA | INVERSE_GAUSSIAN | POISSON]
          [LINK=IDENTITY | CLOGLOG | LOG | LOGC | LOGIT | NLOGLOG | POWER | PROBIT] [LINK_PARAMETER=x]]
   [/FIXED [EFFECTS=effects] [USE_INTERCEPT=TRUE | FALSE]]
   /RANDOM [EFFECTS=effects] [USE_INTERCEPT=FALSE | TRUE] [SUBJECTS=field[*field...]]
           [COVARIANCE_TYPE=type] [SOLUTION=FALSE | TRUE] ...
   [/BUILD_OPTIONS [TARGET_CATEGORY_ORDER=ASCENDING | DESCENDING | DATA] [MAX_ITERATIONS=100]
          [CONFIDENCE_LEVEL=95] [DF_METHOD=RESIDUAL | SATTERTHWAITE | KENWARD_ROGER]
          [PCONVERGE=1E-6[(ABSOLUTE | RELATIVE)]] [LCONVERGE=0[(...)]] [HCONVERGE=0[(...)]]
          [SCORING=0] [SINGULAR=1E-12]]
```

effects: a field, a\*b or a BY b, a(b); a field of scale level is a covariate, any other a factor.

type: VARIANCE\_COMPONENTS (the default: one variance per effect), IDENTITY, DIAGONAL, UNSTRUCTURED, COMPOUND\_SYMMETRY, AR1, ARH1, ARMA11, CSH or TOEPLITZ, as the types of MIXED.

Without DISTRIBUTION a scale target is NORMAL, and one with TRIALS or a categorical one of two values BINOMIAL; the link defaults to IDENTITY, LOGIT, POWER(-1) for GAMMA, POWER(-2) for INVERSE\_GAUSSIAN and LOG for POISSON. BINOMIAL takes either events with TRIALS or a target of two values; then the last value in TARGET\_CATEGORY\_ORDER (or REFERENCE) is the reference and the model is for the other one.

Cases whose target does not fit the distribution, or that miss a field, the offset or a weight above 0, are left out. The fit linearizes the model around the current means and fits the linear mixed model of the working target with the weights that follow from the variance function, by restricted likelihood as MIXED does, then updates the means with the predicted random effects, until no covariance parameter changes by more than PCONVERGE relative to its size. The scale is 1 for BINOMIAL and POISSON and estimated as the residual variance otherwise; NORMAL with IDENTITY is MIXED itself. Each RANDOM must name subjects from DATA\_STRUCTURE. MAX\_ITERATIONS bounds both loops. Tables: Model Summary, Model Dimension, Information Criteria (of the working model), Type III Tests of Fixed Effects, Estimates of Fixed Effects, Estimates of Covariance Parameters with standard errors and Wald intervals, and the predicted random effects of each RANDOM with SOLUTION=TRUE. Degrees of freedom are the cases less the rank of the fixed design unless DF\_METHOD says otherwise. EMMEANS\_OPTIONS is checked and has no effect. Not supported yet: MULTINOMIAL and NEGATIVE\_BINOMIAL, a model without RANDOM, REPEATED\_MEASURES and the other repeated measures keywords, GROUPING, COVB=ROBUST, INPUTS\_CATEGORY\_ORDER other than ASCENDING, EMMEANS, SAVE and OUTFILE.

## Example

```text
GENLINMIXED /DATA_STRUCTURE SUBJECTS=clinic /FIELDS TARGET=cured /TARGET_OPTIONS DISTRIBUTION=BINOMIAL LINK=LOGIT /FIXED EFFECTS=dose /RANDOM USE_INTERCEPT=TRUE SUBJECTS=clinic.
```

`statscraft help GENLINMIXED` prints the same text in the terminal. [All commands](README.md)
