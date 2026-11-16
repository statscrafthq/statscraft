<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# SPECTRA

Periodogram, smoothed spectral density and cross-spectra of series.

## Syntax

```text
SPECTRA VARIABLES= series [/CENTER [NO]] [/CROSS [NO]]
        [/WINDOW=HAMMING | TUKEY | PARZEN | BARTLETT | UNIT | DANIELL [(span)] | NONE | weights]
        [/PLOT=P S CS QS PH A G K ALL NONE [BY FREQ | BY PERIOD]]
        [/SAVE=FREQ(root) PER(root) SIN(root) COS(root) P(root) S(root) RC(root) IC(root)
               CS(root) QS(root) PH(root) A(root) G(root) K(root)] [/APPLY].
```

The terms are listed for the frequencies K/n, K = 0 to n/2: the sine and cosine coefficients and the periodogram (a^2 + b^2) n/2. A series needs six cases and no missing values (use USE or RMV); of an odd count the first case is dropped. CENTER subtracts the mean, else the periodogram at frequency 0 is 2n times its square. The density is a moving average of the periodogram whose weights come from the window's kernel at the span's own frequencies, scaled to sum to 1; the default is HAMMING(5), an even span loses one, a span of 1 or NONE leaves the periodogram as it is, and a list of weights runs from the leftmost to the rightmost (an even list lacks the last one).

Values beyond the ends are mirrored. CROSS pairs the first series with each later one: real and imaginary cross-periodogram, cospectrum (CS), quadrature spectrum (QS), phase (PH, radians), cross amplitude (A), gain of the second series over the first (G) and squared coherency (K). Charts come as data, by frequency and by period unless BY picks one; P (the default) and S are drawn on natural logs. SAVE writes one variable per series (root\_1, root\_2 by their place on VARIABLES) or pair (root\_1\_2), FREQ and PER under the root itself, from the first case used; a name that exists is skipped. APPLY repeats the previous SPECTRA, SAVE excepted.

## Example

```text
SPECTRA VARIABLES=river_flow rainfall /CENTER /CROSS /WINDOW=PARZEN(7) /PLOT=S K BY FREQ.
```

`statscraft help SPECTRA` prints the same text in the terminal. [All commands](README.md)
