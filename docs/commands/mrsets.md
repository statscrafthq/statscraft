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

MDGROUP (dichotomies): every variable is one category, labeled with its variable label or name; a case has a response in it when the variable holds VALUE, a whole number or, for strings, a quoted text no wider than the narrowest variable. CATEGORYLABELS=COUNTEDVALUES labels each category with the value label of VALUE in its variable instead; every variable needs one and they must differ.

LABELSOURCE=VARLABEL (only with COUNTEDVALUES, instead of LABEL) takes the set label from the first variable of the set that has a variable label, else the name of the first variable.

MCGROUP (categories): the values of the variables are the categories, labeled with the value labels of the first variable that has one.

DISPLAY lists the sets in a table. Sets are saved with the data file and used by CTABLES.

## Example

```text
GET FILE='survey.sav'.
MRSETS /DISPLAY NAME=ALL.
MRSETS /MDGROUP NAME=$digital LABEL='Digital channels'
  VARIABLES=ch_web ch_app ch_email VALUE=1
  /DISPLAY NAME=[$digital].
CTABLES /TABLE $digital [COUNT, COLPCT.COUNT PCT5.1] BY region.
```

The example reads the sample data file [survey.sav](../sample/survey.sav), an invented customer survey; save it in the folder you run the syntax from.

## Example output

### Multiple Response Sets

| Name | Label | Type | Counted value | Variables |
| --- | --- | --- | --- | --- |
| $channels | Channels used | Multiple dichotomy | 1 | ch_web ch_app ch_phone ch_store ch_email |

`statscraft help MRSETS` prints the same text in the terminal. [All commands](README.md)
