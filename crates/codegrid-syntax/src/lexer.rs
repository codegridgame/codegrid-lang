use crate::{Diagnostic, Span};

/// A lexical item from a CodeGrid source file.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TokenKind {
    /// A whitespace-delimited directive, cell, or malformed source atom.
    Atom,
    /// A physical LF/CRLF or the zero-width logical terminal newline.
    Newline,
    /// A line or block comment. Comments do not create grid rows or cells.
    Comment,
}

/// A token points into the caller-owned source instead of allocating its text.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn text<'a>(&self, source: &'a str) -> Option<&'a str> {
        source.get(self.span.start..self.span.end)
    }
}

/// Lexical output. Diagnostics are recoverable so an editor can keep using it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Lexed {
    pub tokens: Vec<Token>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Tokenizes UTF-8 source while retaining comments, line endings, and byte spans.
pub fn lex(source: &str) -> Lexed {
    let bytes = source.as_bytes();
    let mut result = Lexed::default();
    let mut cursor = 0;

    while cursor < bytes.len() {
        match bytes[cursor] {
            b' ' | b'\t' => cursor += 1,
            b'\n' => {
                result.tokens.push(Token {
                    kind: TokenKind::Newline,
                    span: Span::new(cursor, cursor + 1),
                });
                cursor += 1;
            }
            b'\r' if bytes.get(cursor + 1) == Some(&b'\n') => {
                result.tokens.push(Token {
                    kind: TokenKind::Newline,
                    span: Span::new(cursor, cursor + 2),
                });
                cursor += 2;
            }
            b'\r' => {
                result.diagnostics.push(Diagnostic::error(
                    "source.unsupported_line_ending",
                    "Bare carriage return is not a supported line ending; use LF or CRLF.",
                    Span::new(cursor, cursor + 1),
                ));
                result.tokens.push(Token {
                    kind: TokenKind::Newline,
                    span: Span::new(cursor, cursor + 1),
                });
                cursor += 1;
            }
            b'/' if bytes.get(cursor + 1) == Some(&b'/') => {
                let start = cursor;
                cursor += 2;
                while cursor < bytes.len() && bytes[cursor] != b'\n' && bytes[cursor] != b'\r' {
                    cursor += 1;
                }
                result.tokens.push(Token {
                    kind: TokenKind::Comment,
                    span: Span::new(start, cursor),
                });
            }
            b'/' if bytes.get(cursor + 1) == Some(&b'*') => {
                let start = cursor;
                cursor += 2;
                let mut closed = false;
                while cursor < bytes.len() {
                    if cursor + 1 < bytes.len()
                        && bytes[cursor] == b'/'
                        && bytes[cursor + 1] == b'*'
                    {
                        result.diagnostics.push(Diagnostic::error(
                            "source.nested_comment",
                            "Block comments cannot nest.",
                            Span::new(cursor, cursor + 2),
                        ));
                        cursor += 2;
                    } else if cursor + 1 < bytes.len()
                        && bytes[cursor] == b'*'
                        && bytes[cursor + 1] == b'/'
                    {
                        cursor += 2;
                        closed = true;
                        break;
                    } else if bytes[cursor] == b'\r' && bytes.get(cursor + 1) != Some(&b'\n') {
                        result.diagnostics.push(Diagnostic::error(
                            "source.unsupported_line_ending",
                            "Bare carriage return is not a supported line ending; use LF or CRLF.",
                            Span::new(cursor, cursor + 1),
                        ));
                        cursor += 1;
                    } else if bytes[cursor] == b'\r' {
                        cursor += 2;
                    } else {
                        cursor += 1;
                    }
                }
                if !closed {
                    cursor = bytes.len();
                    result.diagnostics.push(Diagnostic::error(
                        "source.unterminated_comment",
                        "Unterminated block comment.",
                        Span::new(start, cursor),
                    ));
                }
                result.tokens.push(Token {
                    kind: TokenKind::Comment,
                    span: Span::new(start, cursor),
                });
            }
            _ => {
                let start = cursor;
                while cursor < bytes.len() {
                    if matches!(bytes[cursor], b' ' | b'\t' | b'\n' | b'\r')
                        || (bytes[cursor] == b'/'
                            && matches!(bytes.get(cursor + 1), Some(&b'/') | Some(&b'*')))
                    {
                        break;
                    }
                    cursor += 1;
                }
                result.tokens.push(Token {
                    kind: TokenKind::Atom,
                    span: Span::new(start, cursor),
                });
            }
        }
    }

    if !matches!(result.tokens.last(), Some(token) if token.kind == TokenKind::Newline) {
        result.tokens.push(Token {
            kind: TokenKind::Newline,
            span: Span::empty(source.len()),
        });
    }

    result
}

