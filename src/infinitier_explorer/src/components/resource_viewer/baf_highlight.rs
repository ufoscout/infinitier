//! BAF script tokenizer for `BcsViewer`.
//!
//! Port of the egui-side tokenizer (lives in the egui explorer at
//! `components/resource_viewer/baf_highlight.rs`). The tokenizer
//! itself is GUI-free — same byte-driven scanner with the IF/THEN/END
//! state machine that picks **trigger** vs **action** colouring for
//! identifiers in call position. The gpui-specific bit is
//! [`highlight_ranges`], which turns the token list into the
//! `(Range<usize>, HighlightStyle)` tuples that `StyledText` consumes.

use std::ops::Range;

use gpui::{HighlightStyle, Hsla, rgb};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Keyword,
    Trigger,
    Action,
    Identifier,
    String,
    Number,
    Comment,
    Punctuation,
    Whitespace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub start: usize,
    pub end: usize,
}

/// Per-kind colour palette. Cheap to rebuild per-frame; we pick light
/// vs dark inside `palette_for` based on `cx.theme().mode`.
#[derive(Clone, Copy)]
pub struct BafPalette {
    pub keyword: Hsla,
    pub trigger: Hsla,
    pub action: Hsla,
    pub identifier: Hsla,
    pub string: Hsla,
    pub number: Hsla,
    pub comment: Hsla,
    pub punctuation: Hsla,
}

impl BafPalette {
    /// Dark palette — colour values lifted verbatim from the egui
    /// `BafTheme::dark`, so the two viewers paint scripts identically.
    pub fn dark() -> Self {
        Self {
            keyword: rgb(0xD79BC4).into(),
            trigger: rgb(0xE5C07B).into(),
            action: rgb(0x7EC7CA).into(),
            identifier: rgb(0x9CDCFE).into(),
            string: rgb(0xCE9178).into(),
            number: rgb(0xB5CEA8).into(),
            comment: rgb(0x6A9955).into(),
            punctuation: rgb(0xD4D4D4).into(),
        }
    }

    /// Light palette — same colours as the egui `BafTheme::light`.
    pub fn light() -> Self {
        Self {
            keyword: rgb(0x7F007F).into(),
            trigger: rgb(0xB05500).into(),
            action: rgb(0x0B6E99).into(),
            identifier: rgb(0x001080).into(),
            string: rgb(0xA31515).into(),
            number: rgb(0x098658).into(),
            comment: rgb(0x55824B).into(),
            punctuation: rgb(0x1F1F1F).into(),
        }
    }

    fn color_for(&self, kind: TokenKind) -> Option<Hsla> {
        match kind {
            TokenKind::Keyword => Some(self.keyword),
            TokenKind::Trigger => Some(self.trigger),
            TokenKind::Action => Some(self.action),
            TokenKind::Identifier => Some(self.identifier),
            TokenKind::String => Some(self.string),
            TokenKind::Number => Some(self.number),
            TokenKind::Comment => Some(self.comment),
            TokenKind::Punctuation => Some(self.punctuation),
            // Whitespace inherits the surrounding text style — no
            // highlight needed.
            TokenKind::Whitespace => None,
        }
    }
}

/// Tokenize `text` and turn each non-whitespace token into a
/// `(byte_range, HighlightStyle)` tuple. The byte ranges are valid
/// for `StyledText::with_highlights` because the tokenizer only ever
/// emits boundaries that the source string itself contains.
pub fn highlight_ranges(text: &str, palette: &BafPalette) -> Vec<(Range<usize>, HighlightStyle)> {
    tokenize(text)
        .into_iter()
        .filter_map(|t| {
            palette.color_for(t.kind).map(|c| {
                (
                    t.start..t.end,
                    HighlightStyle {
                        color: Some(c),
                        ..Default::default()
                    },
                )
            })
        })
        .collect()
}

/// Which BAF block we are currently inside. Drives the
/// trigger-vs-action colouring decision: a function-call identifier
/// gets the trigger or action kind depending on this state.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Outside,
    Condition,
    Response,
}

