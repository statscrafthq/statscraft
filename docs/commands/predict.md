<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# PREDICT

Set the forecast period of EXSMOOTH and ARIMA.

## Syntax

```text
PREDICT [start] [THRU [end]].
```

Start and end are case numbers, or dates written with DATE keywords and values; END means the last case. Without a start the period begins right after the USE range (or the file); a start later than that is moved back to it. Without an end it runs to the last case of the file, or covers just its start when that lies past the file. EXSMOOTH and ARIMA estimate on the USE range, give one-step fitted values up to the case before the start and forecasts from there on, adding cases when the period runs past the end of the file (at most TSET MXPREDICT, default 60). Added cases are missing on every variable except the new ones and the DATE variables. Residuals are missing in the forecast period. DATE turns PREDICT off; it stays in force until the next PREDICT.

## Example

```text
PREDICT THRU YEAR 2026 MONTH 12.
```

`statscraft help PREDICT` prints the same text in the terminal. [All commands](README.md)
