use super::frame::Mode;

#[derive(Clone, Copy)]
pub(super) enum OptionPolicy {
    Forbidden,
    Optional { mode: Mode },
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Context {
    Document,
    Latex,
    RLike,
}
pub(super) struct ArgumentSpec {
    pub(super) mode: Mode,
    pub(super) required: bool,
}
pub(super) struct TagSpec {
    pub(super) option_policy: OptionPolicy,
    pub(super) arguments: &'static [ArgumentSpec],
    pub(super) allowed_contexts: &'static [Context],
    /// True for macros that open a top-level document section (WRE §2.1);
    /// these are synchronization points for unclosed options owned at
    /// document level (CONTRACT §6 rules 7 and 11).
    pub(super) section: bool,
}

/// Known `l`/`v` macros are the only structured markup recognized in an
/// ordinary quoted R-like string.
pub(super) fn recognized_in_ordinary_quote(name: &str) -> bool {
    name.as_bytes()
        .get(1)
        .is_some_and(|first| matches!(first, b'l' | b'v'))
        && tag_spec(name, Context::Document).is_some()
}

const NAME_ARGS: &[ArgumentSpec] = &[ArgumentSpec {
    mode: Mode::Verbatim,
    required: true,
}];
const LATEX_ARGS: &[ArgumentSpec] = &[ArgumentSpec {
    mode: Mode::Latex,
    required: true,
}];
const RLIKE_ARGS: &[ArgumentSpec] = &[ArgumentSpec {
    mode: Mode::RLike,
    required: true,
}];
const VERBATIM_ARGS: &[ArgumentSpec] = &[ArgumentSpec {
    mode: Mode::Verbatim,
    required: true,
}];
const VERBATIM_OPTIONAL_ARGS: &[ArgumentSpec] = &[
    ArgumentSpec {
        mode: Mode::Verbatim,
        required: true,
    },
    ArgumentSpec {
        mode: Mode::Verbatim,
        required: false,
    },
];
const HREF_ARGS: &[ArgumentSpec] = &[
    ArgumentSpec {
        mode: Mode::Verbatim,
        required: true,
    },
    ArgumentSpec {
        mode: Mode::Latex,
        required: true,
    },
];
const EQUATION_ARGS: &[ArgumentSpec] = &[
    ArgumentSpec {
        mode: Mode::Equation,
        required: true,
    },
    ArgumentSpec {
        mode: Mode::Verbatim,
        required: false,
    },
];
const EMPTY_ARGS: &[ArgumentSpec] = &[];
const METHOD_ARGS: &[ArgumentSpec] = &[
    ArgumentSpec {
        mode: Mode::Latex,
        required: true,
    },
    ArgumentSpec {
        mode: Mode::Latex,
        required: true,
    },
];
const IFELSE_ARGS: &[ArgumentSpec] = &[
    ArgumentSpec {
        mode: Mode::Latex,
        required: true,
    },
    ArgumentSpec {
        mode: Mode::Latex,
        required: true,
    },
    ArgumentSpec {
        mode: Mode::Latex,
        required: true,
    },
];
const ITEM_ARGS: &[ArgumentSpec] = &[
    ArgumentSpec {
        mode: Mode::Latex,
        required: true,
    },
    ArgumentSpec {
        mode: Mode::Latex,
        required: true,
    },
];
const SECTION_ARGS: &[ArgumentSpec] = METHOD_ARGS;
const DOCUMENT_CONTEXT: &[Context] = &[Context::Document];
const LATEX_CONTEXTS: &[Context] = &[Context::Document, Context::Latex];
const TEXT_CONTEXTS: &[Context] = &[Context::Document, Context::Latex, Context::RLike];
const ANY_CONTEXTS: &[Context] = &[Context::Document, Context::Latex, Context::RLike];
const UNKNOWN_ARGS: &[ArgumentSpec] = &[ArgumentSpec {
    mode: Mode::Latex,
    required: false,
}];

pub(super) fn item_arguments() -> &'static [ArgumentSpec] {
    ITEM_ARGS
}

pub(super) fn unknown_arguments() -> &'static [ArgumentSpec] {
    UNKNOWN_ARGS
}

