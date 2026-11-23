# Databases

This page is for people who keep survey data in a database. `statscraft db` stores a dataset in SQLite, PostgreSQL, MySQL, MariaDB or Dolt and reads it back, and `statscraft plan` applies recodes and labels to a stored dataset in place. Both follow the OpenStatSpec specification, so other programs that follow it read the same tables.

A quick round trip with the [sample file](getting-started.md#open-the-sample-file):

```
statscraft db export survey.sav --sqlite study.db
statscraft plan -e "RECODE satisfaction (9 = SYSMIS)." --sqlite study.db --name survey --apply
statscraft db import --sqlite study.db --name survey --out back.sav
```

The first line stores the file in a new SQLite database, the second recodes one variable in the database, and the third writes the result to a new .sav file.

## Transformation plans: plan

```
statscraft plan [FILE | - | -e TEXT] (--input-schema FILE | -s SESSION | --sqlite DB --name DATASET [--apply [--actor NAME]])
                [--profile 0.1|0.2|0.3] [--alias NAME]
```

`plan` turns a bounded part of the language into an OpenStatSpec Transformation Plan: one line of canonical JSON with the plan, its SHA-256 `plan_hash` and the `source_hash` of the syntax, or an error with a stable `code`, `line` and `column`.

| Flag | Meaning |
|---|---|
| `--input-schema FILE` | JSON of the form `{"variables": [{"name": "q1", "storage_kind": "numeric"}]}` |
| `-s`, `--session NAME` | take the input schema from a session's dataset |
| `--sqlite DB --name DATASET` | take it from a dataset in an OpenStatSpec SQLite database |
| `--apply` | with `--sqlite`, apply the plan to that dataset in place, in one transaction with one audit row |
| `--actor NAME` | who applies the plan, for the audit row; default `$USER` |
| `--profile` | the frontend profile, `0.3` by default |
| `--alias` | the name of the input dataset in the plan, `input` by default |

Profile 0.1 accepts RECODE, VARIABLE LABELS and VALUE LABELS; 0.2 adds COMPUTE and IF with single values and simple comparisons, FORMATS, VARIABLE LEVEL and EXECUTE; 0.3 adds comments, TO lists, NE and NOT, LOWEST and HIGHEST ranges and ADD VALUE LABELS. Everything else is rejected, even where `run` accepts it. With `schema.json` holding `{"variables": [{"name": "q1", "storage_kind": "numeric"}, {"name": "q2", "storage_kind": "numeric"}]}`:

```
statscraft plan -e "RECODE q1 (1 = 0)." --input-schema schema.json --profile 0.1
statscraft plan -s demo -e "RECODE satisfaction (9 = SYSMIS)."
statscraft plan -e "FREQUENCIES q1." --input-schema schema.json
```

The last one prints `{"error":{...,"message":"FREQUENCIES is not supported by frontend profile 0.3",...},"ok":false,...}`, whose `code` is the profile's code for an unsupported command, and exits with 1.

## Store and read datasets: db

```
statscraft db export survey.sav --sqlite study.db [--name survey]   store a dataset in SQLite
statscraft db export survey.sav --postgres URL                      or in PostgreSQL
statscraft db export survey.sav --mysql URL                         or in MySQL or MariaDB
statscraft db export survey.sav --dolt URL                          or in Dolt
statscraft db export -s work --sqlite study.db --name work          store a session's dataset
statscraft db import --sqlite study.db --name survey --out back.sav read it back into a file
statscraft db import --sqlite study.db --name survey -s work        or into a session
statscraft db capabilities                                          versions and limits per database
statscraft plan job.sps --sqlite study.db --name survey --apply     apply syntax to it in place
```

`db` stores a dataset the way OpenStatSpec's SAV/ZSAV profile 1.0 and its SQL dialect profiles specify.

- One table `data_<name>` holds the cases: `__case_ordinal` (1, 2, 3, ... in case order) and one column per variable, in variable order.
- Numbers are binary64, with system-missing as NULL. Strings are text that is never NULL. User-missing codes are kept as values, and dates as numbers.
- The dictionary goes into the catalog tables: labels, value labels, missing values, formats, measurement level, role, display settings, attributes, documents, weight variable and multiple response sets.
- Variable names that are not plain ASCII get generated column names (`v<position>`); the catalog maps them back.
- The database belongs to OpenStatSpec. `export` sets up the catalog in a new or empty one and refuses one that holds anything else.

| Flag | Meaning |
|---|---|
| `--sqlite FILE`, `--postgres URL`, `--mysql URL`, `--dolt URL` | the database; see [Databases](#supported-databases) |
| `--name` | the dataset name: ASCII letters, digits and `_`. For `export` it defaults to the file or session name |
| `--out FILE` | `import` writes this .sav or .zsav file |
| `-s`, `--session NAME` | `export` reads, or `import` replaces, this session's dataset |
| `--allow-loss` | `import` goes on when the catalog holds items a dataset cannot keep |

Each prints one line of JSON, such as `{"case_count":300,"dataset":"survey","dataset_id":"...","ok":true,"table":"data_survey","variable_count":18}`, or `{"ok":false,"error":{"code":...,"message":...}}` with exit code 1.

```
statscraft db export survey.sav --sqlite study.db
statscraft db export -s demo --sqlite study.db --name work
statscraft plan -e "RECODE satisfaction (9 = SYSMIS)." --sqlite study.db --name survey --apply --actor demo
statscraft db import --sqlite study.db --name survey --out back.sav
statscraft db import --sqlite study.db --name work -s fromdb
```

### Writing and reading

- Before writing, a preflight checks the column limit, identifiers, value sizes and the row size against the profile and the server's settings. SQLite refuses NaN and negative zero, which it cannot keep.
- A failure leaves no table and no dataset row, only the failed operation and its diagnostic.
- The rows go in bulk inside the same transaction:
  - PostgreSQL takes them with `COPY` in the binary format;
  - MySQL and MariaDB in prepared multi-row INSERTs of at most 4 MiB, each within `max_allowed_packet`;
  - Dolt in multi-row INSERTs of literal values of at most 256 KiB, each within `max_allowed_packet`;
  - SQLite through one prepared INSERT per case.
- Reading streams the rows from the server into the dataset instead of holding the whole result first.
- Reading a dataset whose catalog holds items Statscraft cannot keep (dataset attributes, variable sets, write formats other than the print format) fails unless `--allow-loss` is given. The losses are then listed in the output.

### Supported databases

| | SQLite | PostgreSQL | MySQL, MariaDB | Dolt |
|---|---|---|---|---|
| Option | `--sqlite FILE` | `--postgres postgres://user@host:port/db[?schema=NAME]` | `--mysql mysql://user@host:port/db` | `--dolt mysql://user@host:port/db` |
| The catalog lives in | the whole file | schema NAME (default `openstatspec`) | database `db` | database `db`, on the session's branch |
| Server versions | bundled SQLite | 17.x, 18.x, UTF8 encoding | MySQL 8.4.x, 9.7.x; MariaDB 11.4.x, 11.8.x, 12.3.x; 16 KiB InnoDB pages | reads 2.2.x from 2.2.2 on; writes 2.2.2 and 2.2.3 |
| Columns, `__case_ordinal` included | 2,000 | 1,600 | 1,017 | 4,097 |
| Numbers | REAL; NaN and negative zero refused | DOUBLE PRECISION, every bit pattern kept | DOUBLE; NaN, infinities and negative zero refused | DOUBLE; NaN, infinities and negative zero refused |
| Text | TEXT NOT NULL | TEXT NOT NULL; U+0000 refused | TEXT NOT NULL, binary collation, at most 65,535 bytes | LONGTEXT NOT NULL |
| Row | the length limit | a heap tuple of 8,160 bytes (NULLs are free) | the InnoDB record of 8,126 bytes: 8 per number, 41 (MySQL) or 21 (MariaDB) per string; one INSERT within `max_allowed_packet` | a table definition below 65,504 bytes: 8 per number, 20 per string |
| Identifiers | 255 bytes | `max_identifier_length` (63 bytes) | 64 characters | 64 characters |
| New variables in `plan --apply` | yes, in the same transaction | yes, in the same transaction | no: `schema_change_not_atomic` before any change | no: `schema_change_not_atomic` |

MySQL, MariaDB and Dolt commit DDL implicitly. So `export` creates the table first, writes the rows and the catalog in one transaction, and drops the table again if that fails. A plan that would create a variable is refused before anything changes; create the column and its catalog variable in a separate step first.

The product is decided by what the server says about itself, not by the protocol: `--mysql` refuses a Dolt server, and `--dolt` refuses MySQL and MariaDB.

### Dolt

- Statscraft never commits, branches, merges or resets.
- `db export` leaves its changes in the working set for you to commit.
- `db import` and the input schema of `plan` write nothing at all (OpenStatSpec's Database I/O Execution Policy v1, which `db capabilities` declares). Allowed losses are therefore reported in the output instead of in the catalog.
- `plan --apply` needs `--expected-branch` and `--expected-head`. The session must be on that branch at that commit with a clean working set, and the branch must still point at that commit when the apply completes. The audit row records both, and the changes stay uncommitted.
- The session's branch is the database's default branch, or the one named as `db/branch` in the URL.

### Passwords and TLS

A URL without a password takes it from the `STATSCRAFT_DB_PASSWORD` environment variable. Passwords are never printed.

Connections use plain TCP unless the URL asks for TLS. TLS always verifies the server's certificate chain, against the CA file when one is given and otherwise against the bundled Mozilla root certificates. It also checks the host name unless the mode says otherwise. Statscraft never makes an unverified TLS connection, and refuses the modes that fall back to plain TCP (`prefer`, `allow`, `PREFERRED`).

```
statscraft db export survey.sav --postgres "postgres://analyst@db.example.org/study?sslmode=verify-full"
statscraft db export survey.sav --postgres "postgres://analyst@10.0.0.5/study?sslmode=verify-ca&sslrootcert=/etc/ssl/study-ca.pem"
statscraft db export survey.sav --mysql "mysql://analyst@db.example.org/study?ssl-mode=VERIFY_IDENTITY&ssl-ca=/etc/ssl/study-ca.pem"
```

| PostgreSQL `sslmode` | MySQL, MariaDB, Dolt `ssl-mode` | Connection |
|---|---|---|
| not given, `disable` | not given, `DISABLED` | plain TCP |
| `verify-full`, `require` | `VERIFY_IDENTITY`, `REQUIRED` | TLS; the certificate chain and the host name are checked |
| `verify-ca` | `VERIFY_CA` | TLS; the certificate chain is checked, the host name is not |

- `sslrootcert=FILE` (PostgreSQL) or `ssl-ca=FILE` (MySQL, MariaDB, Dolt) names a PEM file of the CA certificates to trust, for a server whose certificate comes from a private CA.
- A CA file without a mode means `verify-full`.
- With a CA file, PostgreSQL trusts only that file; the MySQL client trusts the bundled roots as well.
- `require` and `REQUIRED` always check the certificate chain and the host name.

### Applying a plan in place

`plan --name DATASET` with a database option takes the input schema from that dataset. With `--apply` it then applies the plan to the same table in one transaction: direct UPDATEs, `ALTER TABLE ... ADD COLUMN` for new variables, catalog updates and one audit row in `transformation_apply` (plan and source hashes, actor, operation count). Nothing is copied, and on any error nothing changes. `--actor` names who applies the plan (default `$USER`).
