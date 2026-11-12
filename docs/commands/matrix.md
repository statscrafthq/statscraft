<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# MATRIX

Run a program in the matrix language, up to END MATRIX.

## Syntax

```text
MATRIX.
  COMPUTE m = expr | m(rows, cols) = expr.
  CALL EIGEN(m, vectors, values) | SETDIAG(m, v) | SVD(m, u, q, v).
  PRINT [m] [/FORMAT=format] [/TITLE='text'] [/SPACE=n | NEWPAGE]
        [/RLABELS=labels | /RNAMES=vector] [/CLABELS=labels | /CNAMES=vector].
  DO IF expr. ... [ELSE IF expr. ...] [ELSE. ...] END IF.
  LOOP [i = first TO last [BY step]] [IF expr]. ... [BREAK.] ... END LOOP [IF expr].
  READ m [/FILE=file] /FIELD=c1 TO c2 [BY w] [/SIZE=expr] [/MODE=RECTANGULAR | SYMMETRIC]
        [/REREAD] [/FORMAT=format].
  WRITE m [/OUTFILE=file] /FIELD=c1 TO c2 [BY w] [/MODE=RECTANGULAR | TRIANGULAR] [/HOLD]
        [/FORMAT=format].
  GET m [/FILE=file | *] [/VARIABLES=varlist] [/NAMES=name] [/MISSING=ACCEPT | OMIT | value]
        [/SYSMIS=OMIT | value].
  SAVE m [/OUTFILE=file | *] [/VARIABLES=names] [/NAMES=vector] [/STRINGS=names].
  MGET [/FILE=file | *] [/TYPE=COV CORR MEAN STDDEV N COUNT].
  MSAVE m /TYPE=COV | CORR | MEAN | STDDEV | N | COUNT [/OUTFILE=file] [/VARIABLES=names]
        [/SNAMES=names] [/SPLIT=vector] [/FNAMES=names] [/FACTOR=vector].
  DISPLAY [DICTIONARY] [STATUS].
  RELEASE m [, m ...].
```

END MATRIX.

Matrices: {1, 2; 3, 4} (commas between elements, semicolons between rows; the parts may be matrices), a:b and a:b:step for whole numbers, m(i, j) and v(i) with vectors of positions or : for all.

Operators from the tightest: unary -, :, \*\* (matrix power) and &\*\*, \* (matrix product), / &\* and &/ (elementwise), + -, \< \<= \> \>= = \<\> (or LT LE GT GE EQ NE), NOT, AND, OR and XOR; a scalar goes with any matrix. PRINT, WRITE, SAVE and MSAVE take a name, a call, a matrix in braces or a power of one; other expressions go in parentheses, as (A+B). In the expression of a subcommand a slash starts the next subcommand, so a division goes in brackets.

Functions: ABS ALL ANY ARSIN ARTAN BLOCK CHOL CMAX CMIN COS CSSQ CSUM DESIGN DET DIAG EOF EVAL EXP GINV GRADE GSCH IDENT INV KRONEKER LG10 LN MAGIC MAKE MDIAG MMAX MMIN MOD MSSQ MSUM NCOL NROW RANK RESHAPE RMAX RMIN RND RNKORDER RSSQ RSUM SIN SOLVE SQRT SSCP SVAL SWEEP T (TRANSPOS) TRACE TRUNC; UNIFORM(rows, cols) and NORMAL(rows, cols) give random matrices, UNIFORM(max) and NORMAL(sd) random elements; CDFNORM CHICDF FCDF TCDF PROBIT and the families CDF.x, IDF.x, PDF.x, NCDF.x, NPDF.x, SIG.x and RV.x work element by element, a scalar argument going with any matrix.

PRINT gives a table; without FORMAT the numbers share one format and are divided by a power of 10, named in the caption, when they are very large or small. A string element holds up to 8 bytes and prints with an A format. Every loop, also one with an index, stops after SET MXLOOPS iterations (default 40) with a warning. The first READ, WRITE, SAVE and MSAVE name their file; later ones use the last one named. SAVE, MSAVE and WRITE write their files at END MATRIX; SAVE /OUTFILE=\* replaces the active dataset then. An error stops the program at its statement, and nothing is saved.

## Example

```text
MATRIX.
COMPUTE A = {4, 2; 2, 3}.
CALL EIGEN(A, V, E).
PRINT E /TITLE='Eigenvalues'.
PRINT INV(A) /FORMAT=F8.3.
END MATRIX.
```

`statscraft help MATRIX` prints the same text in the terminal. [All commands](README.md)