pub(super) fn tag_spec(name: &str, context: Context) -> Option<TagSpec> {
    match name {
        r"\name" | r"\alias" | r"\Rdversion" | r"\synopsis" => Some(TagSpec {
            option_policy: OptionPolicy::Forbidden,
            arguments: if name == r"\name" {
                NAME_ARGS
            } else {
                VERBATIM_ARGS
            },
            allowed_contexts: DOCUMENT_CONTEXT,
            section: true,
        }),
        r"\encoding" | r"\docType" | r"\keyword" | r"\concept" => Some(TagSpec {
            option_policy: OptionPolicy::Forbidden,
            arguments: LATEX_ARGS,
            allowed_contexts: DOCUMENT_CONTEXT,
            section: true,
        }),
        r"\title" | r"\description" | r"\value" | r"\details" | r"\note" | r"\author"
        | r"\references" | r"\seealso" | r"\format" | r"\source" => Some(TagSpec {
            option_policy: OptionPolicy::Forbidden,
            arguments: LATEX_ARGS,
            allowed_contexts: LATEX_CONTEXTS,
            section: true,
        }),
        r"\emph" | r"\strong" | r"\bold" | r"\pkg" | r"\file" | r"\email" | r"\var" | r"\doi" => {
            Some(TagSpec {
                option_policy: OptionPolicy::Forbidden,
                arguments: LATEX_ARGS,
                allowed_contexts: TEXT_CONTEXTS,
                section: false,
            })
        }
        r"\CRANpkg" => Some(TagSpec {
            option_policy: OptionPolicy::Forbidden,
            arguments: LATEX_ARGS,
            allowed_contexts: &[Context::Latex, Context::RLike],
            section: false,
        }),
        r"\sspace" => Some(TagSpec {
            option_policy: OptionPolicy::Forbidden,
            arguments: EMPTY_ARGS,
            allowed_contexts: &[Context::Latex, Context::RLike],
            section: false,
        }),
        r"\I" => Some(TagSpec {
            option_policy: OptionPolicy::Forbidden,
            arguments: if context == Context::RLike {
                RLIKE_ARGS
            } else {
                LATEX_ARGS
            },
            allowed_contexts: ANY_CONTEXTS,
            section: false,
        }),
        r"\code" => Some(TagSpec {
            option_policy: OptionPolicy::Forbidden,
            arguments: RLIKE_ARGS,
            allowed_contexts: ANY_CONTEXTS,
            section: false,
        }),
        r"\special" => Some(TagSpec {
            option_policy: OptionPolicy::Forbidden,
            arguments: RLIKE_ARGS,
            allowed_contexts: &[Context::Latex, Context::RLike],
            section: false,
        }),
        r"\url" => Some(TagSpec {
            option_policy: OptionPolicy::Forbidden,
            arguments: VERBATIM_ARGS,
            allowed_contexts: ANY_CONTEXTS,
            section: false,
        }),
        r"\href" => Some(TagSpec {
            option_policy: OptionPolicy::Forbidden,
            arguments: HREF_ARGS,
            allowed_contexts: ANY_CONTEXTS,
            section: false,
        }),
        r"\link" => Some(TagSpec {
            option_policy: OptionPolicy::Optional { mode: Mode::Latex },
            arguments: LATEX_ARGS,
            // The usage corpus contains links in R-like signatures; keep this
            // permissive until a focused oracle probe establishes a narrower rule.
            allowed_contexts: ANY_CONTEXTS,
            section: false,
        }),
        r"\linkS4class" => Some(TagSpec {
            option_policy: OptionPolicy::Optional { mode: Mode::Latex },
            arguments: LATEX_ARGS,
            // Keep this permissive pending focused probes outside the normal
            // description context; R accepts this inline tag in both modes.
            allowed_contexts: ANY_CONTEXTS,
            section: false,
        }),
        r"\samp" | r"\kbd" | r"\env" | r"\option" | r"\out" => Some(TagSpec {
            option_policy: OptionPolicy::Forbidden,
            arguments: VERBATIM_ARGS,
            allowed_contexts: ANY_CONTEXTS,
            section: false,
        }),
        r"\command" | r"\sQuote" | r"\dQuote" | r"\acronym" | r"\abbr" | r"\cite" | r"\dfn" => {
            Some(TagSpec {
                option_policy: OptionPolicy::Forbidden,
                arguments: LATEX_ARGS,
                allowed_contexts: ANY_CONTEXTS,
                section: false,
            })
        }
        r"\figure" => Some(TagSpec {
            option_policy: OptionPolicy::Forbidden,
            arguments: if context == Context::RLike {
                VERBATIM_ARGS
            } else {
                VERBATIM_OPTIONAL_ARGS
            },
            allowed_contexts: ANY_CONTEXTS,
            section: false,
        }),
        r"\enc" | r"\if" => Some(TagSpec {
            option_policy: OptionPolicy::Forbidden,
            arguments: METHOD_ARGS,
            allowed_contexts: ANY_CONTEXTS,
            section: false,
        }),
        r"\ifelse" => Some(TagSpec {
            option_policy: OptionPolicy::Forbidden,
            arguments: IFELSE_ARGS,
            allowed_contexts: ANY_CONTEXTS,
            section: false,
        }),
        r"\Sexpr" => Some(TagSpec {
            option_policy: OptionPolicy::Optional { mode: Mode::Latex },
            arguments: RLIKE_ARGS,
            allowed_contexts: &[Context::Document, Context::Latex, Context::RLike],
            section: false,
        }),
        r"\RdOpts" => Some(TagSpec {
            option_policy: OptionPolicy::Forbidden,
            arguments: VERBATIM_ARGS,
            allowed_contexts: DOCUMENT_CONTEXT,
            section: true,
        }),
        r"\section" => Some(TagSpec {
            option_policy: OptionPolicy::Forbidden,
            arguments: SECTION_ARGS,
            allowed_contexts: &[Context::Document, Context::Latex],
            section: true,
        }),
        r"\subsection" => Some(TagSpec {
            option_policy: OptionPolicy::Forbidden,
            arguments: SECTION_ARGS,
            allowed_contexts: &[Context::Document, Context::Latex],
            section: false,
        }),
        r"\usage" => Some(TagSpec {
            option_policy: OptionPolicy::Forbidden,
            arguments: RLIKE_ARGS,
            allowed_contexts: &[Context::Document, Context::RLike, Context::Latex],
            section: true,
        }),
        r"\examples" => Some(TagSpec {
            option_policy: OptionPolicy::Forbidden,
            arguments: RLIKE_ARGS,
            allowed_contexts: &[Context::Document, Context::Latex, Context::RLike],
            section: true,
        }),
        r"\donttest" | r"\dontshow" | r"\dontdiff" | r"\testonly" => Some(TagSpec {
            option_policy: OptionPolicy::Forbidden,
            arguments: RLIKE_ARGS,
            allowed_contexts: &[Context::Document, Context::Latex, Context::RLike],
            section: false,
        }),
        r"\dontrun" => Some(TagSpec {
            option_policy: OptionPolicy::Forbidden,
            arguments: VERBATIM_ARGS,
            allowed_contexts: &[Context::Document, Context::Latex, Context::RLike],
            section: false,
        }),
        r"\eqn" | r"\deqn" => Some(TagSpec {
            option_policy: OptionPolicy::Forbidden,
            arguments: EQUATION_ARGS,
            allowed_contexts: &[Context::Document, Context::Latex, Context::RLike],
            section: false,
        }),
        r"\S3method" | r"\S4method" => Some(TagSpec {
            option_policy: OptionPolicy::Forbidden,
            arguments: METHOD_ARGS,
            allowed_contexts: &[Context::RLike, Context::Latex, Context::Document],
            section: false,
        }),
        r"\verb" | r"\preformatted" => Some(TagSpec {
            option_policy: OptionPolicy::Forbidden,
            arguments: VERBATIM_ARGS,
            allowed_contexts: &[Context::Document, Context::Latex, Context::RLike],
            section: false,
        }),
        r"\method" => Some(TagSpec {
            option_policy: OptionPolicy::Forbidden,
            arguments: METHOD_ARGS,
            allowed_contexts: &[Context::RLike, Context::Latex, Context::Document],
            section: false,
        }),
        r"\tabular" => Some(TagSpec {
            option_policy: OptionPolicy::Forbidden,
            arguments: METHOD_ARGS,
            allowed_contexts: &[Context::Document, Context::Latex],
            section: false,
        }),
        r"\tab" | r"\cr" => Some(TagSpec {
            option_policy: OptionPolicy::Forbidden,
            arguments: EMPTY_ARGS,
            allowed_contexts: &[Context::Document, Context::Latex],
            section: false,
        }),
        r"\R" | r"\dots" | r"\ldots" => Some(TagSpec {
            option_policy: OptionPolicy::Forbidden,
            arguments: EMPTY_ARGS,
            allowed_contexts: ANY_CONTEXTS,
            section: false,
        }),
        r"\arguments" => Some(TagSpec {
            option_policy: OptionPolicy::Forbidden,
            arguments: LATEX_ARGS,
            allowed_contexts: LATEX_CONTEXTS,
            section: true,
        }),
        r"\describe" | r"\itemize" | r"\enumerate" => Some(TagSpec {
            option_policy: OptionPolicy::Forbidden,
            arguments: LATEX_ARGS,
            allowed_contexts: TEXT_CONTEXTS,
            section: false,
        }),
        r"\item" => Some(TagSpec {
            option_policy: OptionPolicy::Forbidden,
            arguments: EMPTY_ARGS,
            allowed_contexts: LATEX_CONTEXTS,
            section: false,
        }),
        _ => None,
    }
}
