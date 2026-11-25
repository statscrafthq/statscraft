<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# WLS

Weighted least squares: weights from a power of a source variable, or a given weight.

## Syntax

```text
WLS [VARIABLES=] dependent WITH independents
   [/SOURCE=variable] [/DELTA=powers] [/WEIGHT=variable] [/CONSTANT | /NOCONSTANT]
   [/PRINT=BEST | ALL] [/SAVE=WEIGHT] [/APPLY].
```

Powers: numbers and grids first TO last BY step (or first BY step TO last), at most 150 in all; POWER is another name for DELTA and every DELTA counts. A case's weight is its SOURCE value raised to minus the power. SOURCE defaults to the first independent and the power to 1; with neither SOURCE, DELTA nor WEIGHT a warning says so. Powers are kept within -6.5 and 7.5, and cases whose source is not above 0 are left out. WEIGHT names a variable that holds the weights instead (cases with a weight not above 0 are left out); it cannot go with SOURCE or DELTA. A case missing on any named variable is left out; case weights (WEIGHT command) multiply the regression weights and count as cases.

For every power the log-likelihood is 0.5 (-n ln 2 pi - n ln s^2 + sum ln w - (n - p)), with s^2 the weighted mean square error and p the number of coefficients; the Log-Likelihood Values table lists it and the power with the largest one is used. For that power (or for the WEIGHT variable) come the Model Summary (R, R square, adjusted R square, standard error and log-likelihood), ANOVA and Coefficients (B, standard error, standardized Beta with its standard error, t and significance). PRINT=ALL adds the coefficients of every power; without PRINT, TSET PRINT=DETAILED does the same and BRIEF leaves out the log-likelihoods.

SAVE=WEIGHT keeps the chosen weights as WGT\_n, the first free n. Without SAVE, TSET NEWVAR decides: CURRENT (the default) saves WGT\_1, replacing it, ALL the next free WGT\_n, NONE nothing. CONSTANT or NOCONSTANT defaults to TSET. APPLY runs the previous WLS's specifications again: the variables of the new command when it names some, else the old ones, and any subcommand it gives replaces the old one (a WEIGHT replaces SOURCE and DELTA and the other way round; SAVE is not carried over); a model named in quotes is not supported.

## Example

```text
WLS rent WITH floor_area rooms /SOURCE=floor_area /POWER=-1 TO 3 BY 0.25 /PRINT=ALL.
```

`statscraft help WLS` prints the same text in the terminal. [All commands](README.md)
