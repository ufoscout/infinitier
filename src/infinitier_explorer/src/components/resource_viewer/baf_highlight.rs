//! BAF script syntax highlighting for `BcsViewer`.
//!
//! Split into two layers so the egui-free tokenizer stays unit-testable:
//!
//! - [`tokenizer`] consumes `&str` and yields [`Token`]s with byte spans
//!   and a [`TokenKind`]. No external deps.
//! - [`layout_baf`] turns those tokens into an [`egui::text::LayoutJob`]
//!   by mapping each kind to a [`TextFormat`] from the chosen [`BafTheme`].
//!
//! The tokenizer is context-aware: identifiers followed by `(` get
//! coloured as **triggers** while inside `IF … THEN`, as **actions**
//! between `THEN … END`, and fall back to the plain "identifier" colour
//! everywhere else. That mirrors NearInfinity's BCS editor enough to be
//! useful without needing TRIGGER.IDS / ACTION.IDS lookups in the
//! highlighter itself.

use eframe::egui::{
    self, Color32, FontId, Galley,
    text::{LayoutJob, TextFormat},
};
use std::sync::Arc;

pub use tokenizer::TokenKind;

/// Raw colour palette used by [`BafTheme::dark`] / [`BafTheme::light`].
/// Keeps the constructors short and readable — they only differ in
/// their colour values.
struct Palette {
    keyword: Color32,
    trigger: Color32,
    action: Color32,
    identifier: Color32,
    string: Color32,
    number: Color32,
    comment: Color32,
    punctuation: Color32,
    default: Color32,
}

/// Per-kind formatting used when laying out BAF text.
#[derive(Clone)]
pub struct BafTheme {
    pub keyword: TextFormat,
    pub trigger: TextFormat,
    pub action: TextFormat,
    pub identifier: TextFormat,
    pub string: TextFormat,
    pub number: TextFormat,
    pub comment: TextFormat,
    pub punctuation: TextFormat,
    pub default: TextFormat,
}

impl BafTheme {
    /// Pick the appropriate palette for the current egui visuals so the
    /// BAF colours stay readable in both dark and light modes. Cheap to
    /// rebuild per-frame — the work is a handful of `TextFormat` clones.
    pub fn for_visuals(visuals: &egui::Visuals) -> Self {
        if visuals.dark_mode {
            Self::dark()
        } else {
            Self::light()
        }
    }

    /// Dark-theme defaults. Tuned for egui's standard dark visuals:
    /// - structural keywords get a single accent colour,
    /// - triggers and actions get distinct hues so the IF/RESPONSE flow
    ///   is visually obvious,
    /// - strings, numbers, comments use their conventional palette.
    pub fn dark() -> Self {
        Self::from_palette(&Palette {
            keyword: Color32::from_rgb(0xD7, 0x9B, 0xC4),
            trigger: Color32::from_rgb(0xE5, 0xC0, 0x7B),
            action: Color32::from_rgb(0x7E, 0xC7, 0xCA),
            identifier: Color32::from_rgb(0x9C, 0xDC, 0xFE),
            string: Color32::from_rgb(0xCE, 0x91, 0x78),
            number: Color32::from_rgb(0xB5, 0xCE, 0xA8),
            comment: Color32::from_rgb(0x6A, 0x99, 0x55),
            punctuation: Color32::from_rgb(0xD4, 0xD4, 0xD4),
            default: Color32::from_rgb(0xD4, 0xD4, 0xD4),
        })
    }

    /// Light-theme defaults. Saturated, darker colours so every token
    /// stays legible on a white-ish background. Roughly follows VSCode's
    /// "Default Light+" palette.
    pub fn light() -> Self {
        Self::from_palette(&Palette {
            // Deep blue/purple for structural keywords (IF / THEN / END).
            keyword: Color32::from_rgb(0x7F, 0x00, 0x7F),
            // Saturated orange for triggers — IF block stands out.
            trigger: Color32::from_rgb(0xB0, 0x55, 0x00),
            // Dark teal for actions — readable against orange triggers.
            action: Color32::from_rgb(0x0B, 0x6E, 0x99),
            // VSCode's local-variable navy.
            identifier: Color32::from_rgb(0x00, 0x10, 0x80),
            // Classic Visual-Studio string red.
            string: Color32::from_rgb(0xA3, 0x15, 0x15),
            // Saturated green for numbers.
            number: Color32::from_rgb(0x09, 0x86, 0x58),
            // Slightly darker green than VSCode's so it doesn't blend
            // into number colour.
            comment: Color32::from_rgb(0x55, 0x82, 0x4B),
            punctuation: Color32::from_rgb(0x1F, 0x1F, 0x1F),
            default: Color32::from_rgb(0x1F, 0x1F, 0x1F),
        })
    }

