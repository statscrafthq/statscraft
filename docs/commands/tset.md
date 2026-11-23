<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# TSET

Settings shared by the time series procedures.

## Syntax

```text
TSET [PRINT=BRIEF | DEFAULT | DETAILED] [NEWVAR=CURRENT | NONE | ALL] [MXAUTO=16]
     [MXCROSS=7] [MXNEWVARS=60] [MXPREDICT=60] [MISSING=EXCLUDE | INCLUDE] [CIN=95]
     [TOLER=0.0001] [CNVERGE=0.001] [ACFSE=IND | MA] [PERIOD=n] [ID=variable]
     [CONSTANT | NOCONSTANT] [DEFAULT].
```

The values shown are the starting ones. Slashes and equals signs are optional, and DEFAULT as a value puts that one setting back; DEFAULT alone puts back all of them. A setting holds for the rest of the session, until TSET changes it; a subcommand of a procedure that does the same job wins for that procedure. Who reads what: ACF, PACF and CCF take MXAUTO, MXCROSS, ACFSE, PERIOD and MISSING; SEASON takes PERIOD, PRINT, NEWVAR, MXNEWVARS and MISSING; PPLOT takes PERIOD and MISSING; CREATE takes PERIOD (for SDIFF) and MISSING; CURVEFIT takes NEWVAR, MXNEWVARS, CIN, TOLER, CONSTANT, ID and MISSING; WLS takes NEWVAR, MXNEWVARS, PRINT and CONSTANT. NEWVAR: CURRENT saves the new variables of a procedure under names numbered from 1, replacing variables of those names; ALL saves them under the next free numbers; NONE saves nothing unless the procedure's SAVE asks. A procedure that would make more than MXNEWVARS variables saves none, with a warning. PERIOD wins over the period that DATE variables give. MXPREDICT and CNVERGE are kept and shown by TSHOW; no procedure reads them. NEWVARS is taken as NEWVAR.

## Example

```text
TSET PERIOD=4 MXAUTO=12 NEWVAR=ALL.
```

`statscraft help TSET` prints the same text in the terminal. [All commands](README.md)
