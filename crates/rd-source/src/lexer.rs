use std::ops::Range;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum TokenKind {
    ControlSequence,
    Escape(EscapeKind),
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comment,
    Newline,
    Whitespace,
    Text,
    Backslash,
    IfDef,
    IfNDef,
    EndIf,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum EscapeKind {
    Percent,
    LBrace,
    RBrace,
    Backslash,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Token {
    pub(crate) kind: TokenKind,
    pub(crate) range: Range<usize>,
}

pub(crate) fn lex(input: &[u8]) -> Vec<Token> {
    lex_with_line_start(input, true)
}

pub(crate) fn lex_with_line_start(input: &[u8], first_is_line_start: bool) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut index = 0;

    while index < input.len() {
        let start = index;
        let kind = match input[index] {
            b'#' if (index == 0 && first_is_line_start
                || index > 0 && input[index - 1] == b'\n')
                && directive_kind(input, index).is_some() =>
            {
                let (kind, length) = directive_kind(input, index).unwrap();
                index += length;
                kind
            }
            b'\\' => {
                if index + 1 < input.len() {
                    let escape = match input[index + 1] {
                        b'%' => Some(EscapeKind::Percent),
                        b'{' => Some(EscapeKind::LBrace),
                        b'}' => Some(EscapeKind::RBrace),
                        b'\\' => Some(EscapeKind::Backslash),
                        _ => None,
                    };
                    if let Some(escape) = escape {
                        index += 2;
                        TokenKind::Escape(escape)
                    } else if input[index + 1].is_ascii_alphanumeric() {
                        index += 2;
                        while index < input.len() && input[index].is_ascii_alphanumeric() {
                            index += 1;
                        }
                        TokenKind::ControlSequence
                    } else {
                        index += 1;
                        TokenKind::Backslash
                    }
                } else {
                    index += 1;
                    TokenKind::Backslash
                }
            }
            b'%' => {
                index += 1;
                while index < input.len() && input[index] != b'\n' && input[index] != b'\r' {
                    index += 1;
                }
                TokenKind::Comment
            }
            b'\r' => {
                index += 1;
                if input.get(index) == Some(&b'\n') {
                    index += 1;
                }
                TokenKind::Newline
            }
            b'\n' => {
                index += 1;
                TokenKind::Newline
            }
            b'{' => {
                index += 1;
                TokenKind::LBrace
            }
            b'}' => {
                index += 1;
                TokenKind::RBrace
            }
            b'[' => {
                index += 1;
                TokenKind::LBracket
            }
            b']' => {
                index += 1;
                TokenKind::RBracket
            }
            byte if is_non_newline_whitespace(byte) => {
                index += 1;
                while index < input.len() && is_non_newline_whitespace(input[index]) {
                    index += 1;
                }
                TokenKind::Whitespace
            }
            _ => {
                index += 1;
                while index < input.len() && is_text_byte(input[index]) {
                    index += 1;
                }
                TokenKind::Text
            }
        };

        tokens.push(Token {
            kind,
            range: start..index,
        });
    }

    tokens
}

fn directive_kind(input: &[u8], index: usize) -> Option<(TokenKind, usize)> {
    for (spelling, kind) in [
        (b"#ifdef".as_slice(), TokenKind::IfDef),
        (b"#ifndef".as_slice(), TokenKind::IfNDef),
        (b"#endif".as_slice(), TokenKind::EndIf),
    ] {
        let end = index + spelling.len();
        if input.get(index..end) == Some(spelling)
            && input
                .get(end)
                .is_none_or(|byte| !byte.is_ascii_alphanumeric())
        {
            return Some((kind, spelling.len()));
        }
    }
    None
}

fn is_non_newline_whitespace(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\x0b' | b'\x0c')
}

fn is_text_byte(byte: u8) -> bool {
    !matches!(
        byte,
        b'\\' | b'%' | b'\r' | b'\n' | b'{' | b'}' | b'[' | b']'
    ) && !is_non_newline_whitespace(byte)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    fn assert_lossless(input: &[u8], tokens: &[Token]) {
        if input.is_empty() {
            assert!(tokens.is_empty());
            return;
        }
        assert_eq!(tokens.first().unwrap().range.start, 0);
        assert_eq!(tokens.last().unwrap().range.end, input.len());
        for token in tokens {
            assert!(token.range.start < token.range.end);
            assert!(token.range.end <= input.len());
        }
        for window in tokens.windows(2) {
            assert_eq!(window[0].range.end, window[1].range.start);
        }
        let reconstructed: Vec<u8> = tokens
            .iter()
            .flat_map(|token| &input[token.range.clone()])
            .copied()
            .collect();
        assert_eq!(reconstructed, input);
    }

    fn kinds(input: &[u8]) -> Vec<TokenKind> {
        lex(input).into_iter().map(|token| token.kind).collect()
    }

    fn fixture(name: &str) -> Vec<u8> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/rd")
            .join(name);
        fs::read(path).unwrap()
    }

    fn fixture_tokens(name: &str) -> (Vec<u8>, Vec<Token>) {
        let input = fixture(name);
        let tokens = lex(&input);
        (input, tokens)
    }

    #[test]
    fn corpus_is_lossless() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rd");
        let mut paths: Vec<_> = fs::read_dir(root)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "Rd"))
            .collect();
        paths.sort();
        assert_eq!(paths.len(), 76);
        for path in paths {
            let input = fs::read(path).unwrap();
            let tokens = lex(&input);
            assert_lossless(&input, &tokens);
        }
    }

    #[test]
    fn focused_fixture_kinds() {
        let (_, tokens) = fixture_tokens("minimal.Rd");
        assert!(
            tokens
                .iter()
                .any(|token| token.kind == TokenKind::ControlSequence)
        );
        assert!(tokens.iter().any(|token| token.kind == TokenKind::LBrace));
        assert!(tokens.iter().any(|token| token.kind == TokenKind::RBrace));
        assert!(tokens.iter().any(|token| token.kind == TokenKind::Newline));
        assert!(tokens.iter().any(|token| token.kind == TokenKind::Text));

        for name in ["options.Rd", "sexpr-options.Rd"] {
            let (_, tokens) = fixture_tokens(name);
            assert!(tokens.iter().any(|token| token.kind == TokenKind::LBracket));
            assert!(tokens.iter().any(|token| token.kind == TokenKind::RBracket));
        }

        for name in [
            "comment.Rd",
            "comment-leading.Rd",
            "comment-trailing.Rd",
            "comment-inside-section.Rd",
            "comment-between-sections.Rd",
        ] {
            let (_, tokens) = fixture_tokens(name);
            assert!(
                tokens.iter().any(|token| token.kind == TokenKind::Comment),
                "{name}"
            );
        }

        for name in ["escapes.Rd", "escapes-rcode.Rd", "escapes-verb.Rd"] {
            let (_, tokens) = fixture_tokens(name);
            assert!(
                tokens
                    .iter()
                    .any(|token| matches!(token.kind, TokenKind::Escape(_)),),
                "{name}"
            );
        }

        let (input, tokens) = fixture_tokens("crlf-document.Rd");
        assert!(tokens.iter().any(|token| {
            token.kind == TokenKind::Newline && &input[token.range.clone()] == b"\r\n"
        }));
        assert!(!tokens.iter().any(|token| {
            token.kind == TokenKind::Newline && &input[token.range.clone()] == b"\r"
        }));

        let (input, tokens) = fixture_tokens("unicode.Rd");
        let unicode_text = tokens
            .iter()
            .find(|token| {
                token.kind == TokenKind::Text
                    && input[token.range.clone()].starts_with("Café".as_bytes())
            })
            .unwrap();
        assert_eq!(&input[unicode_text.range.clone()], "Café".as_bytes());
        let japanese_text = tokens
            .iter()
            .find(|token| {
                token.kind == TokenKind::Text
                    && input[token.range.clone()].starts_with("日本語".as_bytes())
            })
            .unwrap();
        assert_eq!(&input[japanese_text.range.clone()], "日本語.".as_bytes());

        let (_, tokens) = fixture_tokens("whitespace-only-input.Rd");
        assert!(
            tokens
                .iter()
                .any(|token| token.kind == TokenKind::Whitespace)
        );
        assert!(tokens.iter().any(|token| token.kind == TokenKind::Newline));

        for name in ["nested-groups.Rd", "empty-groups.Rd"] {
            let (_, tokens) = fixture_tokens(name);
            let left = tokens
                .iter()
                .filter(|token| token.kind == TokenKind::LBrace)
                .count();
            let right = tokens
                .iter()
                .filter(|token| token.kind == TokenKind::RBrace)
                .count();
            assert_eq!(left, right);
            assert!(left >= 4);
        }

        let (_, tokens) = fixture_tokens("usage-examples-token-kinds.Rd");
        assert!(
            !tokens
                .iter()
                .any(|token| matches!(token.kind, TokenKind::Comment))
        );
        assert!(tokens.iter().any(|token| token.kind == TokenKind::Text));
    }

    #[test]
    fn adversarial_inputs_are_lossless_and_unambiguous() {
        let cases: &[(&[u8], &[TokenKind])] = &[
            (b"", &[]),
            (br"\", &[TokenKind::Backslash]),
            (br"\%", &[TokenKind::Escape(EscapeKind::Percent)]),
            (b"%", &[TokenKind::Comment]),
            (b"% no newline", &[TokenKind::Comment]),
            (
                b"% comment\\%still comment\r\nx",
                &[TokenKind::Comment, TokenKind::Newline, TokenKind::Text],
            ),
            (b"{", &[TokenKind::LBrace]),
            (b"}", &[TokenKind::RBrace]),
            (b"[", &[TokenKind::LBracket]),
            (b"]", &[TokenKind::RBracket]),
            (
                br"\link[x]{y}",
                &[
                    TokenKind::ControlSequence,
                    TokenKind::LBracket,
                    TokenKind::Text,
                    TokenKind::RBracket,
                    TokenKind::LBrace,
                    TokenKind::Text,
                    TokenKind::RBrace,
                ],
            ),
            (
                b"[plain]",
                &[TokenKind::LBracket, TokenKind::Text, TokenKind::RBracket],
            ),
            ("é中".as_bytes(), &[TokenKind::Text]),
            (
                &[b'a', 0xff, b'%', b'x'],
                &[TokenKind::Text, TokenKind::Comment],
            ),
            (b"\r", &[TokenKind::Newline]),
            (b"\n", &[TokenKind::Newline]),
            (b"\r\n", &[TokenKind::Newline]),
            (b" \t\x0c", &[TokenKind::Whitespace]),
        ];
        for (input, expected) in cases {
            let tokens = lex(input);
            assert_lossless(input, &tokens);
            assert_eq!(kinds(input), *expected, "input: {input:?}");
        }

        let escape_cases: &[(&[u8], TokenKind, Range<usize>)] = &[
            (br"\%", TokenKind::Escape(EscapeKind::Percent), 0..2),
            (br"\{", TokenKind::Escape(EscapeKind::LBrace), 0..2),
            (br"\}", TokenKind::Escape(EscapeKind::RBrace), 0..2),
            (br"\\", TokenKind::Escape(EscapeKind::Backslash), 0..2),
        ];
        for (input, expected_kind, expected_range) in escape_cases {
            let tokens = lex(input);
            assert_eq!(tokens.len(), 1, "input: {input:?}");
            assert_eq!(tokens[0].kind, *expected_kind, "input: {input:?}");
            assert_eq!(tokens[0].range, expected_range.clone(), "input: {input:?}");
            assert_lossless(input, &tokens);
        }

        let tokens = lex(br"\name");
        assert_eq!(tokens[0].kind, TokenKind::ControlSequence);
        assert_eq!(tokens[0].range, 0..5);
        assert_eq!(&br"\name"[tokens[0].range.clone()], br"\name");
    }
}
