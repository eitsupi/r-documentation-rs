//! Parsing for Sweave-style `\Sexpr` option lists.
//!
//! The option storage/view contract is in the crate's included `CONTRACT.md`.
//!
//! The grammar is a comma-separated list of scalar `key=value` entries. It
//! has no quoting, escaping, or nesting. Parsed pairs are preserved (with
//! surrounding ASCII whitespace removed) while unknown keys, duplicates, and
//! invalid typed values are collected as soft diagnostics. Typed resolution
//! uses the last valid value for each key. This grammar is distinct from the
//! `\link` and `\item` option shapes; those have their own semantic views.

use std::{borrow::Cow, fmt};

use crate::{RdNode, RdNodeKind, RdPath, RdShapeError, RdShapeErrorKind};

/// A parsed, borrowed Rd option list and its non-fatal diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RdOptionList<'a> {
    path: RdPath,
    pairs: Vec<RdOptionPair<'a>>,
    diagnostics: Vec<RdOptionError>,
    _nodes: std::marker::PhantomData<&'a [RdNode]>,
}

impl<'a> RdOptionList<'a> {
    /// Parses plain string leaves as a comma-separated scalar option list.
    ///
    /// R's `parse_Rd()` represents a `\\RdOpts` body as a `VERB` leaf while
    /// `\\Sexpr`'s `Rd_option` content is a `TEXT` leaf. Both are plain string
    /// leaves, so both kinds are concatenated in source order without
    /// normalization.
    ///
    /// `path` is the path of the option-content container supplied by the
    /// caller. A future `\\Sexpr` view passes `base_path.with_option()`; a
    /// future `\\RdOpts` view passes its own tagged-node path whose children
    /// are the option text. Pair positions are reported by `pair_index`, not
    /// by adding path segments.
    pub fn parse(nodes: &'a [RdNode], path: RdPath) -> Result<Self, RdOptionError> {
        let mut text = String::new();
        for (index, node) in nodes.iter().enumerate() {
            match node {
                RdNode::Text(value) | RdNode::Verb(value) => text.push_str(value),
                other => {
                    return Err(RdShapeError::new(
                        path.with_child(index),
                        None,
                        RdShapeErrorKind::UnexpectedContent {
                            actual: RdNodeKind::of(other),
                        },
                    )
                    .into());
                }
            }
        }

        let mut pairs = Vec::new();
        let mut diagnostics = Vec::new();
        if text.is_empty() {
            return Ok(Self {
                path,
                pairs,
                diagnostics,
                _nodes: std::marker::PhantomData,
            });
        }
        for (pair_index, entry) in text.split(',').enumerate() {
            if entry.is_empty() {
                return Err(RdOptionError::malformed(
                    path.clone(),
                    pair_index,
                    entry,
                    RdOptionPairErrorKind::EmptyPair,
                ));
            }
            let Some((raw_key, raw_value)) = entry.split_once('=') else {
                return Err(RdOptionError::malformed(
                    path.clone(),
                    pair_index,
                    entry,
                    RdOptionPairErrorKind::MissingEquals,
                ));
            };
            let key = raw_key.trim_ascii();
            let value = raw_value.trim_ascii();
            let reason = if key.is_empty() {
                Some(RdOptionPairErrorKind::EmptyKey)
            } else if value.is_empty() {
                Some(RdOptionPairErrorKind::EmptyValue)
            } else {
                None
            };
            if let Some(reason) = reason {
                return Err(RdOptionError::malformed(
                    path.clone(),
                    pair_index,
                    entry,
                    reason,
                ));
            }

            let pair = RdOptionPair {
                index: pair_index,
                key: Cow::Owned(key.to_string()),
                value: Cow::Owned(value.to_string()),
            };
            if RdSexprOptionKey::from_name(key).is_none() {
                diagnostics.push(RdOptionError::UnknownKey {
                    path: path.clone(),
                    pair_index,
                    key: key.to_string(),
                });
            }
            if let Some(first_pair_index) = pairs.iter().find(|p| p.key() == key).map(|p| p.index) {
                diagnostics.push(RdOptionError::DuplicateKey {
                    path: path.clone(),
                    pair_index,
                    key: key.to_string(),
                    first_pair_index,
                });
            }
            if let Some(known) = RdSexprOptionKey::from_name(key)
                && decode_value(known, value).is_err()
            {
                diagnostics.push(RdOptionError::InvalidValue {
                    path: path.clone(),
                    pair_index,
                    key: known,
                    value: value.to_string(),
                    expected: known.value_kind(),
                });
            }
            pairs.push(pair);
        }

        Ok(Self {
            path,
            pairs,
            diagnostics,
            _nodes: std::marker::PhantomData,
        })
    }

