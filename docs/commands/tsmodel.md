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
        [/AUTOOUTLIER DETECT=OFF] [/MODEL ...].
```

Every series of DEPENDENT gets its own model, named PREFIX\_1, PREFIX\_2 and so on. Global subcommands come first; words are written in full and a list goes in square brackets.

The season length is SEASONLENGTH, else the period of the DATE variables. The model is estimated over the USE range less missing values at its ends; forecasts run from the case after it to the end of PREDICT. Smoothing weights minimize the sum of squared one-step errors, from start values found by smoothing backwards. ARIMA models are fitted by exact maximum likelihood; a single lag value is an order (AR=2: lags 1 and 2), a list of two or more names the lags. Predictors enter ARIMA models as regressors at lag 0 (\[E\] after a name is accepted). The expert modeler fits every fitting smoothing method and a search of ARIMA orders and keeps the model with the smallest normalized BIC. Output: the model description, a summary of the fit measures across models, a table of the fit measures and the Ljung-Box Q(18) by model, and on request parameters, residual autocorrelations, forecasts and charts. OUTFILE writes the models to a JSON file for TSAPPLY. SAVE adds root\_series\_model variables (Predicted, LCL, UCL, NResidual). Not supported yet: TRANSFERFUNCTION, OUTLIER, AUTOOUTLIER DETECT=ON, OUTPMML and the MODELSUMMARY charts.

## Example

```text
TSMODEL /MODELDETAILS PRINT=[PARAMETERS FORECASTS] /MODEL DEPENDENT=bookings /EXSMOOTH TYPE=HOLT.
```

`statscraft help TSMODEL` prints the same text in the terminal. [All commands](README.md)
