//! Code coloring from the Material palette. Highlighters classify text
//! into [`Tok`] kinds, and kinds become colors only when painting, so a
//! wallpaper change recolors code without highlighting it again.
//!
//! Apps with syntect map its scopes to kinds with [`TOKEN_SCOPES`]; the
//! small built-in [`tokenize`] covers code blocks in chat and previews.

use crate::theme::palette::blend;
use crate::theme::M3Palette;
use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId, Stroke};

/// What a piece of code is, for coloring.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Tok {
    Plain = 0,
    Comment,
    String,
    Number,
    Keyword,
    Function,
    Type,
    Variable,
    Tag,
    Attribute,
    Heading,
    Inserted,
    Deleted,
    Operator,
    Escape,
    Link,
    Emphasis,
    Macro,
}

impl Tok {
    /// Every kind, in `repr` order (`Tok::ALL[t as usize] == t`).
    pub const ALL: [Tok; 18] = [
        Tok::Plain,
        Tok::Comment,
        Tok::String,
        Tok::Number,
        Tok::Keyword,
        Tok::Function,
        Tok::Type,
        Tok::Variable,
        Tok::Tag,
        Tok::Attribute,
        Tok::Heading,
        Tok::Inserted,
        Tok::Deleted,
        Tok::Operator,
        Tok::Escape,
        Tok::Link,
        Tok::Emphasis,
        Tok::Macro,
    ];

    /// The kind with this `repr` value (`Plain` when out of range). Handy
    /// when a syntect theme encodes kinds in a color channel.
    pub fn from_u8(v: u8) -> Tok {
        Tok::ALL.get(v as usize).copied().unwrap_or(Tok::Plain)
    }

    /// The Material color for this kind of token.
    pub fn color(self, p: &M3Palette) -> Color32 {
        let dark = p.is_dark();
        let lift = |c: Color32| blend(c, if dark { Color32::WHITE } else { Color32::BLACK }, 0.12);
        match self {
            Tok::Plain | Tok::Emphasis => p.on_surface,
            Tok::Comment => p.outline,
            Tok::String => lift(p.tertiary),
            Tok::Number => blend(p.error, p.on_surface, 0.35),
            Tok::Keyword => p.primary,
            Tok::Function => blend(p.primary, p.tertiary, 0.5),
            Tok::Type => p.secondary,
            Tok::Variable => blend(p.on_surface, p.tertiary, 0.45),
            Tok::Tag => p.primary,
            Tok::Attribute => p.secondary,
            Tok::Heading => p.primary,
            Tok::Inserted => blend(Color32::from_rgb(0x4C, 0xAF, 0x50), p.on_surface, 0.2),
            Tok::Deleted => p.error,
            Tok::Operator => p.on_surface_variant,
            Tok::Escape => blend(p.error, p.primary, 0.5),
            Tok::Link => p.primary,
            Tok::Macro => blend(p.tertiary, p.primary, 0.4),
        }
    }

    pub fn italic(self) -> bool {
        matches!(self, Tok::Comment | Tok::Emphasis)
    }

    pub fn underline(self) -> bool {
        self == Tok::Link
    }
}

/// TextMate scope selectors for each kind, in priority order (later
/// entries win in a syntect theme). Build a classification theme from
/// these so every app colors the same scopes the same way.
pub const TOKEN_SCOPES: &[(&str, Tok)] = &[
    ("keyword.operator, punctuation.accessor, punctuation.separator.key-value", Tok::Operator),
    ("comment, punctuation.definition.comment", Tok::Comment),
    ("string, string.quoted, markup.raw, markup.inline.raw", Tok::String),
    ("constant.numeric, constant.language, constant.character, constant.other, support.constant, variable.other.constant", Tok::Number),
    ("constant.character.escape, string.regexp, constant.other.placeholder", Tok::Escape),
    ("keyword, storage, storage.type, storage.modifier, keyword.control, variable.language.this, variable.language.self", Tok::Keyword),
    ("keyword.operator.word, keyword.operator.logical.python, keyword.operator.new", Tok::Keyword),
    ("entity.name.function, support.function, variable.function, meta.function-call.identifier", Tok::Function),
    ("entity.name.type, entity.name.class, entity.name.struct, entity.name.enum, entity.name.trait, entity.name.union, entity.name.interface, entity.name.namespace, support.type, support.class, entity.other.inherited-class, storage.type.primitive, storage.type.numeric", Tok::Type),
    ("variable.parameter, variable.other.member, variable.other.property, meta.property.object", Tok::Variable),
    ("entity.name.tag, punctuation.definition.tag", Tok::Tag),
    ("entity.other.attribute-name, support.type.property-name, meta.mapping.key, entity.name.tag.yaml, keyword.other.definition.ini, entity.name.section.group-title.ini, meta.tag.key.toml, entity.other.attribute-name.class.css, entity.other.attribute-name.id.css", Tok::Attribute),
    ("markup.heading, entity.name.section, punctuation.definition.heading, meta.tag.table.toml, entity.name.section.toml, meta.section.toml", Tok::Heading),
    ("markup.inserted, meta.diff.header.to-file", Tok::Inserted),
    ("markup.deleted, meta.diff.header.from-file", Tok::Deleted),
    ("markup.underline.link, markup.link, string.other.link, meta.link", Tok::Link),
    ("markup.bold, markup.italic", Tok::Emphasis),
    ("meta.preprocessor, keyword.control.import, support.function.macro, entity.name.function.macro, meta.annotation, meta.attribute.rust, punctuation.definition.annotation", Tok::Macro),
];

