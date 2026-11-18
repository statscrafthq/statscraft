<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# TSMODEL

Exponential smoothing and ARIMA models of series, picked by hand or by the expert modeler, with forecasts.

## Syntax

```text
TSMODEL [/MODELSUMMARY [PRINT=[MODELFIT RESIDACF RESIDPACF | NONE]] [PLOT=[...]]]
        [/MODELSTATISTICS [DISPLAY=YES | NO] [MODELFIT=[SRSQUARE RSQUARE RMSE MAPE MAE MAXAPE MAXAE NORMBIC]]]
        [/MODELDETAILS [PRINT=[PARAMETERS RESIDACF RESIDPACF FORECASTS]] [PLOT=[RESIDACF RESIDPACF]]]
        [/SERIESPLOT OBSERVED FORECAST FIT FORECASTCI FITCI]
        [/OUTPUTFILTER [DISPLAY=ALLMODELS | [BESTFIT(N=n | PCT=p) WORSTFIT(N=n | PCT=p)]] [MODELFIT=measure]]
        [/SAVE PREDICTED[(root)] LCL[(root)] UCL[(root)] NRESIDUAL[(root)]]
        [/AUXILIARY [CILEVEL=95] [MAXACFLAGS=24] [SEASONLENGTH=n]] [/MISSING USERMISSING=EXCLUDE | INCLUDE]
        /MODEL DEPENDENT=series [INDEPENDENT=predictors] [OUTFILE=file] [PREFIX='Model']
        [/EXPERTMODELER [TYPE=[ARIMA EXSMOOTH]] [TRYSEASONAL=YES | NO]
         | /EXSMOOTH TYPE=SIMPLE | SIMPLESEASONAL | HOLT | BROWN | DAMPEDTREND | WINTERSADDITIVE
                     | WINTERSMULTIPLICATIVE [TRANSFORM=NONE | SQRT | LN]
         | /ARIMA AR=lags MA=lags [ARSEASONAL=lags] [MASEASONAL=lags] [DIFF=d] [DIFFSEASONAL=D]
                  [CONSTANT=YES | NO] [TRANSFORM=NONE | SQRT | LN]]
        [/TRANSFERFUNCTION [VARIABLES=ALL | predictors] [NUM=lags] [NUMSEASONAL=lags]
                           [DENOM=lags] [DENOMSEASONAL=lags] [DIFF=d] [DIFFSEASONAL=D]
                           [DELAY=b] [TRANSFORM=NONE | SQRT | LN]] ...
        [/AUTOOUTLIER DETECT=OFF | ON [TYPE=[ADDITIVE LEVELSHIFT INNOVATIONAL TRANSIENT
                     SEASONALADDITIVE LOCALTREND ADDITIVEPATCH]]]
        [/OUTLIER LOCATION=[case | date] TYPE=type] ... [/MODEL ...].
```

Every series of DEPENDENT gets its own model, named PREFIX\_1, PREFIX\_2 and so on. Global subcommands come first; words are written in full and a list goes in square brackets.

The season length is SEASONLENGTH, else the period of the DATE variables. The model is estimated over the USE range less missing values at its ends; forecasts run from the case after it to the end of PREDICT. Smoothing weights minimize the sum of squared one-step errors, from start values found by smoothing backwards. ARIMA models are fitted by exact maximum likelihood; a single lag value is an order (AR=2: lags 1 and 2), a list of two or more names the lags. In a custom ARIMA model each predictor passes through a ratio of lag polynomials: TRANSFERFUNCTION sets the numerator lags (lag 0 is always in) and denominator lags, plain and seasonal, and how the predictor is transformed, differenced and shifted by DELAY first; the last one naming a predictor counts. Without it a predictor enters at lag 0, and the expert modeler enters every predictor so (\[E\] after a name is accepted). OUTLIER puts an outlier of the type given at a case number or a DATE point; AUTOOUTLIER DETECT=ON searches the ARIMA model (custom or expert) for outliers of the types listed (additive and level shift by default), keeping those that stay significant when estimated with the model. The expert modeler fits every fitting smoothing method and a search of ARIMA orders and keeps the model with the smallest normalized BIC. Output: the model description, a summary of the fit measures across models, a table of the fit measures, the Ljung-Box Q(18) and the number of outliers by model, and on request parameters (with an Outliers table), residual autocorrelations, forecasts and charts. OUTFILE writes the models to a JSON file for TSAPPLY. SAVE adds root\_series\_model variables (Predicted, LCL, UCL, NResidual). Not supported yet: OUTPMML, the MODELSUMMARY charts and transfer functions chosen by the expert modeler.

## Example

```text
TSMODEL /MODELDETAILS PRINT=[PARAMETERS FORECASTS] /MODEL DEPENDENT=bookings /EXSMOOTH TYPE=HOLT.
```

`statscraft help TSMODEL` prints the same text in the terminal. [All commands](README.md)
