//! `statscraft db`: datasets in an OpenStatSpec database (one wide table plus catalog) in
//! SQLite, PostgreSQL, MySQL or MariaDB.

use crate::{load_session, session_file};
use clap::{Args, Subcommand};
use serde_json::{Value, json};
use statscraft_data::Dataset;
use statscraft_openstatspec::canonical_json;
use statscraft_openstatspec::catalog;
use statscraft_openstatspec::db::{Db, DbError};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Where the password comes from when a URL has none.
pub const PASSWORD_VARIABLE: &str = "STATSCRAFT_DB_PASSWORD";

/// The database: exactly one of these.
#[derive(Args)]
pub struct Target {
    /// OpenStatSpec SQLite database file
    #[arg(long, value_name = "DB")]
    pub sqlite: Option<PathBuf>,
    /// PostgreSQL: postgres://user@host:port/database[?schema=NAME], schema default openstatspec
    #[arg(long, value_name = "URL", conflicts_with = "sqlite")]
    pub postgres: Option<String>,
    /// MySQL or MariaDB: mysql://user@host:port/database, a database dedicated to the catalog
    #[arg(long, value_name = "URL", conflicts_with_all = ["sqlite", "postgres"])]
    pub mysql: Option<String>,
}

impl Target {
    pub fn given(&self) -> bool {
        self.sqlite.is_some() || self.postgres.is_some() || self.mysql.is_some()
    }

    /// Opens the database; `create` installs the catalog into an empty one. A URL without a
    /// password takes it from STATSCRAFT_DB_PASSWORD.
    pub fn open(&self, create: bool) -> Result<Box<dyn Db>, DbError> {
        let password = std::env::var(PASSWORD_VARIABLE).ok();
        let password = password.as_deref();
        use statscraft_openstatspec::{mysql, postgres, sqlite};
        Ok(match (&self.sqlite, &self.postgres, &self.mysql) {
            (Some(path), _, _) => Box::new(sqlite::open(path, create)?),
            (_, Some(url), _) => Box::new(postgres::open(url, password, create)?),
            (_, _, Some(url)) => Box::new(mysql::open(url, password, create)?),
            _ => {
                return Err(DbError::new(
                    "usage",
                    "give --sqlite DB, --postgres URL or --mysql URL",
                ));
            }
        })
    }
}

#[derive(Args)]
#[command(after_help = "\
Prints one line of canonical JSON: {\"ok\":true,...} or {\"ok\":false,\"error\":{\"code\":...,\"message\":...}}.

The database is dedicated to OpenStatSpec: a SQLite file, a PostgreSQL schema or a MySQL or \
MariaDB database. Export \
creates the catalog in a new or empty one and refuses one holding anything else. Each dataset is one table data_<name> with __case_ordinal and one column per \
variable; the dictionary lives in the catalog tables. A URL without a password takes it from \
the STATSCRAFT_DB_PASSWORD environment variable; passwords are never printed.")]
pub struct DbArgs {
    #[command(subcommand)]
    command: DbCmd,
}

#[derive(Subcommand)]
enum DbCmd {
    /// Store a .sav/.zsav file, or the dataset of a session, as a dataset in the database
    Export {
        /// A .sav or .zsav file
        file: Option<PathBuf>,
        /// Export the dataset of this session instead of a file
        #[arg(short, long, conflicts_with = "file")]
        session: Option<String>,
        #[command(flatten)]
        target: Target,
        /// Dataset name: ASCII letters, digits and _; defaults to the file or session name
        #[arg(long)]
        name: Option<String>,
    },
    /// Read a dataset from the database into a .sav/.zsav file or a session
    Import {
        #[command(flatten)]
        target: Target,
        /// Dataset name
        #[arg(long)]
        name: String,
        /// Write this .sav or .zsav file
        #[arg(long, value_name = "FILE", required_unless_present = "session")]
        out: Option<PathBuf>,
        /// Make the dataset the one of this session instead
        #[arg(short, long, conflicts_with = "out")]
        session: Option<String>,
        /// Go on when the catalog holds items the dataset cannot keep, and record each loss
        #[arg(long)]
        allow_loss: bool,
    },
}

pub fn run(args: DbArgs) -> ExitCode {
    let output = match execute(args.command) {
        Ok(ok) => ok,
        Err(e) => json!({"ok": false, "error": {"code": e.code, "message": e.message}}),
    };
    println!(
        "{}",
        canonical_json(&output).expect("plain data with small integers")
    );
    if output["ok"] == true {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn usage(message: String) -> DbError {
    DbError::new("usage", message)
}

fn first_failure(failures: Vec<crate::Failure>) -> DbError {
    let message = failures
        .into_iter()
        .next()
        .map_or_else(String::new, |f| f.message);
    DbError::new("session", message)
}

fn execute(command: DbCmd) -> Result<Value, DbError> {
    match command {
        DbCmd::Export {
            file,
            session,
            target,
            name,
        } => {
            let (dataset, default_name) = match (&file, &session) {
                (Some(file), None) => (
                    Dataset::read_sav(file).map_err(|e| DbError::new("file", e.to_string()))?,
                    stem(file),
                ),
                (None, Some(session)) => (
                    load_session(session)
                        .map_err(first_failure)?
                        .ok_or_else(|| usage(format!("session {session} has no dataset yet")))?,
                    session.replace('-', "_"),
                ),
                _ => return Err(usage("give a .sav/.zsav file or --session NAME".into())),
            };
            let name = name.unwrap_or(default_name);
            let mut db = target.open(true)?;
            let id = catalog::write_dataset(db.as_mut(), &dataset, &name)?;
            Ok(json!({
                "ok": true,
                "dataset": name,
                "dataset_id": id,
                "table": format!("data_{name}"),
                "case_count": dataset.case_count(),
                "variable_count": dataset.variable_count(),
            }))
        }
        DbCmd::Import {
            target,
            name,
            out,
            session,
            allow_loss,
        } => {
            let mut db = target.open(false)?;
            let (dataset, losses) = catalog::read_dataset(db.as_mut(), &name, allow_loss)?;
            let target = match (out, &session) {
                (Some(out), None) => out,
                (None, Some(session)) => {
                    let path = session_file(session).map_err(first_failure)?;
                    std::fs::create_dir_all(path.parent().expect("session path has a parent"))
                        .map_err(|e| DbError::new("session", e.to_string()))?;
                    path
                }
                _ => return Err(usage("give --out FILE or --session NAME".into())),
            };
            dataset
                .write_sav(&target)
                .map_err(|e| DbError::new("file", e.to_string()))?;
            let mut ok = json!({
                "ok": true,
                "dataset": name,
                "case_count": dataset.case_count(),
                "variable_count": dataset.variable_count(),
            });
            if !losses.is_empty() {
                ok["losses"] = json!(losses);
            }
            match session {
                Some(session) => ok["session"] = json!(session),
                None => ok["file"] = json!(target.display().to_string()),
            }
            Ok(ok)
        }
    }
}

/// The file name without extension, with characters a dataset name cannot hold as `_`.
fn stem(file: &Path) -> String {
    file.file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}