    /// Returns the base path supplied to the parser.
    pub fn path(&self) -> &RdPath {
        &self.path
    }
    /// Returns all parsed pairs in source order.
    pub fn pairs(&self) -> &[RdOptionPair<'a>] {
        &self.pairs
    }
    /// Returns non-fatal diagnostics, in occurrence order.
    pub fn diagnostics(&self) -> &[RdOptionError] {
        &self.diagnostics
    }
    /// Resolves valid typed values, using the last valid occurrence per key.
    pub fn typed(&self) -> RdSexprOptionOverrides {
        let mut result = RdSexprOptionOverrides::empty();
        for pair in &self.pairs {
            if let Some(key) = pair.known_key()
                && let Ok(value) = decode_value(key, pair.value())
            {
                result.set(key, value);
            }
        }
        result
    }
}

/// One preserved key/value pair from an option list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RdOptionPair<'a> {
    index: usize,
    key: Cow<'a, str>,
    value: Cow<'a, str>,
}

impl<'a> RdOptionPair<'a> {
    /// Returns the zero-based pair index.
    pub fn index(&self) -> usize {
        self.index
    }
    /// Returns the trimmed key spelling.
    pub fn key(&self) -> &str {
        &self.key
    }
    /// Returns the trimmed value spelling.
    pub fn value(&self) -> &str {
        &self.value
    }
    /// Returns the typed vocabulary key, if recognized.
    pub fn known_key(&self) -> Option<RdSexprOptionKey> {
        RdSexprOptionKey::from_name(&self.key)
    }
}

/// The recognized names in a `\Sexpr` option list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RdSexprOptionKey {
    /// Evaluation stage.
    Stage,
    /// Results rendering mode.
    Results,
    /// Echo source code.
    Echo,
    /// Evaluate the expression.
    Eval,
    /// Keep source code.
    KeepSource,
    /// Strip whitespace.
    StripWhite,
    /// Figure width.
    Width,
    /// Figure height.
    Height,
    /// Figure number.
    Fig,
}

impl RdSexprOptionKey {
    /// Converts an exact Rd option name to its vocabulary key.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "stage" => Self::Stage,
            "results" => Self::Results,
            "echo" => Self::Echo,
            "eval" => Self::Eval,
            "keep.source" => Self::KeepSource,
            "strip.white" => Self::StripWhite,
            "width" => Self::Width,
            "height" => Self::Height,
            "fig" => Self::Fig,
            _ => return None,
        })
    }
    /// Returns the canonical Rd spelling of this key.
    pub const fn as_name(self) -> &'static str {
        match self {
            Self::Stage => "stage",
            Self::Results => "results",
            Self::Echo => "echo",
            Self::Eval => "eval",
            Self::KeepSource => "keep.source",
            Self::StripWhite => "strip.white",
            Self::Width => "width",
            Self::Height => "height",
            Self::Fig => "fig",
        }
    }
    fn value_kind(self) -> RdOptionValueKind {
        match self {
            Self::Stage => RdOptionValueKind::Stage,
            Self::Results => RdOptionValueKind::Results,
            Self::Echo | Self::Eval | Self::KeepSource => RdOptionValueKind::Boolean,
            Self::StripWhite => RdOptionValueKind::StripWhite,
            Self::Width | Self::Height | Self::Fig => RdOptionValueKind::Boolean,
        }
    }
}

/// An Rd `stage` value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RdSexprStage {
    /// Build stage.
    Build,
    /// Install stage.
    Install,
    /// Render stage.
    Render,
}
/// An Rd `results` value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RdSexprResults {
    /// Text results.
    Text,
    /// Verbatim results.
    Verbatim,
    /// Rd results.
    Rd,
    /// Hidden results.
    Hide,
}
/// An Rd `strip.white` value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RdStripWhite {
    /// Do not trim whitespace.
    None,
    /// Trim ordinary whitespace.
    Trim,
    /// Trim all whitespace.
    All,
}

