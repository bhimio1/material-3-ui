//! A compact Markdown renderer for chat messages and previews: headings,
//! paragraphs with emphasis/code/links, lists, task lists, quotes, rules,
//! tables (scrolling when wide) and fenced code blocks colored from the
//! palette. Parsing is cached, and an unfinished document (a reply still
//! streaming in) renders sensibly.

use crate::code;
use crate::theme::M3Palette;
use crate::widgets;
use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, CornerRadius, FontFamily, FontId, Frame, Margin, RichText, Stroke, Ui};
use egui_material_icons::icons::*;
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock};

/// Colors a code block: (code, language) to one `Vec` of spans per line.
/// Apps with syntect plug theirs in through [`show_with`]; [`show`] uses
/// [`code::tokenize`].
pub type Highlighter = dyn Fn(&str, &str) -> Vec<Vec<code::Span>> + Send + Sync;

/// A run of text with one style.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Span {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    pub strike: bool,
    pub link: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    Paragraph(Vec<Span>),
    Heading(u8, Vec<Span>),
    Code { lang: String, code: String },
    List { start: Option<u64>, items: Vec<Vec<Block>> },
    Quote(Vec<Block>),
    Rule,
    Table { header: Vec<Vec<Span>>, rows: Vec<Vec<Vec<Span>>> },
}

/// Parse markdown into blocks.
pub fn parse(src: &str) -> Vec<Block> {
    let opts = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let mut p = Builder::default();
    for ev in Parser::new_ext(src, opts) {
        p.event(ev);
    }
    p.finish()
}

#[derive(Default)]
struct Style {
    bold: u32,
    italic: u32,
    strike: u32,
    link: Option<String>,
}

enum Container {
    Root(Vec<Block>),
    Quote(Vec<Block>),
    List { start: Option<u64>, items: Vec<Vec<Block>> },
    Item(Vec<Block>),
}

#[derive(Default)]
struct Builder {
    stack: Vec<Container>,
    spans: Vec<Span>,
    style: Style,
    heading: Option<u8>,
    code: Option<(String, String)>,
    #[allow(clippy::type_complexity)]
    table: Option<(Vec<Vec<Span>>, Vec<Vec<Vec<Span>>>, Vec<Vec<Span>>, bool)>,
}

impl Builder {
    fn blocks(&mut self) -> &mut Vec<Block> {
        if self.stack.is_empty() {
            self.stack.push(Container::Root(Vec::new()));
        }
        match self.stack.last_mut().expect("non-empty") {
            Container::Root(b) | Container::Quote(b) | Container::Item(b) => b,
            Container::List { items, .. } => {
                if items.is_empty() {
                    items.push(Vec::new());
                }
                items.last_mut().expect("non-empty")
            }
        }
    }

    fn flush_paragraph(&mut self) {
        if self.spans.is_empty() {
            return;
        }
        let spans = std::mem::take(&mut self.spans);
        self.blocks().push(Block::Paragraph(spans));
    }

    fn push_text(&mut self, text: &str, code: bool) {
        self.spans.push(Span {
            text: text.to_string(),
            bold: self.style.bold > 0,
            italic: self.style.italic > 0,
            strike: self.style.strike > 0,
            code,
            link: self.style.link.clone(),
        });
    }

