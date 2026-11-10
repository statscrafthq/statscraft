<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# MRSETS

Define, delete and list multiple response sets.

## Syntax

```text
MRSETS [/]MDGROUP NAME=$name [LABEL='text' | LABELSOURCE=VARLABEL] [CATEGORYLABELS=VARLABELS | COUNTEDVALUES]
       VARIABLES=varlist VALUE=value
       /MCGROUP NAME=$name [LABEL='text'] VARIABLES=varlist
       /DELETE NAME=[$name ...] | ALL
       /DISPLAY NAME=[$name ...] | ALL.
```

Subcommands run in the order written and may repeat. A set name starts with \$; defining an existing name replaces that set. A set has at least two variables, all numeric or all strings.

MDGROUP (dichotomies): every variable is one category, labelled with its variable label or name; a case has a response in it when the variable holds VALUE, a whole number or, for strings, a quoted text no wider than the narrowest variable. CATEGORYLABELS=COUNTEDVALUES labels each category with the value label of VALUE in its variable instead; every variable needs one and they must differ.

LABELSOURCE=VARLABEL (only with COUNTEDVALUES, instead of LABEL) takes the set label from the first variable of the set that has a variable label, else the name of the first variable.

MCGROUP (categories): the values of the variables are the categories, labelled with the value labels of the first variable that has one.

DISPLAY lists the sets in a table. Sets are saved with the data file and used by CTABLES.

## Example

```text
MRSETS /MDGROUP NAME=$channels LABEL='Contact channels' VARIABLES=q5_1 TO q5_5 VALUE=1 /DISPLAY NAME=ALL.
```

`statscraft help MRSETS` prints the same text in the terminal. [All commands](README.md)
