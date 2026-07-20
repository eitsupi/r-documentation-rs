//! Reader for installed R package help databases (aliases, `.rdx`/`.rdb`,
//! and `Meta/hsearch.rds`), built on top of `rd-rds`.
//!
//! An installed package directory looks like:
//!
//! ```text
//! <pkg_dir>/help/<pkg>.rdx       # RDS file: index
//! <pkg_dir>/help/<pkg>.rdb       # concatenated binary records
//! <pkg_dir>/help/aliases.rds     # RDS: named character vector alias -> topic
//! <pkg_dir>/Meta/hsearch.rds     # RDS: help-search index
//! ```
//!
//! `rd-helpdb` does not discover installed packages (R's `.libPaths()`
//! logic is out of scope): [`PackageHelpDb::open`] takes an explicit
//! package directory.

mod db;
mod error;
mod index;
mod rds;
mod util;

pub use db::PackageHelpDb;
pub use error::Error;
pub use rds::{decode_rdb_record, read_rds_file};
