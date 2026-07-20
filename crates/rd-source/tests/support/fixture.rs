use std::{
    fs,
    path::{Path, PathBuf},
};

use rd_ast::{RdDocument, lower_r_object};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SourceStatus {
    Valid,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Comparison {
    #[default]
    OracleParity,
    IntentionalDivergence,
    SourceOnly,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StringPair {
    pub kind: String,
    pub value: String,
}
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Sequence {
    pub path: String,
    pub kinds: Vec<String>,
}
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub path: String,
    pub children: usize,
}
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Arity {
    pub tag: String,
    pub counts: Vec<usize>,
}
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct OptionExpectation {
    pub tag: String,
    pub presence: String,
}
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct OptionNode {
    pub kind: String,
    pub value: Option<String>,
}
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct OptionNodes {
    pub tag: String,
    pub nodes: Vec<OptionNode>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObligationSet {
    pub labels: Vec<String>,
    pub comments: usize,
    pub comment_paths: Option<Vec<String>>,
    pub comment_values: Option<Vec<String>>,
    pub expected_sequence: Option<Vec<Sequence>>,
    pub expected_leaves: Option<Vec<StringPair>>,
    pub expected_leaf_kinds: Option<Vec<String>>,
    pub expected_decoded: Option<Vec<StringPair>>,
    pub expected_groups: Option<Vec<Group>>,
    pub expected_root_nodes: Option<i64>,
    pub expected_arities: Option<Vec<Arity>>,
    pub expected_options: Option<Vec<OptionExpectation>>,
    pub expected_option_nodes: Option<Vec<OptionNodes>>,
    pub required_tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy)]
pub struct ObligationView<'a> {
    pub labels: &'a [String],
    pub comments: usize,
    pub comment_paths: Option<&'a [String]>,
    pub comment_values: Option<&'a [String]>,
    pub expected_sequence: Option<&'a [Sequence]>,
    pub expected_leaves: Option<&'a [StringPair]>,
    pub expected_leaf_kinds: Option<&'a [String]>,
    pub expected_decoded: Option<&'a [StringPair]>,
    pub expected_groups: Option<&'a [Group]>,
    pub expected_root_nodes: Option<i64>,
    pub expected_arities: Option<&'a [Arity]>,
    pub expected_options: Option<&'a [OptionExpectation]>,
    pub expected_option_nodes: Option<&'a [OptionNodes]>,
    pub required_tags: Option<&'a [String]>,
}

impl ObligationSet {
    fn view(&self) -> ObligationView<'_> {
        ObligationView {
            labels: &self.labels,
            comments: self.comments,
            comment_paths: self.comment_paths.as_deref(),
            comment_values: self.comment_values.as_deref(),
            expected_sequence: self.expected_sequence.as_deref(),
            expected_leaves: self.expected_leaves.as_deref(),
            expected_leaf_kinds: self.expected_leaf_kinds.as_deref(),
            expected_decoded: self.expected_decoded.as_deref(),
            expected_groups: self.expected_groups.as_deref(),
            expected_root_nodes: self.expected_root_nodes,
            expected_arities: self.expected_arities.as_deref(),
            expected_options: self.expected_options.as_deref(),
            expected_option_nodes: self.expected_option_nodes.as_deref(),
            required_tags: self.required_tags.as_deref(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub struct Case {
    pub name: String,
    pub category: String,
    pub rd: String,
    pub oracle: Option<String>,
    pub phase: String,
    pub source_status: Option<SourceStatus>,
    #[serde(default)]
    pub comparison: Comparison,
    pub divergence_reason: Option<String>,
    pub oracle_obligations: Option<ObligationSet>,
    pub source_obligations: Option<ObligationSet>,
    pub obligations: Vec<String>,
    pub comments: usize,
    pub comment_paths: Option<Vec<String>>,
    pub comment_values: Option<Vec<String>>,
    pub expected_sequence: Option<Vec<Sequence>>,
    pub expected_leaves: Option<Vec<StringPair>>,
    pub expected_leaf_kinds: Option<Vec<String>>,
    pub expected_decoded: Option<Vec<StringPair>>,
    pub expected_groups: Option<Vec<Group>>,
    pub expected_root_nodes: Option<i64>,
    pub expected_arities: Option<Vec<Arity>>,
    pub expected_options: Option<Vec<OptionExpectation>>,
    pub expected_option_nodes: Option<Vec<OptionNodes>>,
    pub required_tags: Option<Vec<String>>,
}

impl Case {
    pub fn flat_obligations(&self) -> ObligationView<'_> {
        ObligationView {
            labels: &self.obligations,
            comments: self.comments,
            comment_paths: self.comment_paths.as_deref(),
            comment_values: self.comment_values.as_deref(),
            expected_sequence: self.expected_sequence.as_deref(),
            expected_leaves: self.expected_leaves.as_deref(),
            expected_leaf_kinds: self.expected_leaf_kinds.as_deref(),
            expected_decoded: self.expected_decoded.as_deref(),
            expected_groups: self.expected_groups.as_deref(),
            expected_root_nodes: self.expected_root_nodes,
            expected_arities: self.expected_arities.as_deref(),
            expected_options: self.expected_options.as_deref(),
            expected_option_nodes: self.expected_option_nodes.as_deref(),
            required_tags: self.required_tags.as_deref(),
        }
    }

    pub fn oracle_obligations(&self) -> ObligationView<'_> {
        self.oracle_obligations
            .as_ref()
            .unwrap_or_else(|| panic!("{}: oracle obligations are missing", self.name))
            .view()
    }

    pub fn source_obligations(&self) -> ObligationView<'_> {
        self.source_obligations
            .as_ref()
            .unwrap_or_else(|| panic!("{}: source obligations are missing", self.name))
            .view()
    }

    pub fn is_source_only(&self) -> bool {
        self.comparison == Comparison::SourceOnly
    }
}

pub fn load_cases(root: &Path) -> Result<Vec<Case>, String> {
    let text = fs::read_to_string(root.join("cases.toml")).map_err(|e| e.to_string())?;
    parse_cases(&text)
}

pub fn parse_cases(text: &str) -> Result<Vec<Case>, String> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Manifest {
        case: Vec<Case>,
    }
    toml::from_str::<Manifest>(text)
        .map(|manifest| manifest.case)
        .map_err(|e| format!("invalid cases.toml: {e}"))
}

pub fn load_oracle(root: &Path, case: &Case) -> Result<RdDocument, String> {
    let path = root.join(
        case.oracle
            .as_ref()
            .ok_or_else(|| format!("{} has no oracle", case.name))?,
    );
    let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", case.name))?;
    let object =
        rd_rds::parse(&bytes).map_err(|e| format!("{}: RDS parse failed: {e}", case.name))?;
    lower_r_object(&object).map_err(|e| format!("{}: lowering failed: {e}", case.name))
}

pub fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}
