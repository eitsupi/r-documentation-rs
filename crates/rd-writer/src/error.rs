//! Serialization errors.

use rd_ast::RdPath;

/// An error returned when a document cannot be written faithfully.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum WriteError {
    /// The AST value at `path` has no faithful Rd source representation.
    #[error("{path}: document is not serializable: {kind}")]
    Unserializable {
        /// Canonical location of the serialization error in the input AST.
        path: RdPath,
        /// Reason serialization is impossible.
        kind: UnserializableKind,
    },
    /// The destination rejected output.
    #[error("failed to write Rd output")]
    Io {
        #[source]
        source: std::io::Error,
    },
    /// The writer's output failed its parser round-trip safety check.
    #[error("writer verification failed: {reason}")]
    Verification { reason: String },
}

impl WriteError {
    /// Returns the canonical location of this error in the input AST.
    ///
    /// Returns `Some` for [`WriteError::Unserializable`] and `None` for errors
    /// that are not associated with an AST location. Top-level nodes use
    /// [`rd_ast::RdPathSegment::TopLevel`]; tagged and group children use
    /// [`rd_ast::RdPathSegment::Child`], and option contents use
    /// [`rd_ast::RdPathSegment::Option`] followed by `Child`.
    pub fn ast_path(&self) -> Option<&rd_ast::RdPath> {
        match self {
            Self::Unserializable { path, .. } => Some(path),
            Self::Io { .. } | Self::Verification { .. } => None,
        }
    }
}

/// Reasons a canonical AST value cannot be represented as Rd source.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum UnserializableKind {
    #[error("raw node")]
    RawNode,
    #[error("unsupported node")]
    UnsupportedNode,
    #[error("invalid tag spelling: {spelling}")]
    InvalidTagSpelling { spelling: String },
    #[error("invalid tag shape: {tag}")]
    InvalidTagShape { tag: String },
    #[error("unknown tag")]
    UnknownTag,
    #[error("tag is not allowed in this parser context")]
    TagNotAllowedInContext,
    #[error("bare group is not representable here")]
    BareGroup,
    #[error("LIST is not representable in document context")]
    ListInDocument,
    #[error("LIST is only representable inside a Latex argument")]
    ListOutsideLatex,
    #[error("R-like lexical state cannot be closed")]
    UnterminatedRLikeState,
    #[error("unexpected node kind")]
    UnexpectedNodeKind,
    #[error("invalid comment")]
    InvalidComment,
    #[error("invalid option content")]
    InvalidOptionContent,
    #[error("unrepresentable leaf")]
    UnrepresentableLeaf,
    #[error("unrepresentable leaf boundary")]
    UnrepresentableLeafBoundary,
    #[error("unrepresentable equation")]
    UnrepresentableEquation,
    #[error("conditional is not at line start")]
    ConditionalNotAtLineStart,
}
