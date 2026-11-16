<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# VALIDATEDATA

Find suspicious variables, incomplete or duplicate IDs, empty cases and rule violations.

## Syntax

```text
VALIDATEDATA [VARIABLES=varlist] [ID=varlist] [CROSSVARRULES=$VD.CRule[n] ... | n ... | ALL]
  [/VARCHECKS [STATUS=ON | OFF] [PCTMISSING=70] [PCTEQUAL=95] [PCTUNEQUAL=90] [CV=0.001]
              [STDDEV=0]]
  [/IDCHECKS [INCOMPLETE] [DUPLICATE] | NONE]
  [/CASECHECKS [REPORTEMPTY=YES | NO] [SCOPE=ANALYSISVARS | ALLVARS]]
  [/RULESUMMARIES [BYVARIABLE] [BYRULE] | NONE]
  [/CASEREPORT [DISPLAY=YES | NO] [MINVIOLATIONS=1] [CASELIMIT=FIRSTN(500) | NONE]]
  [/SAVE [EMPTYCASE[(name)]] [INCOMPLETEID[(name)]] [DUPLICATEID[(name)]]
         [RULEVIOLATIONS[(name)]]].
```

Give at least one of VARIABLES (analysis variables), ID (the variables that together identify a case) and CROSSVARRULES. Keywords are written in full; a subcommand may come once and may not be empty. VARCHECKS lists analysis variables with more missing values than PCTMISSING percent of the cases; nominal and ordinal ones (strings too) whose largest category holds more than PCTEQUAL percent of the valid values, or whose categories have one case each in more than PCTUNEQUAL percent of them; scale ones whose absolute coefficient of variation is below CV (0 turns it off; skipped when the mean is 0) or whose standard deviation is below STDDEV. A variable of unknown level gets one from its values. IDCHECKS lists the cases whose ID has a missing or blank part and the groups of cases that share a complete ID. CASECHECKS lists empty cases: every analysis variable (SCOPE=ALLVARS: every variable but the ID, split and rule outcome variables) missing or blank.

Rules live in the dictionary. A single-variable rule is the data file attribute \$VD.SRule\[n\], a text of Name='value' properties separated by commas, among them Label and Type (Numeric, String or Date); an analysis variable links to it with the variable attribute \$VD.SRuleRef\[n\] holding Rule='\$VD.SRule\[n\]' and OutcomeVar='name'. A cross-variable rule is \$VD.CRule\[n\] with Label, Expression and OutcomeVar. The outcome variable, made beforehand with COMPUTE, is 1 for a case that breaks the rule; mark it with the variable attribute \$VD.RuleOutcomeVar('Yes') so the variable lists skip it. Broken or unknown rules and links are skipped with a warning; one outcome variable used by two rules is an error. RULESUMMARIES counts single-variable violations by variable (default) or by rule; cross-variable rules are always counted per rule. CASEREPORT lists the cases with at least MINVIOLATIONS violations, the first 500 by default. SAVE adds EmptyCase, IncompleteID (1 or 0), DuplicateIDGroup (the same number for cases sharing an ID, else 0) and ValidationRuleViolations (the count per case); a default name in use gets \_1, \_2 and so on, a given name in use is an error. Missing means user- or system-missing. Works within SPLIT FILE groups and on the cases FILTER keeps; the weight is ignored with a warning.

## Example

```text
VALIDATEDATA VARIABLES=age income visits ID=household member
  /VARCHECKS PCTMISSING=40 CV=0.01 /SAVE DUPLICATEID(dupgroup) EMPTYCASE.
```

`statscraft help VALIDATEDATA` prints the same text in the terminal. [All commands](README.md)
