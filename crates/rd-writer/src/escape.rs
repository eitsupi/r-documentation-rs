//! Inverse lexical escaping for Rd leaves.

use crate::spec::Mode;
use rd_source::unstable_rlike::{ConsumedIn, State, Unit};

pub(crate) type RLikeState = State;

pub(crate) fn is_raw_string_or_comment(state: &RLikeState) -> bool {
    state.is_raw_string() || state.is_comment()
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
        let consumed_in = state.step(if ch.is_ascii() {
            Unit::Ascii(ch as u8)
        } else {
            Unit::Other
        });
        if ch == '\n' {
            if matches!(consumed_in, ConsumedIn::RawString) {
                raw_newline = true;
            } else if i + 1 < chars.len() {
                nonraw_interior_newline = true;
            }
            out.push(ch);
            i += 1;
            continue;
        }
        match consumed_in {
            ConsumedIn::RawString => {
                out.push(ch);
            }
            ConsumedIn::OrdinaryQuote {
                delimiter,
                escaped_before,
            } => {
                if escaped_before {
                    if ch == '\\' {
                        out.push_str(r"\\");
                    } else if ch == '%' {
                        // R's Rd parser comments at a bare % even inside quoted
                        // strings, so the Rd-escape spelling is required.
                        out.push_str(r"\%");
                    } else {
                        out.push(ch);
                    }
                } else if ch == '\\' {
                    if chars.get(i + 1).is_some_and(|next| {
                        *next == char::from(delimiter) || matches!(next, '{' | '}')
                    }) {
                        out.push('\\');
                    } else {
                        out.push_str(r"\\");
                    }
                } else if ch == '%' {
                    // R's Rd parser comments at a bare % even inside quoted
                    // strings, so the Rd-escape spelling is required.
                    out.push_str(r"\%");
                } else {
                    out.push(ch);
                }
            }
            ConsumedIn::Comment => push_escaped(&mut out, ch),
            ConsumedIn::Normal => match ch {
                '\\' | '%' | '{' | '}' => push_escaped(&mut out, ch),
                _ => out.push(ch),
            },
        }
        i += 1;
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
        assert!(RLikeState::default().closure_compatible());

        let mut state = RLikeState::default();
        state.step(Unit::Ascii(b'r'));
        assert!(state.closure_compatible());
        state.step(Unit::Ascii(b'"'));
        assert!(state.closure_compatible());

        let mut state = RLikeState::default();
        state.step(Unit::Ascii(b'#'));
        assert!(state.closure_compatible());

        let mut state = RLikeState::default();
        state.step(Unit::Ascii(b'"'));
        assert!(!state.closure_compatible());

        let mut state = RLikeState::default();
        for byte in br#"r"("# {
            state.step(Unit::Ascii(*byte));
        }
        assert!(!state.closure_compatible());
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