    fn event(&mut self, ev: Event<'_>) {
        if let Some((_, code)) = &mut self.code {
            match ev {
                Event::Text(t) => code.push_str(&t),
                Event::End(TagEnd::CodeBlock) => {
                    let (lang, mut code) = self.code.take().expect("in code");
                    if code.ends_with('\n') {
                        code.pop();
                    }
                    self.blocks().push(Block::Code { lang, code });
                }
                _ => {}
            }
            return;
        }
        match ev {
            Event::Start(tag) => match tag {
                Tag::Paragraph => self.flush_paragraph(),
                Tag::Heading { level, .. } => {
                    self.flush_paragraph();
                    self.heading = Some(match level {
                        HeadingLevel::H1 => 1,
                        HeadingLevel::H2 => 2,
                        HeadingLevel::H3 => 3,
                        _ => 4,
                    });
                }
                Tag::BlockQuote(_) => {
                    self.flush_paragraph();
                    self.stack.push(Container::Quote(Vec::new()));
                }
                Tag::CodeBlock(kind) => {
                    self.flush_paragraph();
                    let lang = match kind {
                        CodeBlockKind::Fenced(l) => l.split(|c: char| c == ',' || c.is_whitespace()).next().unwrap_or("").to_string(),
                        CodeBlockKind::Indented => String::new(),
                    };
                    self.code = Some((lang, String::new()));
                }
                Tag::List(start) => {
                    self.flush_paragraph();
                    self.stack.push(Container::List { start, items: Vec::new() });
                }
                Tag::Item => {
                    self.flush_paragraph();
                    self.stack.push(Container::Item(Vec::new()));
                }
                Tag::Emphasis => self.style.italic += 1,
                Tag::Strong => self.style.bold += 1,
                Tag::Strikethrough => self.style.strike += 1,
                Tag::Link { dest_url, .. } => self.style.link = Some(dest_url.to_string()),
                Tag::Table(_) => {
                    self.flush_paragraph();
                    self.table = Some((Vec::new(), Vec::new(), Vec::new(), true));
                }
                Tag::TableHead => {
                    if let Some(t) = &mut self.table {
                        t.3 = true;
                    }
                }
                Tag::TableRow => {
                    if let Some(t) = &mut self.table {
                        t.2.clear();
                    }
                }
                Tag::TableCell => self.spans.clear(),
                _ => {}
            },
            Event::End(end) => match end {
                TagEnd::Paragraph => self.flush_paragraph(),
                TagEnd::Heading(_) => {
                    let spans = std::mem::take(&mut self.spans);
                    let level = self.heading.take().unwrap_or(3);
                    self.blocks().push(Block::Heading(level, spans));
                }
                TagEnd::BlockQuote(_) => {
                    self.flush_paragraph();
                    if let Some(Container::Quote(b)) = self.stack.pop() {
                        self.blocks().push(Block::Quote(b));
                    }
                }
                TagEnd::List(_) => {
                    self.flush_paragraph();
                    if let Some(Container::List { start, items }) = self.stack.pop() {
                        self.blocks().push(Block::List { start, items });
                    }
                }
                TagEnd::Item => {
                    self.flush_paragraph();
                    if let Some(Container::Item(b)) = self.stack.pop() {
                        if let Some(Container::List { items, .. }) = self.stack.last_mut() {
                            items.push(b);
                        }
                    }
                }
                TagEnd::Emphasis => self.style.italic = self.style.italic.saturating_sub(1),
                TagEnd::Strong => self.style.bold = self.style.bold.saturating_sub(1),
                TagEnd::Strikethrough => self.style.strike = self.style.strike.saturating_sub(1),
                TagEnd::Link => self.style.link = None,
                TagEnd::TableCell => {
                    let cell = std::mem::take(&mut self.spans);
                    if let Some(t) = &mut self.table {
                        t.2.push(cell);
                    }
                }
                TagEnd::TableHead => {
                    if let Some(t) = &mut self.table {
                        t.0 = std::mem::take(&mut t.2);
                        t.3 = false;
                    }
                }
                TagEnd::TableRow => {
                    if let Some(t) = &mut self.table {
                        let row = std::mem::take(&mut t.2);
                        if t.3 {
                            t.0 = row;
                        } else {
                            t.1.push(row);
                        }
                    }
                }
                TagEnd::Table => {
                    if let Some((header, rows, _, _)) = self.table.take() {
                        self.blocks().push(Block::Table { header, rows });
                    }
                }
                _ => {}
            },
            Event::Text(t) => self.push_text(&t, false),
            Event::Code(t) => self.push_text(&t, true),
            Event::SoftBreak => self.push_text(" ", false),
            Event::HardBreak => self.push_text("\n", false),
            Event::Rule => {
                self.flush_paragraph();
                self.blocks().push(Block::Rule);
            }
            Event::TaskListMarker(done) => self.push_text(if done { "■ " } else { "□ " }, false),
            Event::Html(h) | Event::InlineHtml(h) => self.push_text(&h, false),
            _ => {}
        }
    }

    fn finish(mut self) -> Vec<Block> {
        self.flush_paragraph();
        if let Some((lang, code)) = self.code.take() {
            // Unterminated fence while streaming.
            self.blocks().push(Block::Code { lang, code });
        }
        // Close anything left open (streaming).
        while self.stack.len() > 1 {
            match self.stack.pop() {
                Some(Container::Quote(b)) => self.blocks().push(Block::Quote(b)),
                Some(Container::List { start, items }) => self.blocks().push(Block::List { start, items }),
                Some(Container::Item(b)) => {
                    if let Some(Container::List { items, .. }) = self.stack.last_mut() {
                        items.push(b);
                    }
                }
                _ => {}
            }
        }
        match self.stack.pop() {
            Some(Container::Root(b)) => b,
            _ => Vec::new(),
        }
    }
}

