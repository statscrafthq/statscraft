<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# MCONVERT

Matrix data between covariances and correlations with standard deviations.

## Syntax

```text
MCONVERT [[/MATRIX=] [IN({* | file})] [OUT({* | file})]] [/REPLACE | /APPEND].
```

Reads matrix materials (the layout MATRIX DATA makes: split variables, ROWTYPE\_, grouping variables, VARNAME\_, then the variables) from the active dataset or a file and writes the other form to the active dataset (replacing it) or a file. COV records become CORR records and a STDDEV record, the square roots of the variances (not added when the set has one).

CORR records become COV records and need the set's STDDEV record. Each run of records with the same split and grouping values is a set, converted on its own. Records of other types (MEAN, N, STDDEV ...) are copied. REPLACE, the default, writes the converted matrix in place of the original; APPEND writes each original set followed by its converted matrix. The matrix of a set must have one record per variable, in the order of the variables. The original stays as it was in the file or dataset it came from, unless that is also the output.

IN and OUT cannot name the same file.

## Example

```text
MCONVERT MATRIX=IN(*) OUT(correlations) /APPEND.
```

`statscraft help MCONVERT` prints the same text in the terminal. [All commands](README.md)
