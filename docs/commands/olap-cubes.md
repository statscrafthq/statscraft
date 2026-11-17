<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# OLAP CUBES

Statistics of scale variables in nested groups, with differences between groups or variables.

## Syntax

```text
OLAP CUBES summaryvars BY groupvars [BY groupvars ...] [/CELLS=cell ... | NONE]
           [/CREATE ['label' | DEFAULT ...] GAC | GPC | GAC GPC (groupvar (value value) ...)]
           [/CREATE ['label' | DEFAULT ...] VAC | VPC | VAC VPC (var var) ...]
           [/TITLE='text'] [/FOOTNOTE='text'] [/HIDESMALLCOUNTS [COUNT=5]].
```

cell: as on MEANS; DEFAULT (SUM COUNT MEAN STDDEV SPCT NPCT) when CELLS is left out, ALL for every cell. The summary variables must be numeric. A Case Processing Summary has a row per summary variable and group combination; one OLAP Cubes table per combination of grouping variables (one from each BY) nests the categories of the first one outermost, each level with a Total, and gives a row per summary variable. GAC adds a row per pair of categories of one grouping variable with every statistic of the second minus that of the first; GPC the same change as a percentage of the first. VAC and VPC do the same between two summary variables in every group. Labels name the differences in order (each pair gives its types in the order written); DEFAULT keeps the generated name. A case counts for a summary variable when it is valid on it and on the grouping variables of the table. HIDESMALLCOUNTS shows counts below n (from 2, default 5) as \<n. TITLE and FOOTNOTE: one line per quoted string, \\n breaks a line.

A subcommand given twice: the last one counts. Weighted when a weight variable is set.

## Example

```text
OLAP CUBES revenue cost BY year BY store /CELLS=SUM MEAN COUNT
  /CREATE 'Growth' DEFAULT GAC GPC (year (2024 2025)) /TITLE='Store results'.
```

`statscraft help OLAP CUBES` prints the same text in the terminal. [All commands](README.md)