// ------------------------------------------------------------------ cache

fn hash_of(t: &impl Hash) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    t.hash(&mut h);
    h.finish()
}

fn parsed(src: &str) -> Arc<Vec<Block>> {
    static CACHE: OnceLock<Mutex<HashMap<u64, Arc<Vec<Block>>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let key = hash_of(&src);
    if let Some(b) = cache.lock().ok().and_then(|c| c.get(&key).cloned()) {
        return b;
    }
    let blocks = Arc::new(parse(src));
    if let Ok(mut c) = cache.lock() {
        if c.len() > 2000 {
            c.clear();
        }
        c.insert(key, blocks.clone());
    }
    blocks
}

// ----------------------------------------------------------------- render

/// Render Markdown at body text `size`. Links are passed to `on_link`
/// (open them with `ui.ctx().open_url` or `xdg-open`).
pub fn show(ui: &mut Ui, src: &str, size: f32, on_link: &mut dyn FnMut(String)) {
    show_with(ui, src, size, on_link, None);
}

/// [`show`] with an app's own code highlighter.
pub fn show_with(ui: &mut Ui, src: &str, size: f32, on_link: &mut dyn FnMut(String), highlight: Option<&Highlighter>) {
    let blocks = parsed(src);
    let p = widgets::palette(ui);
    let r = Render { p: &p, size, highlight };
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 6.0;
        for b in blocks.iter() {
            r.block(ui, b, on_link);
        }
    });
}

struct Render<'a> {
    p: &'a M3Palette,
    size: f32,
    highlight: Option<&'a Highlighter>,
}

impl Render<'_> {
    fn block(&self, ui: &mut Ui, b: &Block, on_link: &mut dyn FnMut(String)) {
        let (p, size) = (self.p, self.size);
        match b {
            Block::Paragraph(spans) => spans_label(ui, spans, p, size, p.on_surface, on_link),
            Block::Heading(level, spans) => {
                let s = match level {
                    1 => size * 1.6,
                    2 => size * 1.35,
                    3 => size * 1.15,
                    _ => size * 1.05,
                };
                ui.add_space(4.0);
                let bold: Vec<Span> = spans.iter().map(|s| Span { bold: true, ..s.clone() }).collect();
                spans_label(ui, &bold, p, s, p.on_surface, on_link);
            }
            Block::Code { lang, code } => self.code_block(ui, lang, code),
            Block::List { start, items } => {
                for (i, item) in items.iter().enumerate() {
                    ui.horizontal_top(|ui| {
                        let marker = match start {
                            Some(n) => format!("{}.", n + i as u64),
                            None => "•".into(),
                        };
                        ui.add_sized([22.0, size + 4.0], egui::Label::new(RichText::new(marker).size(size).color(p.primary)));
                        ui.vertical(|ui| {
                            for b in item {
                                self.block(ui, b, on_link);
                            }
                        });
                    });
                }
            }
            Block::Quote(inner) => {
                let shown = Frame::new().inner_margin(Margin { left: 14, right: 4, top: 2, bottom: 2 }).show(ui, |ui| {
                    for b in inner {
                        self.block(ui, b, on_link);
                    }
                });
                // The bar spans what the quote actually took.
                let r = shown.response.rect;
                ui.painter().rect_filled(
                    egui::Rect::from_min_size(r.min, egui::vec2(3.0, r.height().max(size))),
                    CornerRadius::same(2),
                    p.outline_variant,
                );
            }
            Block::Rule => {
                ui.separator();
            }
            Block::Table { header, rows } => {
                // Cells keep their natural width; wide tables scroll.
                ui.scope(|ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                    egui::ScrollArea::horizontal().id_salt(ui.next_auto_id()).show(ui, |ui| {
                        Frame::new()
                            .stroke(Stroke::new(1.0, p.outline_variant))
                            .corner_radius(CornerRadius::same(10))
                            .inner_margin(Margin::same(8))
                            .show(ui, |ui| {
                                egui::Grid::new(ui.next_auto_id()).striped(true).spacing([16.0, 6.0]).show(ui, |ui| {
                                    for cell in header {
                                        let bold: Vec<Span> = cell.iter().map(|s| Span { bold: true, ..s.clone() }).collect();
                                        spans_label(ui, &bold, p, size, p.on_surface, on_link);
                                    }
                                    ui.end_row();
                                    for row in rows {
                                        for cell in row {
                                            spans_label(ui, cell, p, size, p.on_surface, on_link);
                                        }
                                        ui.end_row();
                                    }
                                });
                            });
                    });
                });
            }
        }
    }

    fn code_block(&self, ui: &mut Ui, lang: &str, code: &str) {
        let p = self.p;
        Frame::new()
            .fill(p.surface_container_highest)
            .corner_radius(CornerRadius::same(14))
            .inner_margin(Margin { left: 14, right: 8, top: 6, bottom: 10 })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(if lang.is_empty() { "code" } else { lang }).size(12.0).color(p.on_surface_variant));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::icon_button_sized(ui, ICON_CONTENT_COPY, "Copy", true, 28.0, false).clicked() {
                            ui.ctx().copy_text(code.to_string());
                        }
                    });
                });
                egui::ScrollArea::horizontal().id_salt(ui.next_auto_id()).show(ui, |ui| {
                    let job = highlight(code, lang, p, self.size * 0.9, self.highlight);
                    ui.add(egui::Label::new(job).extend());
                });
            });
    }
}

