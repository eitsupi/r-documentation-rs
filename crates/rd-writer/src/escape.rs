//! Inverse lexical escaping for Rd leaves.

use crate::spec::Mode;

#[derive(Debug, Clone, Copy)]
pub(crate) struct RawDelimiter {
    dashes: usize,
    quote: char,
}

#[derive(Debug, Clone)]
pub(crate) enum RLikeState {
    Normal {
        raw_prefix: bool,
        raw_delimiter: Option<RawDelimiter>,
        comment: bool,
    },
    OrdinaryQuote {
        delimiter: char,
        escaped: bool,
    },
    RawString {
        closer: String,
        matched: usize,
    },
}

impl Default for RLikeState {
    fn default() -> Self {
        Self::Normal {
            raw_prefix: false,
            raw_delimiter: None,
            comment: false,
        }
    }
}

impl RLikeState {
    pub(crate) fn closure_compatible(&self) -> bool {
        matches!(self, Self::Normal { .. })
    }

    pub(crate) fn is_ordinary_quote(&self) -> bool {
        matches!(self, Self::OrdinaryQuote { .. })
    }

    pub(crate) fn is_raw_string_or_comment(&self) -> bool {
        matches!(
            self,
            Self::RawString { .. } | Self::Normal { comment: true, .. }
        )
    }

    pub(crate) fn clear_transient_opener(&mut self) {
        if let Self::Normal {
            raw_prefix,
            raw_delimiter,
            ..
        } = self
            && (*raw_prefix || raw_delimiter.is_some())
        {
            *raw_prefix = false;
            *raw_delimiter = None;
        }
    }
}

/// Escape one leaf while advancing the lexical state shared by its frame.
pub(crate) fn escape(input: &str, mode: Mode, state: &mut RLikeState) -> (String, bool, bool) {
    if matches!(mode, Mode::Equation) {
        return (input.to_owned(), false, false);
    }
    let mut out = String::with_capacity(input.len());
    let mut raw_newline = false;
    let mut nonraw_interior_newline = false;
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if mode != Mode::RLike {
            push_escaped(&mut out, ch);
            i += 1;
            continue;
        }
        if ch == '\n' {
            if matches!(state, RLikeState::RawString { .. }) {
                raw_newline = true;
            } else if i + 1 < chars.len() {
                nonraw_interior_newline = true;
            }
            out.push(ch);
            match state {
                RLikeState::Normal {
                    raw_prefix,
                    comment,
                    ..
                } => {
                    *raw_prefix = false;
                    *comment = false;
                }
                RLikeState::RawString { .. } | RLikeState::OrdinaryQuote { .. } => {}
            }
            i += 1;
            continue;
        }
        match state {
            RLikeState::RawString { closer, matched } => {
                out.push(ch);
                let expected = closer.chars().nth(*matched).expect("valid closer progress");
                if ch == expected {
                    *matched += 1;
                    if *matched == closer.chars().count() {
                        *state = RLikeState::Normal {
                            raw_prefix: false,
                            raw_delimiter: None,
                            comment: false,
                        };
                    }
                } else if *matched > 0 {
                    // The current byte was already emitted; retain it as a possible
                    // beginning of an overlapping closer.
                    *matched = usize::from(ch == closer.chars().next().expect("non-empty closer"));
                }
                i += 1;
            }
            RLikeState::OrdinaryQuote { delimiter, escaped } => {
                if *escaped {
                    if ch == '\\' {
                        out.push_str(r"\\");
                    } else if ch == '%' {
                        // R's Rd parser comments at a bare % even inside quoted
                        // strings, so the Rd-escape spelling is required.
                        out.push_str(r"\%");
                    } else {
                        out.push(ch);
                    }
                    *escaped = false;
                } else if ch == '\\' {
                    if chars
                        .get(i + 1)
                        .is_some_and(|next| *next == *delimiter || matches!(next, '{' | '}'))
                    {
                        out.push('\\');
                    } else {
                        out.push_str(r"\\");
                    }
                    *escaped = true;
                } else if ch == '%' {
                    // R's Rd parser comments at a bare % even inside quoted
                    // strings, so the Rd-escape spelling is required.
                    out.push_str(r"\%");
                } else if ch == *delimiter {
                    out.push(ch);
                    *state = RLikeState::Normal {
                        raw_prefix: false,
                        raw_delimiter: None,
                        comment: false,
                    };
                } else {
                    out.push(ch);
                }
                i += 1;
            }
            RLikeState::Normal {
                raw_prefix,
                raw_delimiter,
                comment,
            } => {
                if *comment {
                    push_escaped(&mut out, ch);
                    i += 1;
                    continue;
                }
                if let Some(raw_delimiter) = raw_delimiter {
                    if ch == '-' {
                        out.push(ch);
                        raw_delimiter.dashes += 1;
                        i += 1;
                        continue;
                    }
                    let closing = match ch {
                        '(' => ')',
                        '[' => ']',
                        '{' => '}',
                        _ => '\0',
                    };
                    if closing != '\0' {
                        out.push(ch);
                        let mut closer = String::new();
                        closer.push(closing);
                        closer.push_str(&"-".repeat(raw_delimiter.dashes));
                        closer.push(raw_delimiter.quote);
                        *state = RLikeState::RawString { closer, matched: 0 };
                        i += 1;
                        continue;
                    }
                    *state = RLikeState::OrdinaryQuote {
                        delimiter: raw_delimiter.quote,
                        escaped: false,
                    };
                    continue;
                }
                if *raw_prefix {
                    if matches!(ch, '"' | '\'') {
                        out.push(ch);
                        *raw_prefix = false;
                        *raw_delimiter = Some(RawDelimiter {
                            dashes: 0,
                            quote: ch,
                        });
                        i += 1;
                        continue;
                    }
                    *raw_prefix = false;
                }
                match ch {
                    'r' | 'R' => {
                        out.push(ch);
                        *raw_prefix = true;
                    }
                    '\\' | '%' | '{' | '}' => push_escaped(&mut out, ch),
                    '#' => {
                        out.push(ch);
                        *comment = true;
                    }
                    '\'' | '"' | '\x60' => {
                        out.push(ch);
                        *state = RLikeState::OrdinaryQuote {
                            delimiter: ch,
                            escaped: false,
                        };
                    }
                    _ => out.push(ch),
                }
                i += 1;
            }
        }
    }
    (out, raw_newline, nonraw_interior_newline)
}