    fn from_palette(p: &Palette) -> Self {
        let font = FontId::monospace(13.0);
        let fmt = |color| TextFormat {
            font_id: font.clone(),
            color,
            ..Default::default()
        };
        Self {
            keyword: fmt(p.keyword),
            trigger: fmt(p.trigger),
            action: fmt(p.action),
            identifier: fmt(p.identifier),
            string: fmt(p.string),
            number: fmt(p.number),
            comment: fmt(p.comment),
            punctuation: fmt(p.punctuation),
            default: fmt(p.default),
        }
    }

    /// Resolve the [`TextFormat`] that should be applied to a given kind.
    fn format_for(&self, kind: TokenKind) -> &TextFormat {
        match kind {
            TokenKind::Keyword => &self.keyword,
            TokenKind::Trigger => &self.trigger,
            TokenKind::Action => &self.action,
            TokenKind::Identifier => &self.identifier,
            TokenKind::String => &self.string,
            TokenKind::Number => &self.number,
            TokenKind::Comment => &self.comment,
            TokenKind::Punctuation => &self.punctuation,
            TokenKind::Whitespace => &self.default,
        }
    }
}

/// Tokenize `text` and lay it out into an egui [`Galley`] using `theme`.
/// Intended to plug straight into [`egui::TextEdit::layouter`].
pub fn layout_baf(ui: &egui::Ui, text: &str, wrap_width: f32, theme: &BafTheme) -> Arc<Galley> {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap_width;
    for token in tokenizer::tokenize(text) {
        let slice = &text[token.start..token.end];
        job.append(slice, 0.0, theme.format_for(token.kind).clone());
    }
    ui.ctx().fonts_mut(|f| f.layout_job(job))
}

// ── Tokenizer (egui-free) ───────────────────────────────────────────────────

mod tokenizer {
    /// Lexical category of a BAF token.
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

