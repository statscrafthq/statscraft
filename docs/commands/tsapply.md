<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# TSAPPLY

Forecasts from time series models that TSMODEL saved, as they are or estimated again.

## Syntax

```text
TSAPPLY [global subcommands of TSMODEL] [/AUXILIARY [REESTIMATE=NO | YES] ...]
        /MODEL FILE=file [DROP=['id' ...] | KEEP=['id' ...]] [OUTFILE=file] [/MODEL ...].
```

Reads the JSON model files TSMODEL writes and applies each model to the series of the same name. With REESTIMATE=NO (the default) the saved estimates and the saved series make the forecasts, which start after the case the model was estimated to, counted from the first case of the file; the fit measures are the saved ones. REESTIMATE=YES keeps the form of each model and estimates it again over the USE range of the active dataset.

KEEP and DROP pick models by identifier; OUTFILE writes the models applied. Output and SAVE are as in TSMODEL; residual autocorrelations, fitted values and residuals need REESTIMATE=YES. Not supported under SPLIT FILE.

## Example

```text
TSAPPLY /MODELDETAILS PRINT=FORECASTS /MODEL FILE='bookings_models.json'.
```

`statscraft help TSAPPLY` prints the same text in the terminal. [All commands](README.md)
