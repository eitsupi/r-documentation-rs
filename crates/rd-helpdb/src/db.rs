//! [`PackageHelpDb`], the main entry point for reading an installed
//! package's compiled help database.

use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::OnceLock,
};

use rd_rds::RObject;

use crate::{
    Error,
    index::{Index, Record},
    rds::{decode_rdb_record, read_rds_file},
    util::rstr_to_string,
};

/// A reader over an installed R package's compiled help database:
/// `help/<pkg>.rdx` + `help/<pkg>.rdb`, plus the sibling `help/aliases.rds`
/// and `Meta/hsearch.rds` files.
///
/// The `.rdx` index is parsed eagerly at [`PackageHelpDb::open`] (it's
/// small); `.rdb` records are read on demand by seeking into the `.rdb`
/// file, since records are independent and there's no need to hold the
/// whole file in memory. `help/aliases.rds` is parsed lazily, on the first
/// call to [`PackageHelpDb::aliases`] or [`PackageHelpDb::resolve_alias`],
/// and the resulting index is cached for the lifetime of this reader -- a
/// failed parse is NOT cached, so a transient failure (a missing or
/// momentarily corrupt file) is retried on the next call rather than
/// poisoning every subsequent lookup. A duplicated alias resolves to its
/// LAST occurrence, matching the last-wins semantics of R's own
/// `list2env()`-based alias loader; [`PackageHelpDb::aliases`] still
/// returns every entry, including duplicates, in on-disk order, since
/// that's what oracle comparisons against R's `readRDS()` expect.
/// `search_index()` re-reads `Meta/hsearch.rds` on every call rather than
/// caching -- it's read infrequently relative to alias/topic lookups, and
/// re-reading keeps the API simple and always up to date.
#[derive(Debug)]
pub struct PackageHelpDb {
    package: String,
    pkg_dir: PathBuf,
    help_dir: PathBuf,
    rdb_path: PathBuf,
    index: Index,
    /// Lazily-built `help/aliases.rds` index, cached after the first
    /// successful parse. See the struct-level docs above for the caching
    /// and duplicate-alias semantics.
    alias_index: OnceLock<AliasIndex>,
}

/// A parsed `help/aliases.rds` index: the alias -> topic pairs in on-disk
/// order (`entries`, duplicates included), plus a `lookup` map from alias
/// to its index into `entries`.
///
/// `lookup` is built by inserting `entries` in file order, so a later
/// duplicate alias overwrites an earlier one -- i.e. it always resolves to
/// the LAST occurrence, matching R's own `list2env()`-based alias loader.
#[derive(Debug)]
struct AliasIndex {
    entries: Vec<(String, String)>,
    lookup: HashMap<String, usize>,
}

impl AliasIndex {
    /// Builds an [`AliasIndex`] from the parsed `aliases.rds` root object,
    /// a named character vector mapping alias -> topic.
    fn from_root(root: &RObject) -> Result<Self, Error> {
        let rd_rds::RValue::Character(values) = &root.value() else {
            return Err(Error::MalformedIndex(
                "aliases.rds root is not a character vector".into(),
            ));
        };
        let names = root.names().ok_or_else(|| {
            Error::MalformedIndex("aliases.rds is missing a names attribute".into())
        })?;
        if names.len() != values.len() {
            return Err(Error::MalformedIndex(format!(
                "aliases.rds has {} names but {} values",
                names.len(),
                values.len()
            )));
        }

        let entries = names
            .iter()
            .zip(values.iter())
            .map(|(alias, topic)| Ok((rstr_to_string(alias)?, rstr_to_string(topic)?)))
            .collect::<Result<Vec<(String, String)>, Error>>()?;

        // Insert in file order so a later duplicate alias overwrites an
        // earlier one's index -- last-wins.
        let mut lookup = HashMap::with_capacity(entries.len());
        for (position, (alias, _)) in entries.iter().enumerate() {
            lookup.insert(alias.clone(), position);
        }

        Ok(Self { entries, lookup })
    }
}