/// Typed overrides extracted from an Rd option list.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RdSexprOptionOverrides {
    /// Stage override.
    pub stage: Option<RdSexprStage>,
    /// Results override.
    pub results: Option<RdSexprResults>,
    /// Echo override.
    pub echo: Option<bool>,
    /// Evaluation override.
    pub eval: Option<bool>,
    /// Keep-source override.
    pub keep_source: Option<bool>,
    /// Strip-whitespace override.
    pub strip_white: Option<RdStripWhite>,
}
impl RdSexprOptionOverrides {
    /// Creates an override set with no values.
    pub const fn empty() -> Self {
        Self {
            stage: None,
            results: None,
            echo: None,
            eval: None,
            keep_source: None,
            strip_white: None,
        }
    }
    /// Applies present overrides to effective options.
    pub fn apply_to(self, options: &mut RdEffectiveSexprOptions) {
        if let Some(v) = self.stage {
            options.stage = v;
        }
        if let Some(v) = self.results {
            options.results = v;
        }
        if let Some(v) = self.echo {
            options.echo = v;
        }
        if let Some(v) = self.eval {
            options.eval = v;
        }
        if let Some(v) = self.keep_source {
            options.keep_source = v;
        }
        if let Some(v) = self.strip_white {
            options.strip_white = v;
        }
    }
    fn set(&mut self, key: RdSexprOptionKey, value: DecodedValue) {
        match (key, value) {
            (RdSexprOptionKey::Stage, DecodedValue::Stage(v)) => self.stage = Some(v),
            (RdSexprOptionKey::Results, DecodedValue::Results(v)) => self.results = Some(v),
            (RdSexprOptionKey::Echo, DecodedValue::Boolean(v)) => self.echo = Some(v),
            (RdSexprOptionKey::Eval, DecodedValue::Boolean(v)) => self.eval = Some(v),
            (RdSexprOptionKey::KeepSource, DecodedValue::Boolean(v)) => self.keep_source = Some(v),
            (RdSexprOptionKey::StripWhite, DecodedValue::StripWhite(v)) => {
                self.strip_white = Some(v)
            }
            _ => {}
        }
    }
}

/// Fully resolved options for an Rd `\Sexpr` expression.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RdEffectiveSexprOptions {
    /// Evaluation stage.
    pub stage: RdSexprStage,
    /// Results rendering mode.
    pub results: RdSexprResults,
    /// Echo source code.
    pub echo: bool,
    /// Evaluate the expression.
    pub eval: bool,
    /// Keep source code.
    pub keep_source: bool,
    /// Strip whitespace.
    pub strip_white: RdStripWhite,
}
impl Default for RdEffectiveSexprOptions {
    fn default() -> Self {
        Self {
            stage: RdSexprStage::Install,
            results: RdSexprResults::Text,
            echo: false,
            eval: true,
            keep_source: true,
            strip_white: RdStripWhite::Trim,
        }
    }
}

/// A hard or soft option-list diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RdOptionError {
    /// A node had an invalid shape.
    Shape(RdShapeError),
    /// A scalar pair was malformed.
    MalformedPair {
        path: RdPath,
        pair_index: usize,
        text: String,
        reason: RdOptionPairErrorKind,
    },
    /// The key is not recognized.
    UnknownKey {
        path: RdPath,
        pair_index: usize,
        key: String,
    },
    /// The key occurred previously.
    DuplicateKey {
        path: RdPath,
        pair_index: usize,
        key: String,
        first_pair_index: usize,
    },
    /// A recognized typed value could not be decoded.
    InvalidValue {
        path: RdPath,
        pair_index: usize,
        key: RdSexprOptionKey,
        value: String,
        expected: RdOptionValueKind,
    },
}
impl RdOptionError {
    fn malformed(
        path: RdPath,
        pair_index: usize,
        text: &str,
        reason: RdOptionPairErrorKind,
    ) -> Self {
        Self::MalformedPair {
            path,
            pair_index,
            text: text.to_string(),
            reason,
        }
    }

