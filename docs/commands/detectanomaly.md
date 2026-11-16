<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# DETECTANOMALY

Unusual cases: peer groups by two-step clustering, an anomaly index and the variables behind it.

## Syntax

```text
DETECTANOMALY [/VARIABLES [CATEGORICAL=varlist] [SCALE=varlist] [ID=var] [EXCEPT=varlist]]
              [/HANDLEMISSING [APPLY=NO | YES] [CREATEMISPROPVAR=NO | YES]]
              [/CRITERIA [MINNUMPEERS=1] [MAXNUMPEERS=15] [MLWEIGHT=6] [NUMREASONS=1]
                         [PCTANOMALOUSCASES=5] [NUMANOMALOUSCASES=n] [ANOMALYCUTPOINT=2 | NONE]]
              [/SAVE [ANOMALY[(name)]] [PEERID[(name)]] [PEERSIZE[(name)]] [PEERPCTSIZE[(name)]]
                     [REASONVAR[(root)]] [REASONMEASURE[(root)]] [REASONVALUE[(root)]] [REASONNORM[(root)]]]
              [/PRINT [CPS] [ANOMALYLIST] [NORMS] [ANOMALYSUMMARY] [REASONSUMMARY] [NONE]] [/OUTFILE MODEL='file'].
```

Without CATEGORICAL and SCALE every variable but ID and EXCEPT is used, strings and nominal or ordinal variables as categorical. Keywords are written in full, each subcommand once. The cases are grouped into peer groups by two-step clustering (log-likelihood distance, the variances eased by the variable's overall variance over MLWEIGHT, BIC choosing between MINNUMPEERS and MAXNUMPEERS). For each case, each variable's share of the distance between the case and its peer group measures how far it strays; their sum over the peer group's mean sum is the anomaly index, and a variable's share of the sum its impact. The anomaly list takes the cases of highest index, PCTANOMALOUSCASES percent (rounded down) or NUMANOMALOUSCASES of them, keeping those with an index of at least ANOMALYCUTPOINT; its reasons are each case's NUMREASONS variables of largest impact, with the value and the peer group's norm (mean or mode).

Missing values leave the case out unless APPLY=YES, which puts the mean in for a continuous variable and makes missing a category of a categorical one; CREATEMISPROPVAR=YES then adds the share of missing values as a variable. A variable that does not vary is left out. Tables: Case Processing Summary (CPS), the anomaly index, peer and reason lists (ANOMALYLIST, the default), Scale and Categorical Variable Norms (NORMS), Anomaly Index Summary, Reason Summary.

SAVE adds AnomalyIndex, PeerId, PeerSize, PeerPctSize and per reason k ReasonVar\_k, ReasonMeasure\_k, ReasonValue\_k and ReasonNorm\_k, a free suffix added to a default name in use.

WEIGHT and SPLIT FILE are not used. OUTFILE writes no file yet.

## Example

```text
DETECTANOMALY /VARIABLES SCALE=amount days CATEGORICAL=channel ID=order_no
  /CRITERIA NUMREASONS=2 PCTANOMALOUSCASES=2 /SAVE ANOMALY PEERID REASONVAR /PRINT CPS ANOMALYLIST NORMS.
```

`statscraft help DETECTANOMALY` prints the same text in the terminal. [All commands](README.md)