fn push_escaped(out: &mut String, ch: char) {
    match ch {
        '\\' => out.push_str(r"\\"),
        '%' => out.push_str(r"\%"),
        '{' => out.push_str(r"\{"),
        '}' => out.push_str(r"\}"),
        _ => out.push(ch),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closure_compatibility_matches_parser_states() {
        assert!(
            RLikeState::Normal {
                raw_prefix: true,
                raw_delimiter: None,
                comment: false
            }
            .closure_compatible()
        );
        assert!(
            RLikeState::Normal {
                raw_prefix: false,
                raw_delimiter: Some(RawDelimiter {
                    dashes: 3,
                    quote: '\'',
                }),
                comment: false
            }
            .closure_compatible()
        );
        assert!(
            RLikeState::Normal {
                raw_prefix: false,
                raw_delimiter: None,
                comment: true
            }
            .closure_compatible()
        );
        assert!(
            !RLikeState::OrdinaryQuote {
                delimiter: '"',
                escaped: false
            }
            .closure_compatible()
        );
        assert!(
            !RLikeState::RawString {
                closer: ")".into(),
                matched: 0
            }
            .closure_compatible()
        );
    }

    #[test]
    fn ordinary_and_raw_escaping() {
        let mut state = RLikeState::default();
        assert_eq!(
            escape(r#""a\"b\{""#, Mode::RLike, &mut state).0,
            r#""a\"b\{""#
        );
        let mut state = RLikeState::default();
        assert_eq!(
            escape(r#"r"---(a%{})---" %%"#, Mode::RLike, &mut state).0,
            r#"r"---(a%{})---" \%\%"#
        );
    }

    #[test]
    fn single_quote_raw_strings_are_opaque() {
        for (input, expected) in [
            (r#"r'(a%{\q})'"#, r#"r'(a%{\q})'"#),
            (r#"r'---(a%{\q})---'"#, r#"r'---(a%{\q})---'"#),
            (r#"R'-[a%{\q}]-'"#, r#"R'-[a%{\q}]-'"#),
            (r#"r'(a)"b)'"#, r#"r'(a)"b)'"#),
            (r#"r"(a)'b)""#, r#"r"(a)'b)""#),
        ] {
            let mut state = RLikeState::default();
            assert_eq!(
                escape(input, Mode::RLike, &mut state).0,
                expected,
                "{input:?}"
            );
        }
    }

    #[test]
    fn backtick_after_raw_prefix_remains_an_ordinary_quote() {
        let mut state = RLikeState::default();
        assert_eq!(escape("r`100%`", Mode::RLike, &mut state).0, r"r`100\%`");
    }

    #[test]
    fn ordinary_quote_backslash_table() {
        let cases = [
            (r#""\a"#, r#""\\a"#),
            (r#""\\a"#, r#""\\\\a"#),
            (r#""\\\a"#, r#""\\\\\\a"#),
            (r#""\\\\a"#, r#""\\\\\\\\a"#),
            (r#""\"x"#, r#""\"x"#),
            (r#""\{x"#, r#""\{x"#),
            (r#""\}x"#, r#""\}x"#),
            (r#""\%x"#, r#""\\\%x"#),
            (r#""\ax"#, r#""\\ax"#),
            (r#""\"#, r#""\\"#),
            (r#""100%"#, r#""100\%"#),
        ];
        for (input, expected) in cases {
            let mut state = RLikeState::default();
            assert_eq!(
                escape(input, Mode::RLike, &mut state).0,
                expected,
                "{input:?}"
            );
        }
    }

    #[test]
    fn ordinary_quote_backslashes_before_markup_are_not_control_sequences() {
        let mut state = RLikeState::default();
        let output = escape(r#""\\value{foo}"#, Mode::RLike, &mut state).0;
        assert_eq!(output, r#""\\\\value{foo}"#);
    }
}
