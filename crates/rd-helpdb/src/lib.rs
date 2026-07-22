//! Reader for installed R package help databases (aliases, `.rdx`/`.rdb`,
//! help-search, vignette, and demo indexes), built on top of `rd-rds`.
//!
//! An installed package directory looks like:
//!
//! ```text
//! <pkg_dir>/help/<pkg>.rdx       # RDS file: index
//! <pkg_dir>/help/<pkg>.rdb       # concatenated binary records
//! <pkg_dir>/help/aliases.rds     # RDS: named character vector alias -> topic
//! <pkg_dir>/Meta/hsearch.rds     # RDS: help-search index
//! <pkg_dir>/Meta/vignette.rds    # optional RDS: vignette data frame
//! <pkg_dir>/Meta/demo.rds        # optional RDS: two-column demo matrix
//! ```
//!
//! `rd-helpdb` does not discover installed packages (R's `.libPaths()`
//! logic is out of scope): [`PackageHelpDb::open`] takes an explicit
//! package directory.

mod db;
pub mod demo;
mod error;
mod index;
mod rds;
mod util;
pub mod vignette;

pub use db::PackageHelpDb;
pub use demo::{DemoEntry, DemoIndex};
pub use error::Error;
pub use rds::{decode_rdb_record, read_rds_file};
pub use vignette::{VignetteEntry, VignetteIndex};