#[cfg(test)]
mod tests {
    use super::{lex, TokenKind};

    #[test]
    fn tokenizes_atoms_comments_and_crlf_as_distinct_spanned_items() {
        let source = "+ /* note */\t=\r\n// tail\n";
        let lexed = lex(source);

        assert!(lexed.diagnostics.is_empty());
        let items = lexed
            .tokens
            .iter()
            .map(|token| (token.kind, token.text(source).expect("token span is valid")))
            .collect::<Vec<_>>();
        assert_eq!(
            items,
            [
                (TokenKind::Atom, "+"),
                (TokenKind::Comment, "/* note */"),
                (TokenKind::Atom, "="),
                (TokenKind::Newline, "\r\n"),
                (TokenKind::Comment, "// tail"),
                (TokenKind::Newline, "\n"),
            ]
        );
    }

    #[test]
    fn spans_comments_and_atoms_in_utf8_bytes_across_crlf() {
        let source = "// café\n+\r\n";
        let lexed = lex(source);

        assert_eq!(lexed.tokens[0].span.start, 0);
        assert_eq!(lexed.tokens[0].text(source), Some("// café"));
        assert_eq!(lexed.tokens[1].span, super::Span::new(8, 9));
        assert_eq!(lexed.tokens[1].text(source), Some("\n"));
        assert_eq!(lexed.tokens[2].span, super::Span::new(9, 10));
        assert_eq!(lexed.tokens[2].text(source), Some("+"));
        assert_eq!(lexed.tokens[3].span, super::Span::new(10, 12));
        assert_eq!(lexed.tokens[3].text(source), Some("\r\n"));
    }

    #[test]
    fn reports_nested_and_unterminated_block_comments_without_panicking() {
        let nested = lex("/* outer /* inner */ +");
        assert_eq!(nested.diagnostics.len(), 1);
        assert_eq!(nested.diagnostics[0].message, "Block comments cannot nest.");
        assert!(nested
            .tokens
            .iter()
            .any(|token| token.kind == TokenKind::Atom));

        let unterminated = lex("/* unfinished");
        assert_eq!(unterminated.diagnostics.len(), 1);
        assert_eq!(
            unterminated.diagnostics[0].message,
            "Unterminated block comment."
        );
        assert_eq!(unterminated.diagnostics[0].span, super::Span::new(0, 13));
    }

    #[test]
    fn accepts_lf_and_crlf_inside_block_comments() {
        for line_ending in ["\n", "\r\n"] {
            let source = format!("/* first{line_ending}second */");
            let lexed = lex(&source);

            assert!(lexed.diagnostics.is_empty(), "{source:?}");
            assert_eq!(lexed.tokens[0].kind, TokenKind::Comment);
            assert_eq!(lexed.tokens[0].text(&source), Some(source.as_str()));
        }
    }

    #[test]
    fn reports_bare_carriage_return_inside_block_comments_at_its_byte_span() {
        let source = "/* first\rsecond */";
        let lexed = lex(source);
        let carriage_return = source.find('\r').expect("fixture contains a bare CR");

        assert_eq!(lexed.diagnostics.len(), 1);
        assert_eq!(
            lexed.diagnostics[0].message,
            "Bare carriage return is not a supported line ending; use LF or CRLF."
        );
        assert_eq!(
            lexed.diagnostics[0].span,
            super::Span::new(carriage_return, carriage_return + 1)
        );
    }

    #[test]
    fn supplies_an_empty_spanned_logical_newline_only_when_needed() {
        for source in ["", "cell", "cell // café", "cell /* comment\n*/"] {
            let lexed = lex(source);
            let newline = lexed.tokens.last().expect("logical newline must exist");
            assert_eq!(newline.kind, TokenKind::Newline, "{source:?}");
            assert_eq!(newline.span, super::Span::empty(source.len()), "{source:?}");
            assert_eq!(newline.text(source), Some(""), "{source:?}");
        }

        for source in ["cell\n", "cell\r\n"] {
            let lexed = lex(source);
            assert_eq!(
                lexed
                    .tokens
                    .iter()
                    .filter(|token| token.kind == TokenKind::Newline)
                    .count(),
                1,
                "{source:?} must not receive a second logical newline"
            );
            assert_eq!(
                lexed.tokens.last().unwrap().text(source),
                Some(if source.ends_with("\r\n") {
                    "\r\n"
                } else {
                    "\n"
                })
            );
        }
    }
}
