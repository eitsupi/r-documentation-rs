//! Serialization errors.

use rd_ast::RdPath;

/// An error returned when a document cannot be written faithfully.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum WriteError {
    /// The node at `path` has no faithful Rd source representation.
    #[error("{path}: document is not serializable: {kind}")]
    Unserializable {
        /// Location of the offending node.
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