/// Tokenize a BAF script. Whitespace is preserved as `Whitespace`
/// tokens so that joining the slices reproduces the original source.
pub fn tokenize(text: &str) -> Vec<Token> {
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;
    let mut section = Section::Outside;

    while i < bytes.len() {
        let start = i;
        let b = bytes[i];
        let token = match b {
            b' ' | b'\t' | b'\n' | b'\r' => {
                while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\n' | b'\r') {
                    i += 1;
                }
                Token {
                    kind: TokenKind::Whitespace,
                    start,
                    end: i,
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                Token {
                    kind: TokenKind::Comment,
                    start,
                    end: i,
                }
            }
            b'"' => {
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' && bytes[i] != b'\n' {
                    i += 1;
                }
                if i < bytes.len() && bytes[i] == b'"' {
                    i += 1;
                }
                Token {
                    kind: TokenKind::String,
                    start,
                    end: i,
                }
            }
            b'-' if matches!(bytes.get(i + 1), Some(b'0'..=b'9')) => {
                i += 2;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                Token {
                    kind: TokenKind::Number,
                    start,
                    end: i,
                }
            }
            b'0'..=b'9' => {
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                Token {
                    kind: TokenKind::Number,
                    start,
                    end: i,
                }
            }
            b if is_ident_start(b) => {
                while i < bytes.len() && is_ident_cont(bytes[i]) {
                    i += 1;
                }
                let word = &text[start..i];
                let kind = match word {
                    "IF" => {
                        section = Section::Condition;
                        TokenKind::Keyword
                    }
                    "THEN" => {
                        section = Section::Response;
                        TokenKind::Keyword
                    }
                    "END" => {
                        section = Section::Outside;
                        TokenKind::Keyword
                    }
                    "RESPONSE" | "OR" => TokenKind::Keyword,
                    _ => {
                        let mut j = i;
                        while j < bytes.len() && matches!(bytes[j], b' ' | b'\t') {
                            j += 1;
                        }
                        let is_call = j < bytes.len() && bytes[j] == b'(';
                        if is_call {
                            match section {
                                Section::Condition => TokenKind::Trigger,
                                Section::Response => TokenKind::Action,
                                Section::Outside => TokenKind::Identifier,
                            }
                        } else {
                            TokenKind::Identifier
                        }
                    }
                };
                Token {
                    kind,
                    start,
                    end: i,
                }
            }
            _ => {
                i += 1;
                Token {
                    kind: TokenKind::Punctuation,
                    start,
                    end: i,
                }
            }
        };
        tokens.push(token);
    }

    tokens
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

fn is_ident_cont(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds_and_slices(text: &str) -> Vec<(TokenKind, &str)> {
        tokenize(text)
            .into_iter()
            .map(|t| (t.kind, &text[t.start..t.end]))
            .collect()
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert!(tokenize("").is_empty());
    }

    #[test]
    fn keywords_recognised() {
        let toks: Vec<TokenKind> = kinds_and_slices("IF THEN RESPONSE END OR")
            .into_iter()
            .filter(|(k, _)| *k != TokenKind::Whitespace)
            .map(|(k, _)| k)
            .collect();
        assert_eq!(toks, vec![TokenKind::Keyword; 5]);
    }

    #[test]
    fn function_call_in_condition_is_trigger() {
        let toks = kinds_and_slices("IF\n    HPPercentLT(Myself,50)\n");
        let hp = toks.iter().find(|(_, s)| *s == "HPPercentLT").unwrap();
        assert_eq!(hp.0, TokenKind::Trigger);
    }

    #[test]
    fn function_call_in_response_is_action() {
        let toks = kinds_and_slices("IF\nTHEN\n    Wait(3)\nEND\n");
        let wait = toks.iter().find(|(_, s)| *s == "Wait").unwrap();
        assert_eq!(wait.0, TokenKind::Action);
    }

    #[test]
    fn round_trip_preserves_source() {
        let src = "IF\n    HPPercentLT(Myself,50)\nTHEN\n    Wait(3)\nEND\n";
        let rebuilt: String = tokenize(src)
            .into_iter()
            .map(|t| &src[t.start..t.end])
            .collect();
        assert_eq!(rebuilt, src);
    }
}
