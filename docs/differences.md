# Differences and limits

StatsCraft runs syntax compatible with the reference program syntax. Its behaviour follows the published documentation of the reference program. GNU PSPP, another free program for the same language, is used only to compare numbers. This page lists where StatsCraft differs from either on purpose, the choices it made where the documentation says nothing, and what does not work yet.

## Where StatsCraft follows the documentation and GNU PSPP does not

- **RECODE ... INTO an existing variable** keeps the old value of a case that no rule matches. A new target variable gets system-missing.
- **Fractional weights** are used as they are, not rounded, in counts, percentages and statistics. Only significance tests, confidence intervals, standard errors and T-TEST effect sizes round them, as documented.
- **Zero, negative and missing weights** leave the case out of FREQUENCIES, unweighted CTABLES counts and the MEANS minimum and maximum. CORRELATIONS counts a negative weight as 0.
- **DESCRIPTIVES**: "Valid N (listwise)" always counts the cases valid on every listed variable. String variables are skipped with a warning. Saved z-scores are named Z plus the variable name, then ZSC001 to ZSC099, STDZ01 to STDZ09, ZZZZ01 to ZZZZ09 and ZQZQ01 to ZQZQ09.
- **CROSSTABS**: expected counts in the margins are counts; FORMAT=DVALUE reverses the rows only; `/CELLS` and `/STATISTICS` without a list are accepted.
- **CTABLES**: an empty count is 0 and EMPTY decides how it shows; the median and percentiles use the averaged empirical distribution; a variable without a measurement level is classed by all the documented rules. With CLABELS ROWLABELS=OPPOSITE the moved categories sit inside the column categories and outside the summary labels.
- **Expressions**: `0 ** missing`, `missing ** 0` and `0 / 0` are missing; MISSING() sees user-missing strings; month 0 in a date is invalid; RND and TRUNC allow for rounding error relative to the size of the value.
- **ADD FILES** keeps the weight when every file is weighted by the same variable. **MATCH FILES** fails when a TABLE file holds a key twice.
- **ONEWAY**: Games-Howell uses separate variances.
- **MEANS, CORRELATIONS, EXAMINE**: undefined skewness and kurtosis are left empty; the CORRELATIONS listwise descriptives, the EXAMINE hinges with small weights, percentiles of small groups, CINTERVAL, listwise totals and the default descriptives table follow the documentation.

## Syntax StatsCraft accepts beyond the documentation

These are accepted where the documented syntax refuses them. Results on documented syntax do not change.

- ADD FILES widens a string variable to its widest width across the files.
- COUNT and RECODE accept MISSING for string variables.
- CTABLES accepts MIN and MAX for MINIMUM and MAXIMUM, and a format before the label in a summary list.
- COUNT is allowed inside DO IF.

## Choices where the documentation is silent

- A logical operand other than 0, 1 or missing is an error. `0 ** 0` is missing.
- RECODE COPY into a narrower string cuts the value. A reversed range such as `10 THRU 1` is turned around, with a warning.
- Harmonic and geometric means need every value positive; otherwise the cell is empty.
- MEANS shows one report per dependent variable. Its ANOVA uses the first control variable only.
- A CORRELATIONS coefficient marked by NOSIG shows 3 decimals and stars, explained in a caption.
- Levene's test is based on the mean.
- CROSSTABS shows Fisher's exact test for every 2 x 2 table.
- EXAMINE: Shapiro-Wilk uses Royston's algorithm for 3 to 5000 cases; the Lilliefors significance comes from a published table and shows ">0.200" beyond it; M-estimators start from the median, and a cell is empty when the absolute deviation is 0; percentiles below the first or above the last value take the minimum or maximum.
- CTABLES significance tests: column letters run across the whole table (A, B, ... then AA), so no two column groups share a letter; the Total of an outer variable is a subtable of its own and is tested; a period marks every column left out of a comparison; ADJUST defaults to BONFERRONI and MEANSVARIANCE to ALLCATS; significant means p below alpha (Benjamini-Hochberg: at most alpha); a repeated COMPARETEST of the same type replaces the earlier one; the means test needs MEAN, UMEAN alone does not qualify.
- CTABLES layers give one table per layer category or combination, named in the title. A subtotal stays shown under EMPTY=EXCLUDE. KEY=function sorts categories without a value last and keeps value order for ties. Computed categories are left out of totals and tests.
- Multiple response sets: in a category set a value that is user-missing in any of its variables is a missing category, and an empty string counts as a value.

## Not supported yet

- **Commands.** The list of commands of the language that StatsCraft recognises but does not run is at the end of [the command index](commands/README.md#not-supported-yet). That list is generated from the program, so it is always current. A script that uses one stops at that line, and nothing is changed.
- **Subcommands.** Every command page lists what it accepts. Notably: FREQUENCIES takes only VARIABLES; ONEWAY has no POLYNOMIAL, RANGES, EFFECTS or WALLER; NONPAR CORR has no MATRIX or CI; EXAMINE computes the normality tests but draws no plots (no STEMLEAF, BOXPLOT, HISTOGRAM or SPREADLEVEL).
- **Charts.** No command draws charts yet.
- **Data files.** StatsCraft reads and writes .sav and .zsav files. Portable (.por) files, spreadsheets and text files cannot be read yet. An old .sav file that does not state its code page is read as Western European (windows-1252); there is no way to choose another encoding yet.
- **Output.** Tables come out as Markdown, JSON, HTML (from the window) and Excel workbooks.
- **Installer.** There is no installer; the release zips are unpacked by hand.