/// A colored run within a line, in chars.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub tok: Tok,
}

/// Lay out `code` in a monospace font, coloring each line's spans (one
/// `Vec` per line; missing lines and gaps are plain).
pub fn layout(code: &str, lines: &[Vec<Span>], p: &M3Palette, size: f32) -> LayoutJob {
    let font = FontId::monospace(size);
    let plain = TextFormat::simple(font.clone(), p.on_surface);
    let mut job = LayoutJob::default();
    for (i, line) in code.split_inclusive('\n').enumerate() {
        let spans = lines.get(i).map(Vec::as_slice).unwrap_or(&[]);
        let offsets: Vec<usize> = line.char_indices().map(|(b, _)| b).chain(std::iter::once(line.len())).collect();
        let last = offsets.len() - 1;
        let mut at = 0usize;
        for s in spans {
            let (start, end) = (s.start.clamp(at, last), s.end.min(last));
            if start >= end {
                continue;
            }
            if start > at {
                job.append(&line[offsets[at]..offsets[start]], 0.0, plain.clone());
            }
            let mut f = TextFormat::simple(font.clone(), s.tok.color(p));
            f.italics = s.tok.italic();
            if s.tok.underline() {
                f.underline = Stroke::new(1.0, s.tok.color(p).gamma_multiply(0.6));
            }
            job.append(&line[offsets[start]..offsets[end]], 0.0, f);
            at = end;
        }
        if at < last {
            job.append(&line[offsets[at]..], 0.0, plain.clone());
        }
    }
    job
}

/// [`layout`] with the built-in [`tokenize`].
pub fn highlight(code: &str, lang: &str, p: &M3Palette, size: f32) -> LayoutJob {
    layout(code, &tokenize(code, lang), p, size)
}

const KEYWORDS: &[&str] = &[
    "abstract", "and", "as", "async", "await", "break", "case", "catch", "class", "const", "continue", "def", "default",
    "defer", "del", "do", "done", "elif", "else", "enum", "esac", "except", "export", "extends", "extern", "false", "fi",
    "finally", "fn", "for", "from", "func", "function", "go", "if", "impl", "implements", "import", "in", "interface",
    "is", "lambda", "let", "local", "loop", "match", "mod", "module", "mut", "new", "nil", "None", "not", "null", "or",
    "package", "pass", "private", "protected", "pub", "public", "raise", "ref", "return", "self", "Self", "static",
    "struct", "super", "switch", "then", "this", "throw", "trait", "True", "False", "true", "try", "type", "typeof",
    "undefined", "unsafe", "use", "var", "void", "where", "while", "with", "yield",
];

