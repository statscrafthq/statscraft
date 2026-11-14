<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# PROXIMITIES

Similarities, dissimilarities or distances between cases or variables by one of 37 measures.

## Syntax

```text
PROXIMITIES varlist | /MATRIX=IN('file' | dataset | *)
   [/VIEW=CASE | VARIABLE] [/STANDARDIZE=[VARIABLE | CASE] [NONE | Z | SD | RANGE | MAX | MEAN | RESCALE]]
   [/MEASURE=[EUCLID | SEUCLID | COSINE | CORRELATION | BLOCK | CHEBYCHEV | MINKOWSKI(p) | POWER(p,r) |
             CHISQ | PH2 | binary[(p[,np])] | NONE] [ABSOLUTE] [REVERSE] [RESCALE]]
   [/PRINT=PROXIMITIES | NONE] [/ID=var] [/MISSING=EXCLUDE | INCLUDE]
   [/MATRIX=[IN('file' | dataset | *)] [OUT('file' | dataset | *)]].
```

binary: the 27 binary measures of CLUSTER, from the 2 x 2 table of presence (p, default 1) and absence (np, default 0; with p alone every other value is absence). The items are the cases (VIEW=CASE, the default) or the variables. A case missing on any variable is dropped either way; the weight variable is not used. STANDARDIZE works within each variable (default) or each case before the measure: Z (the method when none is named) gives mean 0 and standard deviation 1, SD standard deviation 1, RANGE range 1, MAX largest value 1, MEAN mean 1, RESCALE values from 0 to 1; a constant set of values stays as it is, except under Z (all 0), RESCALE (all 0.5) and MEAN (plus 1). It cannot go with a binary measure. The default measure is EUCLID. ABSOLUTE drops the sign of every proximity, REVERSE changes it (a similarity becomes a dissimilarity and back) and RESCALE maps the smallest proximity to 0 and the largest to 1; they run in this order whatever the order written, on the whole matrix with its diagonal. Tables: Case Processing Summary and Proximity Matrix (left out with PRINT=NONE); a pair for which the measure is undefined is empty. ID labels the cases. MATRIX=OUT writes a matrix file that PROXIMITIES, CLUSTER, PROXSCAL and ALSCAL read with MATRIX=IN: ROWTYPE\_ (PROX, with the measure and SIMILARITY or DISSIMILARITY as its value label), the ID variable, CASENO\_ (the case number), VARNAME\_ and one variable per item, VAR1, VAR2, ... for cases and the variables' own names for VIEW=VARIABLE; under SPLIT FILE every group adds its own matrix after the split variables. OUT(\*) replaces the active dataset. MATRIX=IN reads such a file: no measure is computed from it (MEASURE and STANDARDIZE are ignored with a warning), only ABSOLUTE, REVERSE and RESCALE apply.

## Example

```text
PROXIMITIES taste aroma price /STANDARDIZE=VARIABLE RESCALE /MEASURE=BLOCK /ID=brand /MATRIX=OUT(dist).
```

`statscraft help PROXIMITIES` prints the same text in the terminal. [All commands](README.md)
