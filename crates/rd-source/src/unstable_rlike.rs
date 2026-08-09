//! Implementation-sharing surface for the lockstep workspace crates.
//!
//! This module is not a stable public API. It shares the R-like lexical
//! transition engine with `rd-writer`; `crates/rd-source/CONTRACT.md` remains
//! the normative specification of parser behaviour.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Ascii(u8),
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsumedIn {
    Normal,
    Comment,
    OrdinaryQuote { delimiter: u8, escaped_before: bool },
    RawString,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RawDelimiter {
    dashes: usize,
    quote: u8,
}

#[derive(Clone, Debug, Default)]
pub struct State {
    state: LexicalState,
}

#[derive(Clone, Debug)]
enum LexicalState {
    Normal {
        raw_prefix: bool,
        raw_delimiter: Option<RawDelimiter>,
        comment: bool,
    },
    OrdinaryQuote {
        delimiter: u8,
        escaped: bool,
    },
    RawString {
        closer: Vec<u8>,
        matched: usize,
    },
}

impl Default for LexicalState {
    fn default() -> Self {
        Self::Normal {
            raw_prefix: false,
            raw_delimiter: None,
            comment: false,
        }
    }
}

impl State {
    /// Consume one ASCII-oriented input unit and return its effective context.
    pub fn step(&mut self, unit: Unit) -> ConsumedIn {
        loop {
            match &mut self.state {
                LexicalState::RawString { closer, matched } => {
                    let current = ascii(unit);
                    if current == Some(closer[*matched]) {
                        *matched += 1;
                        if *matched == closer.len() {
                            self.state = LexicalState::default();
                        }
                    } else {
                        // The current unit was already consumed by the raw
                        // string. It may also start an overlapping closer.
                        *matched = usize::from(current == Some(closer[0]));
                    }
                    return ConsumedIn::RawString;
                }
                LexicalState::OrdinaryQuote { delimiter, escaped } => {
                    let delimiter_value = *delimiter;
                    let escaped_before = *escaped;
                    if *escaped {
                        *escaped = false;
                    } else if ascii(unit) == Some(b'\\') {
                        *escaped = true;
                    } else if ascii(unit) == Some(*delimiter) {
                        self.state = LexicalState::default();
                    }
                    return ConsumedIn::OrdinaryQuote {
                        delimiter: delimiter_value,
                        escaped_before,
                    };
                }
                LexicalState::Normal {
                    raw_prefix,
                    raw_delimiter,
                    comment,
                } => {
                    let current = ascii(unit);
                    if current == Some(b'\n') {
                        let consumed_in = if *comment {
                            ConsumedIn::Comment
                        } else {
                            ConsumedIn::Normal
                        };
                        // Prefix detection is line-local. A delimiter scan is
                        // allowed to continue across a newline.
                        *raw_prefix = false;
                        *comment = false;
                        return consumed_in;
                    }
                    if *comment {
                        return ConsumedIn::Comment;
                    }
                    if current == Some(b'#') && raw_delimiter.is_none() {
                        *raw_prefix = false;
                        *comment = true;
                        return ConsumedIn::Comment;
                    }
                    if let Some(delimiter) = raw_delimiter {
                        if current == Some(b'-') {
                            delimiter.dashes += 1;
                            return ConsumedIn::Normal;
                        }
                        if let Some(closing) = raw_closing(current) {
                            let mut closer = Vec::with_capacity(delimiter.dashes + 2);
                            closer.push(closing);
                            closer.extend(std::iter::repeat_n(b'-', delimiter.dashes));
                            closer.push(delimiter.quote);
                            self.state = LexicalState::RawString { closer, matched: 0 };
                            return ConsumedIn::RawString;
                        }
                        self.state = LexicalState::OrdinaryQuote {
                            delimiter: delimiter.quote,
                            escaped: false,
                        };
                        continue;
                    }
                    if *raw_prefix {
                        if matches!(current, Some(b'"' | b'\'')) {
                            *raw_delimiter = Some(RawDelimiter {
                                dashes: 0,
                                quote: current.expect("matched quote"),
                            });
                            *raw_prefix = false;
                            return ConsumedIn::Normal;
                        }
                        *raw_prefix = false;
                    }
                    if matches!(current, Some(b'r' | b'R')) {
                        *raw_prefix = true;
                    } else if matches!(current, Some(b'"' | b'\'' | b'`')) {
                        self.state = LexicalState::OrdinaryQuote {
                            delimiter: current.expect("matched quote"),
                            escaped: false,
                        };
                    }
                    return ConsumedIn::Normal;
                }
            }
        }
    }

    pub fn is_normal(&self) -> bool {
        matches!(
            self.state,
            LexicalState::Normal {
                raw_prefix: false,
                raw_delimiter: None,
                comment: false,
            }
        )
    }

    pub fn is_transient_opener(&self) -> bool {
        matches!(
            self.state,
            LexicalState::Normal {
                raw_prefix: true,
                ..
            } | LexicalState::Normal {
                raw_delimiter: Some(_),
                ..
            }
        )
    }

    pub fn is_ordinary_quote(&self) -> bool {
        matches!(self.state, LexicalState::OrdinaryQuote { .. })
    }

    pub fn is_raw_string(&self) -> bool {
        matches!(self.state, LexicalState::RawString { .. })
    }

    pub fn is_comment(&self) -> bool {
        matches!(self.state, LexicalState::Normal { comment: true, .. })
    }

    pub fn is_raw_delimiter(&self) -> bool {
        matches!(
            self.state,
            LexicalState::Normal {
                raw_delimiter: Some(_),
                ..
            }
        )
    }

    pub fn closure_compatible(&self) -> bool {
        matches!(self.state, LexicalState::Normal { .. })
    }

    pub fn clear_transient_opener(&mut self) {
        if let LexicalState::Normal {
            raw_prefix,
            raw_delimiter,
            ..
        } = &mut self.state
            && (*raw_prefix || raw_delimiter.is_some())
        {
            *raw_prefix = false;
            *raw_delimiter = None;
        }
    }

    pub fn clear_raw_prefix(&mut self) {
        if let LexicalState::Normal { raw_prefix, .. } = &mut self.state {
            *raw_prefix = false;
        }
    }
}

fn ascii(unit: Unit) -> Option<u8> {
    match unit {
        Unit::Ascii(byte) => Some(byte),
        Unit::Other => None,
    }
}

fn raw_closing(byte: Option<u8>) -> Option<u8> {
    match byte {
        Some(b'(') => Some(b')'),
        Some(b'[') => Some(b']'),
        Some(b'{') => Some(b'}'),
        _ => None,
    }
}