/// Spans as one wrapped label; links are found from the hovered character
/// so they can be clicked.
pub fn spans_label(ui: &mut Ui, spans: &[Span], p: &M3Palette, size: f32, color: Color32, on_link: &mut dyn FnMut(String)) {
    let mut job = LayoutJob::default();
    job.wrap.max_width =
        if ui.style().wrap_mode == Some(egui::TextWrapMode::Extend) { f32::INFINITY } else { ui.available_width() };
    let mut links: Vec<(std::ops::Range<usize>, String)> = Vec::new();
    let mut chars = 0usize;
    for s in spans {
        let family = if s.code {
            FontFamily::Monospace
        } else if s.bold {
            crate::theme::style::medium_family()
        } else {
            FontFamily::Proportional
        };
        let mut f = TextFormat {
            font_id: FontId::new(if s.code { size * 0.92 } else { size }, family),
            color: if s.link.is_some() {
                p.primary
            } else if s.code {
                p.on_secondary_container
            } else {
                color
            },
            italics: s.italic,
            ..Default::default()
        };
        if s.code {
            f.background = p.secondary_container.gamma_multiply(0.6);
        }
        if s.strike {
            f.strikethrough = Stroke::new(1.0, color);
        }
        if s.link.is_some() {
            f.underline = Stroke::new(1.0, p.primary.gamma_multiply(0.6));
        }
        let n = s.text.chars().count();
        if let Some(l) = &s.link {
            links.push((chars..chars + n, l.clone()));
        }
        chars += n;
        job.append(&s.text, 0.0, f);
    }
    let galley = ui.painter().layout_job(job);
    let sense = if links.is_empty() { egui::Sense::hover() } else { egui::Sense::click() };
    let resp = ui.add(egui::Label::new(galley.clone()).sense(sense).selectable(links.is_empty()));
    if !links.is_empty() {
        if let Some(pos) = resp.hover_pos() {
            let idx = galley.cursor_from_pos(pos - resp.rect.min).index.0;
            if let Some((_, url)) = links.iter().find(|(r, _)| r.contains(&idx)) {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                if resp.clicked() {
                    on_link(url.clone());
                }
                resp.on_hover_text(url);
            }
        }
    }
}

