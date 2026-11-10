# Test fixtures for the .sav reader

Small system files (`.sav`, `.zsav`) with known ground truth, used to test the StatsCraft reader.

All data is synthetic and was created for this project. The survey, its question texts, the provider
names and every value are invented. Nothing was downloaded or copied from another dataset, and no
software of the format's original vendor and no GNU PSPP code or test file was used to make these files.

## Files

| File | Writer | Compression | Cases x variables | Purpose |
|---|---|---|---|---|
| `survey_basic.sav` | pyreadstat | none | 200 x 19 | Typical survey file: value labels, discrete user-missing codes (98, 99) that occur in the data, a user-missing range (97 to 99), system-missing values, F8.2 decimals, an A10 and an A200 string (with empty strings), a DATE11 date, five 0/1 items `q5_1..q5_5`, three mention variables `q6_mention1..3`, a non-integer `weight` variable, a file label |
| `survey_basic.zsav` | pyreadstat | zlib (ZSAV) | 200 x 19 | The same content, zsav-compressed |
| `survey_basic_phpwriter.sav` | pure-PHP writer | bytecode | 200 x 19 | The same content written by a second, independent pure-PHP writer. The only file with multiple-response set definitions (`$q5` dichotomy, `$q6` category) and a designated weight variable |
| `unicode.sav` | pyreadstat | none | 20 x 6 | UTF-8 text in the file label, variable labels, value labels and string values: Estonian (õ ä ö ü š ž), German, Cyrillic, Greek, CJK and emoji. Three non-ASCII variable names: `linn_õäöü`, `оценка`, `コメント` |
| `edge_cases.sav` | pyreadstat | bytecode | 12 x 12 | See "Edge cases" below |
| `empty.sav` | pyreadstat | none | 0 x 4 | Valid dictionary (labels, value labels, a missing value, a date format), zero cases |
| `wide.sav` | pyreadstat | bytecode | 10 x 300 | 300 numeric variables `v001..v300`, about 2% system-missing |

Other files:

* `<name>.expected.json`: ground truth for `<name>.sav`. The zsav file has `survey_basic.zsav.expected.json`
  (rule: strip a trailing `.sav` from the file name, then append `.expected.json`).
* `survey_basic_phpwriter.input.json`: written by `generate.py`, read by the PHP writer. It holds
  the expected block plus ALL 200 cases of the survey, so it also works as full-data ground truth for
  the three `survey_basic*` files.
* `generate.py`: the generator of the pyreadstat files and of the PHP writer's input. The PHP writer's
  script is not part of the repository.

### Edge cases in `edge_cases.sav`

* A 64-character variable name, and a second long name that shares its first 8 bytes with it.
* `very_long_text`, width 1200: values of 1200, 1000, 255, 256, 0, 1, 8, 9, 252, 253 and 1199 bytes,
  plus one value of 600 x `õ` (1200 bytes), where a 2-byte character straddles the 255-byte segment
  boundaries. In the ASCII values byte 255 (the last byte of the first segment) is a space that belongs
  to the value, so a reader that trims each segment separately loses it.
* `all_missing`: numeric, every case system-missing.
* `empty_text`: string, every case empty.
* `extreme_numbers` (format E16.8): 9.99e307, -9.99e307, 1e-300, -1e-300, 5e-324 (smallest subnormal
  double), 1e15, -1e15, 2^53 and a few ordinary values. None of them equals a value the format reserves
  (system-missing, LOWEST, HIGHEST).
* `three_missing`: exactly three discrete missing values (97, 98, 99).
* `range_plus_one`: missing range 90 to 99 plus the discrete missing value -1.
* `no_label`: a variable without a variable label.
* `country_code` (A2) and `plan_name` (A10): value labels on a short and on a long string variable.

## Sidecar format

Each `.expected.json` is written from the same in-script definitions that are handed to the writer,
never by reading the data file back.

* File level: `encoding`, `compression` ("none", "bytecode" or "zlib", the header compression code
  0, 1 or 2), `case_count`, `variable_count`, `file_label` (only when the file has one),
  `multiple_response_sets`, `weight_variable`, `variables`, `cases_sample`.
* `variables`, in file order: `name`, `type` ("numeric" or "string"), `width` (strings only, in BYTES
  of UTF-8, not characters), `label` (null when the variable has none), `format` (for example "F8.2",
  "F2.0", "E16.8", "DATE11", "A200"), `measure`, `value_labels` (value as string key, for example
  "1" or "EE"), `missing_values` (discrete values), `missing_range` (`[low, high]` or null),
  `n_system_missing`.
