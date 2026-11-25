<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# EXSMOOTH

Exponential smoothing with or without trend and season, with fitted values and forecasts.

## Syntax

```text
EXSMOOTH [VARIABLES=] series [/MODEL=NN | NA | NM | LN | LA | LM | EN | EA | EM | DN | DA | DM]
         [/PERIOD=n] [/SEASFACT=(factors) | variable]
         [/ALPHA=value | GRID[(start end step)]] [/GAMMA=...] [/DELTA=...] [/PHI=...]
         [/INITIAL=CALCULATE | (start trend)] [/APPLY].
```

The first letter of the model is the trend (N none, L linear, E exponential, D damped), the second the season (N none, A additive, M multiplicative); SINGLE means NN, HOLT LN and WINTERS LM. ALPHA smooths the level, GAMMA the trend (not in NN, NA, NM, DA, DM), DELTA the season and PHI damps the trend; each defaults to 0.1, a parameter the model does not use is ignored. ALPHA, GAMMA and DELTA lie from 0 to 1, PHI above 0 and below 1. GRID tries every value from start to end by step (defaults 0 to 1 by 0.1 for ALPHA, by 0.2 for GAMMA and DELTA, 0.1 to 0.9 by 0.2 for PHI); every combination is fitted and the one with the smallest sum of squared one-step errors is kept. The output lists the ten smallest (all of them under TSET PRINT=DETAILED). Start values come from the published formulas: means of the first and last complete periods, initial seasonal factors from SEASON's method unless SEASFACT gives them (a list as long as the period, or a variable such as a saved SAF\_n whose first values are read). INITIAL gives the start level and trend instead. A seasonal model needs a period (PERIOD, TSET PERIOD or DATE) and four full periods; the multiplicative and exponential models need values above 0. The series may not have missing values inside the USE range. TSET NEWVAR saves FIT\_n (the one-step forecast) and ERR\_n per series; with PREDICT, FIT\_n holds the forecasts. APPLY repeats the previous EXSMOOTH, or the model named in quotes (MOD\_1, MOD\_2, ... in the order the session made them) with the subcommands given now replacing its own; SEASFACT is not carried over.

## Example

```text
EXSMOOTH VARIABLES=room_nights /MODEL=DA /PERIOD=7 /PHI=GRID /DELTA=0.25.
```

`statscraft help EXSMOOTH` prints the same text in the terminal. [All commands](README.md)