    /// Returns the diagnostic path.
    pub fn path(&self) -> &RdPath {
        match self {
            Self::Shape(error) => error.path(),
            Self::MalformedPair { path, .. }
            | Self::UnknownKey { path, .. }
            | Self::DuplicateKey { path, .. }
            | Self::InvalidValue { path, .. } => path,
        }
    }
}
impl From<RdShapeError> for RdOptionError {
    fn from(error: RdShapeError) -> Self {
        Self::Shape(error)
    }
}
impl fmt::Display for RdOptionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Shape(e) => e.fmt(f),
            Self::MalformedPair {
                path,
                pair_index,
                reason,
                ..
            } => write!(f, "malformed option pair {pair_index}: {reason} at {path}"),
            Self::UnknownKey {
                path,
                pair_index,
                key,
            } => write!(
                f,
                "unknown option key '{key}' (pair {pair_index}) at {path}"
            ),
            Self::DuplicateKey {
                path,
                pair_index,
                key,
                first_pair_index,
            } => write!(
                f,
                "duplicate option key '{key}' (pair {pair_index}; first pair {first_pair_index}) at {path}"
            ),
            Self::InvalidValue {
                path,
                pair_index,
                key,
                value,
                expected,
            } => write!(
                f,
                "invalid value '{value}' for option {} (pair {pair_index}; expected {expected}) at {path}",
                key.as_name()
            ),
        }
    }
}
impl std::error::Error for RdOptionError {}

/// Reasons a scalar option pair is malformed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RdOptionPairErrorKind {
    /// The entry was empty.
    EmptyPair,
    /// No equals separator was present.
    MissingEquals,
    /// The key was empty.
    EmptyKey,
    /// The value was empty.
    EmptyValue,
}
/// Expected typed value categories for known options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RdOptionValueKind {
    /// A boolean.
    Boolean,
    /// A stage name.
    Stage,
    /// A results name.
    Results,
    /// A strip-white name.
    StripWhite,
}
impl fmt::Display for RdOptionPairErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::EmptyPair => "empty pair",
            Self::MissingEquals => "missing '='",
            Self::EmptyKey => "empty key",
            Self::EmptyValue => "empty value",
        })
    }
}
impl fmt::Display for RdOptionValueKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Boolean => "boolean",
            Self::Stage => "stage",
            Self::Results => "results",
            Self::StripWhite => "strip.white",
        })
    }
}

