<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# GENLINMIXED

Generalized linear mixed model: a target with a normal, binomial, multinomial, gamma, inverse Gaussian, Poisson or negative binomial distribution and a link, fixed effects, random effects of subjects and repeated measures.

## Syntax

```text
GENLINMIXED [/DATA_STRUCTURE SUBJECTS=field[*field...] [REPEATED_MEASURES=field[*field...]]
          [COVARIANCE_TYPE=DIAGONAL | type]]
   /FIELDS TARGET=field [TRIALS=NONE | VALUE(n) | FIELD(field)] [ANALYSIS_WEIGHT=field]
          [OFFSET=NONE | VALUE(x) | FIELD(field)]
   [/TARGET_OPTIONS [REFERENCE=value]
          [DISTRIBUTION=NORMAL | BINOMIAL | MULTINOMIAL | GAMMA | INVERSE_GAUSSIAN | POISSON |
          NEGATIVE_BINOMIAL] [LINK=IDENTITY | CAUCHIT | CLOGLOG | LOG | LOGC | LOGIT | NLOGLOG |
          POWER | PROBIT] [LINK_PARAMETER=x]]
   [/FIXED [EFFECTS=effects] [USE_INTERCEPT=TRUE | FALSE]]
   [/RANDOM [EFFECTS=effects] [USE_INTERCEPT=FALSE | TRUE] [SUBJECTS=field[*field...]]
           [COVARIANCE_TYPE=type] [SOLUTION=FALSE | TRUE]] ...
   [/BUILD_OPTIONS [TARGET_CATEGORY_ORDER=ASCENDING | DESCENDING | DATA] [MAX_ITERATIONS=100]
          [CONFIDENCE_LEVEL=95] [DF_METHOD=RESIDUAL | SATTERTHWAITE | KENWARD_ROGER]
          [COVB=MODEL | ROBUST] [PCONVERGE=1E-6[(ABSOLUTE | RELATIVE)]] [LCONVERGE=0[(...)]]
          [HCONVERGE=0[(...)]] [SCORING=0] [SINGULAR=1E-12]]
   [/EMMEANS [TABLES=factor[*factor...]] [COMPARE=factor]
          [CONTRAST=NONE | PAIRWISE | DEVIATION | SIMPLE]] ...
   [/EMMEANS_OPTIONS [CONTROL=covariate(x) ...] [SCALE=ORIGINAL | TRANSFORMED]
          [PADJUST=LSD | SEQBONFERRONI | SEQSIDAK]]
   [/OUTFILE MODEL='file']
   [/SAVE [PREDICTED_VALUES[(name)]] [PREDICTED_PROBABILITY[(root)]] [MAX_CATEGORIES(25)]
          [CONFIDENCE[(name)]] [CONFIDENCE_METHOD(PREDICTEDPROBABILITY | PROBABILITYIMPROVEMENT)]
          [CONFIDENCE_INTERVALS[(root)]] [PEARSON_RESIDUALS[(name)]]]
```

effects: a field, a\*b or a BY b, a(b); a field of scale level is a covariate, any other a factor.

type: VARIANCE\_COMPONENTS (the default on RANDOM: one variance per effect; on DATA\_STRUCTURE the same as DIAGONAL), IDENTITY, DIAGONAL, UNSTRUCTURED, COMPOUND\_SYMMETRY, AR1, ARH1, ARMA11, CSH or TOEPLITZ, as the types of MIXED.

Without DISTRIBUTION a scale target is NORMAL, and one with TRIALS or a categorical one of two values BINOMIAL; the link defaults to IDENTITY, LOGIT, POWER(-1) for GAMMA, POWER(-2) for INVERSE\_GAUSSIAN and LOG for POISSON and NEGATIVE\_BINOMIAL. BINOMIAL takes either events with TRIALS or a target of two values; then the last value in TARGET\_CATEGORY\_ORDER (or REFERENCE) is the reference and the model is for the other one.

Cases whose target does not fit the distribution, or that miss a field, the offset or a weight above 0, are left out. The fit linearizes the model around the current means and fits the linear mixed model of the working target with the weights that follow from the variance function, by restricted likelihood as MIXED does, then updates the means with the predicted random effects, until no covariance parameter changes by more than PCONVERGE relative to its size. REPEATED\_MEASURES orders the cases of each subject by these fields and lets their residuals be correlated as COVARIANCE\_TYPE says; a model with neither is a generalized linear model of the fixed effects. The scale is 1 for BINOMIAL, POISSON and NEGATIVE\_BINOMIAL without repeated measures and estimated otherwise; NORMAL with IDENTITY is MIXED itself. NEGATIVE\_BINOMIAL takes its k from the model without random effects and keeps it. Each RANDOM must name subjects from DATA\_STRUCTURE. MAX\_ITERATIONS bounds both loops. COVB=ROBUST tests with the sandwich covariance over the subjects.

Tables: Model Summary, Model Dimension, Information Criteria (of the working model), Type III Tests of Fixed Effects, Estimates of Fixed Effects, Estimates of Covariance Parameters with standard errors and Wald intervals, and the predicted random effects of each RANDOM with SOLUTION=TRUE. Degrees of freedom are the cases less the rank of the fixed design unless DF\_METHOD says otherwise. EMMEANS gives the means of a fixed effect of factors (the overall mean without TABLES) with covariates at their mean or CONTROL value, on the scale of the target or of the link, and with COMPARE the contrasts among the levels of one factor, adjusted by PADJUST, and their joint F test. SAVE adds the predicted mean (the events for events and trials, the category for a target of two values), its interval and the Pearson residual; probabilities and confidence need a categorical target. OUTFILE writes the fixed effects as a JSON model file.

A categorical target of three or more values is MULTINOMIAL. An ordinal one gets a threshold per category but the last in place of the intercept and a cumulative link (LOGIT, PROBIT, CLOGLOG, NLOGLOG or CAUCHIT); a nominal one is compared with its REFERENCE category (the last by default) by the logit, with its own parameters and random effects for each other category. Neither takes REPEATED\_MEASURES; EMMEANS is ignored with a warning. SAVE gives the predicted category, the probabilities (cumulative for an ordinal target), the confidence and the intervals of the probabilities. Not supported yet: SAVE PEARSON\_RESIDUALS of a multinomial target, the spatial and Kronecker covariance types, KRONECKER\_MEASURES, SPCOORDS, GROUPING and INPUTS\_CATEGORY\_ORDER other than ASCENDING.

## Example

```text
GENLINMIXED /DATA_STRUCTURE SUBJECTS=clinic /FIELDS TARGET=cured /TARGET_OPTIONS DISTRIBUTION=BINOMIAL LINK=LOGIT /FIXED EFFECTS=dose /RANDOM USE_INTERCEPT=TRUE SUBJECTS=clinic.
```

`statscraft help GENLINMIXED` prints the same text in the terminal. [All commands](README.md)
