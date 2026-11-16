<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# CREATE

New series from functions of existing series: lags, differences, smoothers, FFT.

## Syntax

```text
CREATE new names = function(series [, numbers]) [/new names = function(...) ...].
```

function: CSUM(series) running total; DIFF(series, order) differences; SDIFF(series, order \[, period\]) differences over a period; LAG(series, order \[, order\]) and LEAD(series, order \[, order\]) shifted copies, one per order of the range; MA(series, span \[, minimum\]) centered moving average; RMED(series, span \[, minimum\]) centered running median; PMA(series, span) mean of the span cases before; T4253H(series) compound smoother; FFT(series) real and imaginary parts; IFFT(real, imaginary) the series back.

Commas or blanks separate the items in parentheses; v1 TO v4 names numbered new series. A function makes one series per source (LAG and LEAD: per source and order, sources outer; FFT two per source; IFFT one per pair), and the names on the left must match that count.

An existing variable of the same name is replaced; a later equation may use a series an earlier one made. A value missing (user-missing too, unless TSET MISSING=INCLUDE) gives a missing result where it is used; MA, RMED and PMA restart after it. An even span is centered by averaging two neighbouring spans. Near the ends MA and RMED shrink the span symmetrically down to the minimum (of the same parity, raised by 1 otherwise; 1 is always allowed). SDIFF without a period uses TSET PERIOD, then the DATE period. T4253H, FFT and IFFT need a series without gaps inside it; FFT pads an odd length with a 0 and gives n/2 cases (the first case's imaginary part holds the highest frequency's cosine term), IFFT twice as many as it reads. With SPLIT FILE each group is a series of its own; cases a filter leaves out keep their value (system-missing in a new series). A table lists every new series with its first and last valid case, its number of valid values and its function.

## Example

```text
CREATE sales_lag1 TO sales_lag3 = LAG(sales, 1, 3) /trend = MA(sales, 6, 2).
```

`statscraft help CREATE` prints the same text in the terminal. [All commands](README.md)