#[derive(Debug, Clone, Copy)]
enum DecodedValue {
    Boolean(bool),
    Stage(RdSexprStage),
    Results(RdSexprResults),
    StripWhite(RdStripWhite),
}
fn decode_value(key: RdSexprOptionKey, value: &str) -> Result<DecodedValue, ()> {
    match key {
        RdSexprOptionKey::Echo | RdSexprOptionKey::Eval | RdSexprOptionKey::KeepSource => value
            .eq_ignore_ascii_case("true")
            .then_some(DecodedValue::Boolean(true))
            .or_else(|| {
                value
                    .eq_ignore_ascii_case("false")
                    .then_some(DecodedValue::Boolean(false))
            })
            .ok_or(()),
        RdSexprOptionKey::Stage => match value {
            "build" => Ok(DecodedValue::Stage(RdSexprStage::Build)),
            "install" => Ok(DecodedValue::Stage(RdSexprStage::Install)),
            "render" => Ok(DecodedValue::Stage(RdSexprStage::Render)),
            _ => Err(()),
        },
        RdSexprOptionKey::Results => match value {
            "text" => Ok(DecodedValue::Results(RdSexprResults::Text)),
            "verbatim" => Ok(DecodedValue::Results(RdSexprResults::Verbatim)),
            "rd" => Ok(DecodedValue::Results(RdSexprResults::Rd)),
            "hide" => Ok(DecodedValue::Results(RdSexprResults::Hide)),
            _ => Err(()),
        },
        RdSexprOptionKey::StripWhite => {
            if value.eq_ignore_ascii_case("true") {
                Ok(DecodedValue::StripWhite(RdStripWhite::Trim))
            } else if value.eq_ignore_ascii_case("all") {
                Ok(DecodedValue::StripWhite(RdStripWhite::All))
            } else if value.eq_ignore_ascii_case("false") {
                Ok(DecodedValue::StripWhite(RdStripWhite::None))
            } else {
                Err(())
            }
        }
        RdSexprOptionKey::Width | RdSexprOptionKey::Height | RdSexprOptionKey::Fig => {
            Ok(DecodedValue::Boolean(false))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RdPathSegment, RdTag};

    fn path() -> RdPath {
        RdPath::new(vec![RdPathSegment::TopLevel(4)])
    }
    fn text(value: &str) -> RdNode {
        RdNode::Text(value.to_string())
    }

    #[test]
    fn concatenates_text_nodes_and_preserves_pairs() {
        let nodes = [text(" stage = build,echo=true"), text(", results = rd ")];
        let parsed = RdOptionList::parse(&nodes, path()).unwrap();
        assert_eq!(
            parsed.pairs().iter().map(|p| p.index()).collect::<Vec<_>>(),
            [0, 1, 2]
        );
        assert_eq!(parsed.pairs()[0].key(), "stage");
        assert_eq!(parsed.pairs()[0].value(), "build");
        assert_eq!(parsed.typed().stage, Some(RdSexprStage::Build));
        assert_eq!(parsed.typed().echo, Some(true));
        assert_eq!(parsed.typed().results, Some(RdSexprResults::Rd));
    }

    #[test]
    fn accepts_verb_and_mixed_string_leaves() {
        let verb = [RdNode::Verb("stage=build".to_string())];
        assert_eq!(
            RdOptionList::parse(&verb, path()).unwrap().typed().stage,
            Some(RdSexprStage::Build)
        );
        let mixed = [
            RdNode::Text("stage=build,".to_string()),
            RdNode::Verb("results=rd".to_string()),
        ];
        let parsed = RdOptionList::parse(&mixed, path()).unwrap();
        assert_eq!(parsed.typed().stage, Some(RdSexprStage::Build));
        assert_eq!(parsed.typed().results, Some(RdSexprResults::Rd));
    }

    #[test]
    fn malformed_reasons_are_hard_errors() {
        for (input, expected) in [
            ("", RdOptionPairErrorKind::EmptyPair),
            ("noequals", RdOptionPairErrorKind::MissingEquals),
            (" =value", RdOptionPairErrorKind::EmptyKey),
            ("key= ", RdOptionPairErrorKind::EmptyValue),
        ] {
            let nodes = [text(input)];
            let result = RdOptionList::parse(&nodes, path());
            if input.is_empty() {
                assert!(result.is_ok());
            } else {
                assert_eq!(result.as_ref().unwrap_err().path(), &path());
                assert!(
                    matches!(result, Err(RdOptionError::MalformedPair { reason, .. }) if reason == expected)
                );
            }
        }
        let error = RdOptionList::parse(&[text("a=1,,b=2")], path()).unwrap_err();
        assert_eq!(error.path(), &path());
        assert!(matches!(
            error,
            RdOptionError::MalformedPair {
                reason: RdOptionPairErrorKind::EmptyPair,
                ..
            }
        ));
    }

    #[test]
    fn rejects_non_text_at_node_path() {
        let nodes = [RdNode::tagged(RdTag::Title, None, vec![])];
        let error = RdOptionList::parse(&nodes, path()).unwrap_err();
        assert!(
            matches!(error, RdOptionError::Shape(shape) if shape.path() == &path().with_child(0) && matches!(shape.kind(), RdShapeErrorKind::UnexpectedContent { actual: RdNodeKind::Tagged }))
        );
    }

    #[test]
    fn decodes_all_typed_keys_and_case_insensitive_values() {
        let nodes = [text(
            "stage=render,results=verbatim,echo=TrUe,eval=false,keep.source=FALSE,strip.white=TRUE",
        )];
        let typed = RdOptionList::parse(&nodes, path()).unwrap().typed();
        assert_eq!(typed.stage, Some(RdSexprStage::Render));
        assert_eq!(typed.results, Some(RdSexprResults::Verbatim));
        assert_eq!(typed.echo, Some(true));
        assert_eq!(typed.eval, Some(false));
        assert_eq!(typed.keep_source, Some(false));
        assert_eq!(typed.strip_white, Some(RdStripWhite::Trim));
        for value in ["false", "all", "true"] {
            let nodes = [text(&format!("strip.white={value}"))];
            let parsed = RdOptionList::parse(&nodes, path()).unwrap();
            assert_eq!(
                parsed.typed().strip_white,
                Some(match value {
                    "false" => RdStripWhite::None,
                    "all" => RdStripWhite::All,
                    _ => RdStripWhite::Trim,
                })
            );
        }
    }

    #[test]
    fn collects_soft_diagnostics_and_keeps_pairs() {
        let nodes = [text("unknown=x,echo=true,echo=maybe,echo=false")];
        let parsed = RdOptionList::parse(&nodes, path()).unwrap();
        assert_eq!(parsed.pairs().len(), 4);
        assert!(
            parsed
                .diagnostics()
                .iter()
                .any(|e| matches!(e, RdOptionError::UnknownKey { key, .. } if key == "unknown"))
        );
        assert!(
            parsed
                .diagnostics()
                .iter()
                .all(|error| error.path() == &path())
        );
        assert!(parsed.diagnostics().iter().any(|e| matches!(
            e,
            RdOptionError::DuplicateKey {
                first_pair_index: 1,
                pair_index: 2,
                ..
            }
        )));
        assert!(parsed.diagnostics().iter().any(|e| matches!(
            e,
            RdOptionError::InvalidValue {
                pair_index: 2,
                expected: RdOptionValueKind::Boolean,
                ..
            }
        )));
        assert_eq!(parsed.typed().echo, Some(false));
    }

    #[test]
    fn formats_soft_diagnostics_with_pair_indices() {
        let nodes = [text("unknown=x,echo=true,echo=maybe")];
        let parsed = RdOptionList::parse(&nodes, path()).unwrap();
        let messages: Vec<_> = parsed
            .diagnostics()
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(
            messages,
            [
                "unknown option key 'unknown' (pair 0) at top-level[4]",
                "duplicate option key 'echo' (pair 2; first pair 1) at top-level[4]",
                "invalid value 'maybe' for option echo (pair 2; expected boolean) at top-level[4]",
            ]
        );
    }

    #[test]
    fn invalid_duplicate_does_not_erase_last_valid_value() {
        let nodes = [text("stage=build,stage=bad")];
        let parsed = RdOptionList::parse(&nodes, path()).unwrap();
        assert_eq!(parsed.typed().stage, Some(RdSexprStage::Build));
    }

    #[test]
    fn figure_keys_are_known_but_raw_only() {
        let nodes = [text("width=bad,height=also-bad,fig=anything")];
        let parsed = RdOptionList::parse(&nodes, path()).unwrap();
        assert_eq!(
            parsed
                .pairs()
                .iter()
                .map(|p| p.known_key())
                .collect::<Vec<_>>(),
            [
                Some(RdSexprOptionKey::Width),
                Some(RdSexprOptionKey::Height),
                Some(RdSexprOptionKey::Fig)
            ]
        );
        assert_eq!(parsed.typed(), RdSexprOptionOverrides::empty());
        assert!(parsed.diagnostics().is_empty());
    }

    #[test]
    fn empty_inputs_and_defaults_are_valid() {
        assert!(RdOptionList::parse(&[], path()).unwrap().pairs().is_empty());
        assert!(
            RdOptionList::parse(&[text("")], path())
                .unwrap()
                .pairs()
                .is_empty()
        );
        let defaults = RdEffectiveSexprOptions::default();
        assert_eq!(
            defaults,
            RdEffectiveSexprOptions {
                stage: RdSexprStage::Install,
                results: RdSexprResults::Text,
                echo: false,
                eval: true,
                keep_source: true,
                strip_white: RdStripWhite::Trim
            }
        );
    }

    #[test]
    fn option_error_is_displayable_standard_error() {
        let error = RdOptionList::parse(&[text("bad")], path()).unwrap_err();
        assert!(error.to_string().contains("missing '='"));
        let _: Box<dyn std::error::Error> = Box::new(error);
    }
}
