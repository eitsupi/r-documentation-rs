use super::frame::Mode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RawDelimiter {
    dashes: usize,
    quote: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum RLikeState {
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

impl RLikeState {
    pub(super) fn is_normal(&self) -> bool {
        matches!(
            self,
            Self::Normal {
                raw_prefix: false,
                raw_delimiter: None,
                comment: false,
            }
        )
    }

    pub(super) fn is_transient_opener(&self) -> bool {
        matches!(
            self,
            Self::Normal {
                raw_prefix: true,
                ..
            } | Self::Normal {
                raw_prefix: false,
                raw_delimiter: Some(_),
                ..
            }
        )
    }

    pub(super) fn clear_transient_opener(&mut self) {
        if self.is_transient_opener() {
            let comment = matches!(self, Self::Normal { comment: true, .. });
            *self = Self::Normal {
                raw_prefix: false,
                raw_delimiter: None,
                comment,
            };
        }
    }

    pub(super) fn clear_raw_prefix(&mut self) {
        if let Self::Normal { raw_prefix, .. } = self {
            *raw_prefix = false;
        }
    }

    pub(super) fn is_active(&self) -> bool {
        !self.is_normal()
    }

    pub(super) fn append_to(&mut self, buf: &mut String, value: &str, mode: Mode) {
        if mode != Mode::RLike {
            buf.push_str(value);
            return;
        }
        buf.push_str(value);
        let bytes = value.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'\n' {
                // Raw prefix detection is line-local; delimiter scans and bodies survive.
                self.clear_raw_prefix();
                if let Self::Normal { comment, .. } = self {
                    *comment = false;
                }
                i += 1;
                continue;
            }
            match self {
                Self::RawString { closer, matched } => {
                    if bytes[i] == closer[*matched] {
                        *matched += 1;
                        if *matched == closer.len() {
                            *self = Self::Normal {
                                raw_prefix: false,
                                raw_delimiter: None,
                                comment: false,
                            };
                        }
                    } else {
                        if *matched > 0 {
                            *matched = 0;
                            continue;
                        }
                    }
                }
                Self::OrdinaryQuote { delimiter, escaped } => {
                    if *escaped {
                        *escaped = false;
                    } else if bytes[i] == b'\\' {
                        *escaped = true;
                    } else if bytes[i] == *delimiter {
                        *self = Self::Normal {
                            raw_prefix: false,
                            raw_delimiter: None,
                            comment: false,
                        };
                    }
                }
                Self::Normal {
                    raw_prefix,
                    raw_delimiter,
                    comment,
                } => {
                    if *comment {
                        i += 1;
                        continue;
                    }
                    if bytes[i] == b'#' && raw_delimiter.is_none() {
                        *raw_prefix = false;
                        *comment = true;
                        i += 1;
                        continue;
                    }
                    if let Some(raw_delimiter) = raw_delimiter {
                        if bytes[i] == b'-' {
                            raw_delimiter.dashes += 1;
                            i += 1;
                            continue;
                        }
                        let closing = match bytes[i] {
                            b'(' => b')',
                            b'[' => b']',
                            b'{' => b'}',
                            _ => 0,
                        };
                        if closing != 0 {
                            let mut closer = vec![closing];
                            closer.extend(std::iter::repeat_n(b'-', raw_delimiter.dashes));
                            closer.push(raw_delimiter.quote);
                            *self = Self::RawString { closer, matched: 0 };
                            i += 1;
                            continue;
                        }
                        *self = Self::OrdinaryQuote {
                            delimiter: raw_delimiter.quote,
                            escaped: false,
                        };
                        continue;
                    }
                    if *raw_prefix {
                        if matches!(bytes[i], b'"' | b'\'') {
                            *raw_delimiter = Some(RawDelimiter {
                                dashes: 0,
                                quote: bytes[i],
                            });
                            *raw_prefix = false;
                            i += 1;
                            continue;
                        }
                        *raw_prefix = false;
                    }
                    if bytes[i] == b'r' || bytes[i] == b'R' {
                        *raw_prefix = true;
                    } else if matches!(bytes[i], b'"' | b'\'' | b'`') {
                        *self = Self::OrdinaryQuote {
                            delimiter: bytes[i],
                            escaped: false,
                        };
                    }
                }
            }
            i += 1;
        }
    }
}