/// A code block's layout: token spans (from `highlighter`, else the
/// built-in tokenizer) are cached, and colored from the palette each time
/// so a theme change shows at once. Very long blocks are shown plain.
pub fn highlight(code: &str, lang: &str, p: &M3Palette, size: f32, highlighter: Option<&Highlighter>) -> LayoutJob {
    type Lines = Arc<Vec<Vec<code::Span>>>;
    static CACHE: OnceLock<Mutex<HashMap<u64, Lines>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let key = hash_of(&(code, lang, highlighter.is_some()));
    let spans = match cache.lock().ok().and_then(|c| c.get(&key).cloned()) {
        Some(s) => s,
        None => {
            let s = Arc::new(match highlighter {
                _ if code.len() > 60_000 => Vec::new(),
                Some(h) => h(code, lang),
                None => code::tokenize(code, lang),
            });
            if let Ok(mut c) = cache.lock() {
                if c.len() > 500 {
                    c.clear();
                }
                c.insert(key, s.clone());
            }
            s
        }
    };
    code::layout(code, &spans, p, size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_markdown() {
        let b = parse("# Title\n\nSome **bold** and `code` with [a link](https://x.y).\n\n- one\n- two\n\n```rust\nfn main() {}\n```\n\n> quote\n\n| a | b |\n|---|---|\n| 1 | 2 |\n");
        assert!(matches!(&b[0], Block::Heading(1, s) if s[0].text == "Title"));
        let Block::Paragraph(spans) = &b[1] else { panic!("{:?}", b[1]) };
        assert!(spans.iter().any(|s| s.bold && s.text == "bold"));
        assert!(spans.iter().any(|s| s.code && s.text == "code"));
        assert!(spans.iter().any(|s| s.link.as_deref() == Some("https://x.y")));
        assert!(matches!(&b[2], Block::List { start: None, items } if items.len() == 2));
        assert!(matches!(&b[3], Block::Code { lang, code } if lang == "rust" && code == "fn main() {}"));
        assert!(matches!(&b[4], Block::Quote(_)));
        assert!(matches!(&b[5], Block::Table { header, rows } if header.len() == 2 && rows.len() == 1));
    }

    #[test]
    fn unterminated_fence_while_streaming() {
        let b = parse("Here:\n\n```py\nprint(1)\n");
        assert!(matches!(&b[1], Block::Code { lang, code } if lang == "py" && code.contains("print")));
    }

    #[test]
    fn task_lists_nesting_and_breaks() {
        let b = parse("1. [x] done\n2. [ ] todo\n   - nested *em* ~~gone~~\n\nline one  \nline two");
        let Block::List { start: Some(1), items } = &b[0] else { panic!("{:?}", b[0]) };
        assert_eq!(items.len(), 2);
        let Block::Paragraph(first) = &items[0][0] else { panic!("{:?}", items[0]) };
        assert!(first[0].text.starts_with('■'));
        assert!(matches!(&items[1][1], Block::List { start: None, items } if items.len() == 1));
        let Block::List { items: nested, .. } = &items[1][1] else { unreachable!() };
        let Block::Paragraph(spans) = &nested[0][0] else { panic!() };
        assert!(spans.iter().any(|s| s.italic && s.text == "em"));
        assert!(spans.iter().any(|s| s.strike && s.text == "gone"));
        let Block::Paragraph(lines) = &b[1] else { panic!("{:?}", b[1]) };
        assert!(lines.iter().any(|s| s.text == "\n"), "hard break");
    }

    #[test]
    fn unfinished_lists_and_quotes_while_streaming() {
        let b = parse("> quoted\n> - item one\n> - item tw");
        let Block::Quote(inner) = &b[0] else { panic!("{:?}", b[0]) };
        assert!(matches!(&inner[1], Block::List { items, .. } if items.len() == 2));
        assert!(parse("").is_empty());
    }

    #[test]
    fn highlights_with_palette_or_a_plugged_highlighter() {
        let p = M3Palette::from_seed("#6750A4", true);
        let job = highlight("fn main() { let x = 1; }", "rust", &p, 13.0, None);
        assert!(job.sections.len() > 3, "tokens get separate colors");
        assert!(job.sections.iter().any(|s| s.format.color == p.primary));
        assert_eq!(job.text, "fn main() { let x = 1; }");
        let all_strings = |code: &str, _: &str| -> Vec<Vec<code::Span>> {
            code.lines().map(|l| vec![code::Span { start: 0, end: l.chars().count(), tok: code::Tok::String }]).collect()
        };
        let job = highlight("abc", "x", &p, 13.0, Some(&all_strings));
        assert_eq!(job.sections.len(), 1);
        assert_eq!(job.sections[0].format.color, code::Tok::String.color(&p));
    }

    #[test]
    fn renders_without_panicking() {
        let ctx = egui::Context::default();
        crate::widgets::set_palette(&ctx, M3Palette::from_seed("#6750A4", false));
        let doc = "# T\n\n**b** [l](https://x)\n\n- a\n\n> q\n\n```rust\nfn x() {}\n```\n\n|a|b|\n|-|-|\n|1|2|\n\n---\n";
        for _ in 0..2 {
            ctx.run_ui(egui::RawInput::default(), |ui| {
                let spacing = ui.spacing().item_spacing;
                show(ui, doc, 14.0, &mut |_| {});
                assert_eq!(ui.spacing().item_spacing, spacing, "the caller's spacing is left alone");
                assert_eq!(ui.style().wrap_mode, None);
            })
            .drop_without_applying_deltas();
        }
    }
}