    /// Half-open byte range into the source string, plus its kind.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Token {
        pub kind: TokenKind,
        pub start: usize,
        pub end: usize,
    }

    /// Which BAF block we are currently inside. Drives the
    /// trigger-vs-action colouring decision: a function-call identifier
    /// gets the trigger or action kind depending on this state.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Section {
        /// Outside any `IF … END` block.
        Outside,
        /// After `IF`, before the matching `THEN`.
        Condition,
        /// After `THEN`, before the matching `END`.
        Response,
    }

    /// Tokenize a BAF script. Whitespace is preserved as `Whitespace`
    /// tokens so that joining the slices reproduces the original source.
    pub fn tokenize(text: &str) -> Vec<Token> {
        // BAF is ASCII in practice; operating on bytes keeps the scanner
        // simple. We still slice the original `&str` for spans so any
        // multibyte content survives intact (we just won't recognise it
        // as part of an identifier — it falls through as Punctuation).
        let bytes = text.as_bytes();
        let mut tokens = Vec::new();
        let mut i = 0;
        let mut section = Section::Outside;

        while i < bytes.len() {
            let start = i;
            let b = bytes[i];
            let token = match b {
                b' ' | b'\t' | b'\n' | b'\r' => {
                    while i < bytes.len()
                        && matches!(bytes[i], b' ' | b'\t' | b'\n' | b'\r')
                    {
                        i += 1;
                    }
                    Token {
                        kind: TokenKind::Whitespace,
                        start,
                        end: i,
                    }
                }
                b'/' if bytes.get(i + 1) == Some(&b'/') => {
                    // `// …` to end of line (or EOF).
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
                    // Quoted string — no escape handling (BAF doesn't use
                    // any). Newlines also terminate, defensively.
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
                            // Function call vs identifier: skip horizontal
                            // whitespace and check for `(`. Identifiers in
                            // call position get coloured by the current
                            // section so the IF/RESPONSE flow stands out.
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
                    // Single-byte fallthrough: punctuation, or any byte
                    // we don't recognise. Advance one byte; multibyte
                    // UTF-8 sequences will land entirely in this branch,
                    // one byte at a time, but rendering treats them as
                    // contiguous Punctuation runs which is fine.
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

        /// Helper: return `(kind, slice)` pairs for every token in `text`.
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
        fn whitespace_is_preserved_as_a_run() {
            assert_eq!(
                kinds_and_slices("  \n\t"),
                vec![(TokenKind::Whitespace, "  \n\t")]
            );
        }

        #[test]
        fn keywords_recognised() {
            let toks = kinds_and_slices("IF THEN RESPONSE END OR");
            let kinds: Vec<TokenKind> = toks
                .iter()
                .filter(|(k, _)| *k != TokenKind::Whitespace)
                .map(|(k, _)| *k)
                .collect();
            assert_eq!(
                kinds,
                vec![
                    TokenKind::Keyword,
                    TokenKind::Keyword,
                    TokenKind::Keyword,
                    TokenKind::Keyword,
                    TokenKind::Keyword,
                ]
            );
        }

        #[test]
        fn comment_runs_to_end_of_line() {
            let toks = kinds_and_slices("Foo // bar\nBaz");
            // ["Foo", " ", "// bar", "\n", "Baz"]
            assert_eq!(toks[0], (TokenKind::Identifier, "Foo"));
            assert_eq!(toks[2], (TokenKind::Comment, "// bar"));
            assert_eq!(toks[4], (TokenKind::Identifier, "Baz"));
        }

        #[test]
        fn string_literal() {
            assert_eq!(
                kinds_and_slices(r#""hello world""#),
                vec![(TokenKind::String, "\"hello world\"")]
            );
        }

        #[test]
        fn unterminated_string_still_tokenises() {
            // Defensive: the lexer must not panic on a missing closing quote.
            let toks = kinds_and_slices("\"oops\nrest");
            assert_eq!(toks[0].0, TokenKind::String);
        }

        #[test]
        fn integers_signed_and_unsigned() {
            assert_eq!(
                kinds_and_slices("42 -7 100"),
                vec![
                    (TokenKind::Number, "42"),
                    (TokenKind::Whitespace, " "),
                    (TokenKind::Number, "-7"),
                    (TokenKind::Whitespace, " "),
                    (TokenKind::Number, "100"),
                ]
            );
        }

        #[test]
        fn function_call_in_condition_is_trigger() {
            // The `IF` keyword switches the section so the following
            // call-position identifier is coloured as a trigger.
            let toks = kinds_and_slices("IF\n    HPPercentLT(Myself,50)\n");
            // Find the `HPPercentLT` token.
            let hp = toks.iter().find(|(_, s)| *s == "HPPercentLT").unwrap();
            assert_eq!(hp.0, TokenKind::Trigger);
            // `Myself` is not in call position → plain identifier.
            let me = toks.iter().find(|(_, s)| *s == "Myself").unwrap();
            assert_eq!(me.0, TokenKind::Identifier);
        }

        #[test]
        fn function_call_in_response_is_action() {
            let toks = kinds_and_slices("IF\nTHEN\n    Wait(3)\nEND\n");
            let wait = toks.iter().find(|(_, s)| *s == "Wait").unwrap();
            assert_eq!(wait.0, TokenKind::Action);
        }

        #[test]
        fn function_call_outside_blocks_is_identifier() {
            // Without an enclosing IF/RESPONSE we don't know whether a
            // call is a trigger or action; fall back to plain identifier.
            let toks = kinds_and_slices("Wait(3)");
            assert_eq!(toks[0], (TokenKind::Identifier, "Wait"));
        }

        #[test]
        fn end_keyword_resets_section() {
            // After `END` we are back to the outside section, so the
            // next call-position identifier is not coloured as an action.
            let toks = kinds_and_slices(
                "IF\nTHEN\n    Foo()\nEND\nBar()",
            );
            assert_eq!(
                toks.iter().find(|(_, s)| *s == "Foo").unwrap().0,
                TokenKind::Action
            );
            assert_eq!(
                toks.iter().find(|(_, s)| *s == "Bar").unwrap().0,
                TokenKind::Identifier
            );
        }

        #[test]
        fn punctuation_runs_as_individual_tokens() {
            let toks = kinds_and_slices("!(");
            assert_eq!(toks[0], (TokenKind::Punctuation, "!"));
            assert_eq!(toks[1], (TokenKind::Punctuation, "("));
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
}