impl PackageHelpDb {
    /// Opens `<pkg_dir>/help/<pkg>.rdx` (+ `.rdb`), where `<pkg>` is the
    /// basename of `pkg_dir`, e.g. `/opt/R/4.6.1/lib/R/library/utils`.
    ///
    /// Library-path discovery (R's `.libPaths()`) is out of scope: callers
    /// must supply the installed package directory explicitly.
    pub fn open(pkg_dir: impl AsRef<Path>) -> Result<Self, Error> {
        let pkg_dir = pkg_dir.as_ref();
        let package = pkg_dir
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                Error::MalformedIndex(format!(
                    "cannot determine a package name from directory {}",
                    pkg_dir.display()
                ))
            })?
            .to_string();

        let help_dir = pkg_dir.join("help");
        let rdx_path = help_dir.join(format!("{package}.rdx"));
        let rdb_path = help_dir.join(format!("{package}.rdb"));

        let rdx_root = read_rds_file(&rdx_path)?;
        let index = Index::from_rdx(&rdx_root)?;

        Ok(Self {
            package,
            pkg_dir: pkg_dir.to_path_buf(),
            help_dir,
            rdb_path,
            index,
            alias_index: OnceLock::new(),
        })
    }

    /// The package name (the basename of the directory passed to
    /// [`PackageHelpDb::open`]).
    pub fn package(&self) -> &str {
        &self.package
    }

    /// Topic names present in the `.rdx` `variables` map, in index order.
    pub fn topics(&self) -> impl Iterator<Item = &str> {
        self.index.topics()
    }

    /// Persistence keys present in the `.rdx` `references` map (e.g.
    /// `"env::0"`), in index order.
    pub fn reference_keys(&self) -> impl Iterator<Item = &str> {
        self.index.reference_keys()
    }

    /// Raw decoded Rd object for `topic`: a `.rdx` `variables` lookup, a
    /// `.rdb` record fetch, zlib decompression, and `rd_rds::parse`.
    pub fn raw_topic(&self, topic: &str) -> Result<RObject, Error> {
        let record = self
            .index
            .variable(topic)
            .ok_or_else(|| Error::UnknownTopic {
                topic: topic.to_string(),
            })?;
        self.fetch_record(record)
    }

    /// Fetches a record via the `.rdx` `references` map (persistence keys
    /// like `"env::0"`). Same mechanics as [`PackageHelpDb::raw_topic`],
    /// different key space.
    ///
    /// `rd-helpdb` does not attempt to resolve `PERSISTSXP` nodes into
    /// environments; this only exposes the raw record a reference key
    /// points at.
    pub fn reference(&self, key: &str) -> Result<RObject, Error> {
        let record = self
            .index
            .reference(key)
            .ok_or_else(|| Error::UnknownReference {
                key: key.to_string(),
            })?;
        self.fetch_record(record)
    }

    /// alias -> topic map from `help/aliases.rds`, in on-disk order,
    /// including duplicate aliases if the file has any (see the
    /// struct-level docs for how [`PackageHelpDb::resolve_alias`] handles
    /// duplicates).
    pub fn aliases(&self) -> Result<Vec<(String, String)>, Error> {
        Ok(self.alias_index()?.entries.clone())
    }

    /// Resolves `alias` to its topic name via `help/aliases.rds`. If
    /// `alias` occurs more than once in the file, resolves to the topic of
    /// its LAST occurrence, matching R's own `list2env()`-based alias
    /// loader.
    pub fn resolve_alias(&self, alias: &str) -> Result<Option<&str>, Error> {
        let index = self.alias_index()?;
        Ok(index
            .lookup
            .get(alias)
            .map(|&position| index.entries[position].1.as_str()))
    }

    /// Returns the cached `help/aliases.rds` index, parsing and caching it
    /// on the first call. Parse failures are not cached: on error, the
    /// `OnceLock` is left unset so the next call retries from scratch.
    fn alias_index(&self) -> Result<&AliasIndex, Error> {
        if let Some(index) = self.alias_index.get() {
            return Ok(index);
        }

        let path = self.help_dir.join("aliases.rds");
        let root = read_rds_file(&path)?;
        let index = AliasIndex::from_root(&root)?;

        // If another thread won the race and set the index first, that's
        // fine -- both builds are equivalent, so just fetch whichever one
        // landed.
        let _ = self.alias_index.set(index);
        Ok(self
            .alias_index
            .get()
            .expect("alias_index was just set above"))
    }

    /// Decoded `Meta/hsearch.rds` as a raw [`RObject`] (no typed model yet).
    pub fn search_index(&self) -> Result<RObject, Error> {
        let path = self.pkg_dir.join("Meta").join("hsearch.rds");
        read_rds_file(&path)
    }

    fn fetch_record(&self, record: Record) -> Result<RObject, Error> {
        let mut file = File::open(&self.rdb_path).map_err(|err| Error::io(&self.rdb_path, err))?;
        file.seek(SeekFrom::Start(record.offset as u64))
            .map_err(|err| Error::io(&self.rdb_path, err))?;

        let mut buf = vec![0u8; record.length];
        file.read_exact(&mut buf)
            .map_err(|err| Error::io(&self.rdb_path, err))?;

        decode_rdb_record(&buf)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    /// `tests/fixtures/data/aliases_vector_dup_v3.rds`: a named character
    /// vector `c(shared = "first-topic", unique = "unique-topic", shared =
    /// "second-topic")`, generated by `tests/fixtures/generate_fixtures.R`
    /// (section 9b) -- `RObject`/`RValue` can't be hand-built outside
    /// `rd-rds` (its `Attributes` constructor is crate-private), so the
    /// duplicate-alias input has to come from a real parsed `.rds` file
    /// rather than a struct literal.
    fn dup_aliases_root() -> RObject {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/data/aliases_vector_dup_v3.rds");
        read_rds_file(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
    }

    #[test]
    fn alias_index_resolves_duplicates_last_wins() {
        let root = dup_aliases_root();
        let index = AliasIndex::from_root(&root).expect("AliasIndex::from_root");

        // `entries` keeps both occurrences of "shared", in on-disk order.
        assert_eq!(
            index.entries,
            vec![
                ("shared".to_string(), "first-topic".to_string()),
                ("unique".to_string(), "unique-topic".to_string()),
                ("shared".to_string(), "second-topic".to_string()),
            ]
        );

        // `lookup` resolves the duplicate to the LAST occurrence.
        let shared_position = *index.lookup.get("shared").expect("'shared' in lookup");
        assert_eq!(index.entries[shared_position].1, "second-topic");
        let unique_position = *index.lookup.get("unique").expect("'unique' in lookup");
        assert_eq!(index.entries[unique_position].1, "unique-topic");
    }
}