* `cases_sample`: the first 5 and the last 2 cases, each an array in variable order. System-missing is
  null. User-missing values appear as their raw codes (98, 99 and so on).
* `multiple_response_sets`: list of `{name, type ("dichotomy" or "category"), label, counted_value,
  variables}`. `weight_variable`: variable name or null. For the pyreadstat files these are `[]` and
  `null`, which is what the files really contain (see "Gaps").

Notes for test authors:

* Date variables are numeric. Their sample values are the stored numbers: seconds since
  1582-10-14 00:00:00 (13918176000 is 2023-11-01).
* Numbers are written with the shortest text that round-trips to the same IEEE 754 double. For exact
  comparisons the JSON parser must parse floats exactly, otherwise compare with a relative tolerance.
* No string value has trailing spaces, because a .sav file cannot distinguish them from padding.
  An empty string is stored as all spaces.

## Regenerating

From the repo root (uv creates a throwaway environment, nothing is installed into the repo):

```bash
uv run --with pyreadstat==1.3.7 --with pandas==3.1.1 python tests/fixtures/generate.py
uv run --with pyreadstat==1.3.7 --with pandas==3.1.1 python tests/fixtures/generate.py verify
```

`generate.py` uses a fixed random seed, so the data values and all JSON files are identical on every
run. The pyreadstat data files differ between runs only in the creation date and time in the header
(bytes 92 to 108). The 60-byte product text (bytes 4 to 64) of every data file is replaced with a
neutral one of the same length; `generate.py` does that right after pyreadstat writes a file, and the
PHP writer's file was patched the same way. `survey_basic_phpwriter.sav` and its sidecar were written once by a second,
independent pure-PHP writer from `survey_basic_phpwriter.input.json`; run `generate.py verify` after
rewriting it.

Tool versions used:

* pyreadstat 1.3.7 (ReadStat based), pandas 3.1.1, Python 3.13.2, uv 0.7.2
* the pure-PHP writer 3.1.1, commit c4ff108, on PHP 8.5.9

## Verification

Every run of `generate.py` (and `generate.py verify`) reads each data file back with pyreadstat and
compares it with the in-script definitions: file label, encoding, case and variable counts, names,
types, string widths, labels, formats, measure levels, value labels, missing definitions,
system-missing counts, multiple-response sets and ALL cases (not only the sample). It also reads the
header bytes directly for the magic (`$FL2` or `$FL3`), the compression code, the weight index and the
case count, and checks that the `.expected.json` on disk matches the definitions.

The files were additionally read once with the reader of the pure-PHP library, which is independent of ReadStat. It
agreed with every sidecar except for the `very_long_text` values that have a space on byte 255 of a
segment: version 3.1.1 of that reader trims each 255-byte segment and drops that space. The weight variable
designation is confirmed only by the header index and by the PHP reader, because pyreadstat does
not report it.

## Gaps: what is NOT in these files

Limits of the pyreadstat 1.3.7 writer:

* Multiple-response set definitions. `write_sav` has no parameter for them. `q5_1..q5_5` and
  `q6_mention1..3` are ordinary variables in the pyreadstat files. Only `survey_basic_phpwriter.sav`
  defines the sets.
* Weight variable designation. `write_sav` has no parameter for it, so the header weight index is 0
  and `weight` is an ordinary variable. Only `survey_basic_phpwriter.sav` designates it.
* A declared string width. pyreadstat ignores an `A` format and sizes a string variable by its longest
  value in UTF-8 bytes (minimum 1). So `empty_text` in `edge_cases.sav` and `name` in `empty.sav` are
  A1, and no pyreadstat file has a string variable wider than its longest value. The A10 and A200 in
  `survey_basic` are real because a value of exactly that length is present.
* Any encoding other than UTF-8, and big-endian files. `write_sav` has no option for either. There is
  no code-page file (for example windows-1252) in this set.

Limit of the pure-PHP writer 3.1.1:

* For a newly built dataset it writes the weight index as the plain variable number, which is wrong
  when a string variable precedes the weight variable. The PHP script passes the correct
  dictionary position through the public `dictionaryIndex` argument. The library was not modified.

Not attempted, so do not expect them here: user-missing values on string variables, open-ended
missing ranges (LOWEST THRU x, x THRU HIGHEST), document records, variable roles, custom display width
or alignment, variable attributes and data file attributes, variable sets, files written by old writer
versions (no long-name or encoding records), encrypted files and portable (.por) files.
