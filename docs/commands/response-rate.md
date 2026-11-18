<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# RESPONSE RATE

Response rates of a numerator over a denominator in each group, with cumulative rates and limits.

## Syntax

```text
RESPONSE RATE /VARIABLES NUMERATOR=var DENOMINATOR=var GROUP=var
              [/MINRATE RATE=NONE | RATE=value | FORMULA=cost, revenue]
              [/MAXCOUNT COUNT=ALL | COUNT=n | PERCENT=value].
```

NUMERATOR and DENOMINATOR are numeric; GROUP is numeric or string. Both are summed within each value of GROUP, and the rate of a group is 100 times its numerator sum over its denominator sum. Groups go in ascending order of their value; the cumulative rate of a group adds up the numerator and the denominator over it and every lower group before dividing. A group whose denominator sum is 0 or below is left out, with a warning. MINRATE sets the lowest acceptable cumulative rate in percent: a value above 0 and below 100, or 100 times cost over revenue per response (both above 0). MAXCOUNT sets the highest acceptable cumulative denominator sum: a whole number, or a percentage (above 0, below 100) of the denominator total. With either limit, each table row is coloured green when it keeps every limit given (cumulative rate at least MINRATE, cumulative sum at most MAXCOUNT) and red when it does not, and the caption states the limits. Table: Response Rates, one row per group with the sums, the rate, the cumulative sums and the cumulative rate. Charts: the rates and cumulative rates by group with a line at MINRATE, and the cumulative denominator sum by group with a line at MAXCOUNT. A case missing on any of the three variables is left out; a case weight multiplies both values. /MINRATE=value and /MAXCOUNT=n are short for RATE= and COUNT=. Every word is written in full and each subcommand given once.

## Example

```text
RESPONSE RATE /VARIABLES NUMERATOR=orders DENOMINATOR=letters GROUP=area_rank
  /MINRATE FORMULA=0.6, 30 /MAXCOUNT PERCENT=50.
```

`statscraft help RESPONSE RATE` prints the same text in the terminal. [All commands](README.md)
