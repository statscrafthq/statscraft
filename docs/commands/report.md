<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# REPORT

Case listings and summary statistics by break groups, laid out as a report.

## Syntax

```text
REPORT [FORMAT=[MANUAL | AUTOMATIC] [NOLIST | LIST[(n)]] [ALIGN(LEFT | CENTER | RIGHT)] [TSPACE(1)]
        [CHDSPACE(1)] [FTSPACE(1)] [SUMSPACE(1)] [COLSPACE(4)] [BRKSPACE(n | -1)] [LENGTH(t,b)]
        [MARGINS(l,r)] [CHALIGN(TOP | BOTTOM)] [UNDERSCORE(ON | OFF)] [PAGE1(1)] [MISSING '.']
        [ONEBREAKCOLUMN ON | OFF] [INDENT(2)] [CHWRAP ON | OFF] [PREVIEW ON | OFF]]
   [/OUTFILE='path'] [/STRING=name (var [(width)] [(BLANK)] 'literal' ...) ...]
   /VARIABLES=var [(VALUE | LABEL | DUMMY)] [+ var ...] ['heading' ...] [(width)]
      [(OFFSET(n | CENTER))] [(LEFT | CENTER | RIGHT)] ...
   [/MISSING=VAR | NONE | LIST[([varlist] [n])]] [/TITLE=[LEFT | CENTER | RIGHT] 'line' ...]
   [/FOOTNOTE=[LEFT | CENTER | RIGHT] 'line' ...]
   [/BREAK=varlist | (TOTAL) ['heading' ...] [(width)] [(VALUE | LABEL)] [(TOTAL | NOTOTAL)]
      [(SKIP(1))] [(PAGE)] [(OFFSET(n | CENTER))] [(UNDERSCORE[(varlist)])] [(LEFT | CENTER | RIGHT)]
      [(NAME | NONAME)] [/SUMMARY=function ... ['title'] [(break column)] [SKIP(n)] | PREVIOUS[(n)]] ...] ...
```

functions: VALIDN SUM MIN MAX MEAN STDDEV (SD, STDEV) VARIANCE KURTOSIS SKEWNESS MEDIAN(min,max) MODE(min,max) PGT(n) PLT(n) PIN(min,max) FREQUENCY(min,max) PERCENT(min,max), each with an optional (var \[(format \| PLAIN)\] \[(decimals)\] ...); composite functions DIVIDE(a b \[factor\]) MULTIPLY PCT(a b) SUBTRACT(a b) ADD GREAT LEAST AVERAGE of aggregates such as SUM(var) and numbers, with an optional (column \[(format)\] \[(decimals)\]).

The report is a table: a column for each BREAK (with ONEBREAKCOLUMN ON one for all, indented by INDENT), then one for each report column, headed by its heading or the variable label or name. LIST lists the cases, each value in its print format or with LABEL as its value label; a stacked column lists each of its variables on a line of its own. A break group starts when a value of its BREAK or an earlier one changes (sort the cases first): its values head it, and its SUMMARY lines follow with their title (by default the function's) in the break column. The statistics use the valid values and the weight; MISSING=NONE counts user-missing values as valid, and MISSING=LIST leaves out the cases missing on n of the variables. MEDIAN, MODE, FREQUENCY and PERCENT count the whole numbers of the range, cutting decimals off.

TOTAL on the first BREAK repeats its first SUMMARY for all cases at the end, under Grand Total. Formats follow the manual's defaults for each function. TITLE and FOOTNOTE lines (by default the text of the TITLE command) show above and below the report; )PAGE, )DATE and )var in them give the page number, the date and the value label of the first case. OUTFILE writes the report as text, with the column widths, spacing and alignment, instead of showing it; a later REPORT of the same session appends to the file. The report is one page, so LENGTH, PAGE and PREVIEW change nothing.

## Example

```text
SORT CASES BY dept.
REPORT FORMAT=LIST /VARIABLES=product (LABEL) sales 'Annual' 'Sales'
  /BREAK=dept 'Department' (LABEL) /SUMMARY=VALIDN(product) MEAN(sales).
```

`statscraft help REPORT` prints the same text in the terminal. [All commands](README.md)