/// A small language-agnostic highlighter: comments (`//`, `#`, `--`, `;`
/// by language, and `/* */`), strings, numbers, common keywords, calls
/// and capitalized type names. One `Vec` of spans per line.
pub fn tokenize(code: &str, lang: &str) -> Vec<Vec<Span>> {
    let lang = lang.to_ascii_lowercase();
    let line_comment: &[&str] = match lang.as_str() {
        "py" | "python" | "sh" | "bash" | "zsh" | "fish" | "shell" | "console" | "toml" | "yaml" | "yml" | "conf"
        | "ini" | "rb" | "ruby" | "perl" | "r" | "make" | "makefile" | "dockerfile" | "nix" | "hyprlang" => &["#"],
        "lua" | "sql" | "haskell" | "hs" => &["--"],
        "lisp" | "scheme" | "clojure" | "asm" => &[";"],
        "" | "text" | "txt" | "plain" | "markdown" | "md" | "json" => &[],
        _ => &["//"],
    };
    let block_comments = !matches!(lang.as_str(), "py" | "python" | "sh" | "bash" | "zsh" | "fish" | "toml" | "yaml" | "yml");
    let plain_text = matches!(lang.as_str(), "" | "text" | "txt" | "plain");
    let mut in_block = false;
    code.split_inclusive('\n')
        .map(|line| {
            let chars: Vec<char> = line.chars().collect();
            let mut spans = Vec::new();
            if plain_text {
                return spans;
            }
            let mut i = 0;
            let starts = |i: usize, pat: &str| pat.chars().enumerate().all(|(k, c)| chars.get(i + k) == Some(&c));
            while i < chars.len() {
                if in_block {
                    let from = i;
                    while i < chars.len() && !starts(i, "*/") {
                        i += 1;
                    }
                    if i < chars.len() {
                        i += 2;
                        in_block = false;
                    }
                    spans.push(Span { start: from, end: i, tok: Tok::Comment });
                    continue;
                }
                let c = chars[i];
                if block_comments && starts(i, "/*") {
                    in_block = true;
                    continue;
                }
                if line_comment.iter().any(|m| starts(i, m)) {
                    let end = chars.len() - usize::from(chars.last() == Some(&'\n'));
                    spans.push(Span { start: i, end, tok: Tok::Comment });
                    break;
                }
                if c == '"' || c == '\'' || c == '`' {
                    // A Rust lifetime ('a) isn't a string.
                    let lifetime = c == '\''
                        && lang == "rust"
                        && chars.get(i + 1).is_some_and(|n| n.is_alphabetic())
                        && chars.get(i + 2) != Some(&'\'');
                    if lifetime {
                        i += 1;
                        continue;
                    }
                    let from = i;
                    i += 1;
                    while i < chars.len() && chars[i] != c && chars[i] != '\n' {
                        if chars[i] == '\\' {
                            i += 1;
                        }
                        i += 1;
                    }
                    i = (i + 1).min(chars.len());
                    spans.push(Span { start: from, end: i, tok: Tok::String });
                    continue;
                }
                if c.is_ascii_digit() {
                    let from = i;
                    while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '.' || chars[i] == '_') {
                        i += 1;
                    }
                    spans.push(Span { start: from, end: i, tok: Tok::Number });
                    continue;
                }
                if c.is_alphabetic() || c == '_' {
                    let from = i;
                    while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                        i += 1;
                    }
                    let word: String = chars[from..i].iter().collect();
                    let next = chars[i..].iter().find(|c| **c != ' ');
                    let tok = if KEYWORDS.contains(&word.as_str()) {
                        Tok::Keyword
                    } else if next == Some(&'!') && lang == "rust" {
                        Tok::Macro
                    } else if next == Some(&'(') {
                        Tok::Function
                    } else if word.chars().next().is_some_and(char::is_uppercase) && word.chars().any(char::is_lowercase) {
                        Tok::Type
                    } else {
                        Tok::Plain
                    };
                    if tok != Tok::Plain {
                        spans.push(Span { start: from, end: i, tok });
                    }
                    continue;
                }
                i += 1;
            }
            spans
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(line: &str, lang: &str) -> Vec<(String, Tok)> {
        let chars: Vec<char> = line.chars().collect();
        tokenize(line, lang)[0].iter().map(|s| (chars[s.start..s.end].iter().collect(), s.tok)).collect()
    }

    #[test]
    fn kinds_round_trip_through_u8() {
        for t in Tok::ALL {
            assert_eq!(Tok::from_u8(t as u8), t);
        }
        assert_eq!(Tok::from_u8(200), Tok::Plain);
        assert!(TOKEN_SCOPES.iter().any(|(_, t)| *t == Tok::Comment));
    }

    #[test]
    fn tokenizes_common_code() {
        let t = toks("fn main() { let x = \"hi\"; } // done", "rust");
        assert!(t.contains(&("fn".into(), Tok::Keyword)));
        assert!(t.contains(&("main".into(), Tok::Function)));
        assert!(t.contains(&("\"hi\"".into(), Tok::String)));
        assert!(t.contains(&("// done".into(), Tok::Comment)));
        let t = toks("def f(x): return 42  # answer", "python");
        assert!(t.contains(&("42".into(), Tok::Number)));
        assert!(t.contains(&("# answer".into(), Tok::Comment)));
        let t = toks("let v: Vec<u8> = vec![];", "rust");
        assert!(t.contains(&("Vec".into(), Tok::Type)));
        assert!(t.contains(&("vec".into(), Tok::Macro)));
        assert!(toks("anything at all", "text").is_empty());
        let t = toks("fn f<'a>(s: &'a str) -> char { 'x' }", "rust");
        assert!(t.contains(&("'x'".into(), Tok::String)));
        assert!(!t.iter().any(|(s, _)| s.starts_with("'a")), "lifetimes aren't strings");
    }

    #[test]
    fn block_comments_span_lines() {
        let lines = tokenize("a /* one\ntwo */ b\nc", "c");
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], vec![Span { start: 2, end: 9, tok: Tok::Comment }]);
        assert_eq!(lines[1], vec![Span { start: 0, end: 6, tok: Tok::Comment }]);
        assert!(lines[2].is_empty());
    }

    #[test]
    fn layout_keeps_the_text_and_colors_tokens() {
        let p = M3Palette::from_seed("#6750A4", true);
        let code = "fn main() { let x = 1; }\nüñí \"ok\"";
        let job = highlight(code, "rust", &p, 13.0);
        assert_eq!(job.text, code);
        assert!(job.sections.len() > 3, "tokens get separate colors");
        assert!(job.sections.iter().any(|s| s.format.color == p.primary));
        // Spans past the end of a line are cut, not a panic.
        let job = layout("ab", &[vec![Span { start: 1, end: 99, tok: Tok::Keyword }]], &p, 13.0);
        assert_eq!(job.text, "ab");
    }
}
