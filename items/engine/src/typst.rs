//! A `markdown` field as typst markup, for its PDF (`pdf.render`): its
//! headings, which become the PDF's bookmarks, text, lists, task lists,
//! quotes, code, tables, rules and footnotes, and its references as links
//! to the app.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Parser, Tag, TagEnd};

use crate::markdown::Reference;

/// The page and text settings every PDF starts with.
const PREAMBLE: &str = "#set page(paper: \"a4\", margin: 2cm)
#set text(font: \"Libertinus Serif\", size: 11pt, lang: \"en\")
#set par(justify: false)
#show raw: set text(font: \"DejaVu Sans Mono\", size: 9pt)
#show link: underline
";

/// Text, escaped so that typst reads it as text.
fn escaped(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if "\\#*_`$<>@[]=-+~/\"'".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// A string as a typst string literal.
fn string(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

fn level(level: HeadingLevel) -> usize {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// Where the conversion is: the typst written so far, and what it is
/// inside of.
struct Writer<'a> {
    out: String,
    link_of: &'a dyn Fn(&Reference) -> String,
    /// Each list it is in, with its start if it is ordered.
    lists: Vec<Option<u64>>,
    /// A code block's language and text, while in one.
    code: Option<(String, String)>,
    /// A table's rows of cells, while in one.
    table: Option<Vec<Vec<String>>>,
    /// A table cell's typst, while in one.
    cell: Option<String>,
    /// Whether the cells are the table's header.
    header: bool,
    /// Each footnote's typst, by label, read before the text that refers
    /// to it is written.
    notes: HashMap<String, String>,
    /// The footnotes written so far, which a later reference repeats.
    noted: HashSet<String>,
    /// The typst written before the footnote definition it is in, while in
    /// one, and the definition's label.
    note: Option<(String, String)>,
}

impl Writer<'_> {
    /// Inline typst, into the cell it is in, else the document.
    fn put(&mut self, text: &str) {
        match self.cell.as_mut() {
            Some(cell) => cell.push_str(text),
            None => self.out.push_str(text),
        }
    }

    fn event(&mut self, event: Event<'_>) {
        if self.footnote(&event) {
            return;
        }
        if self.code.is_some() {
            self.in_code(event);
        } else if !self.block(&event) && !self.table(&event) {
            self.inline(event);
        }
    }

    fn in_code(&mut self, event: Event<'_>) {
        match event {
            Event::Text(t) => {
                if let Some((_, body)) = self.code.as_mut() {
                    body.push_str(&t);
                }
            }
            Event::End(TagEnd::CodeBlock) => {
                let (lang, body) = self.code.take().unwrap_or_default();
                let lang = if lang.is_empty() {
                    "none".to_owned()
                } else {
                    string(&lang)
                };
                let _ = writeln!(
                    self.out,
                    "#raw(block: true, lang: {lang}, {})\n",
                    string(body.trim_end_matches('\n'))
                );
            }
            _ => {}
        }
    }

    /// Headings, paragraphs, quotes, code, lists and rules; whether it was one.
    fn block(&mut self, event: &Event<'_>) -> bool {
        match event {
            Event::Start(Tag::Heading { level: l, .. }) => {
                self.out.push_str(&"=".repeat(level(*l)));
                self.out.push(' ');
            }
            Event::End(TagEnd::Heading(_)) => self.out.push_str("\n\n"),
            Event::End(TagEnd::Paragraph) if self.cell.is_none() => {
                self.out
                    .push_str(if self.lists.is_empty() { "\n\n" } else { "\n" });
            }
            Event::Start(Tag::BlockQuote(_)) => self.out.push_str("#quote(block: true)[\n"),
            Event::End(TagEnd::BlockQuote(_)) => self.out.push_str("]\n\n"),
            Event::Start(Tag::CodeBlock(kind)) => {
                let lang = match kind {
                    CodeBlockKind::Fenced(lang) => {
                        lang.split_whitespace().next().unwrap_or("").to_owned()
                    }
                    CodeBlockKind::Indented => String::new(),
                };
                self.code = Some((lang, String::new()));
            }
            Event::Start(Tag::List(start)) => {
                if !self.lists.is_empty() {
                    self.out.push('\n');
                }
                self.lists.push(*start);
            }
            Event::End(TagEnd::List(_)) => {
                self.lists.pop();
                if self.lists.is_empty() {
                    self.out.push('\n');
                }
            }
            Event::Start(Tag::Item) => {
                let depth = self.lists.len().saturating_sub(1);
                let marker = if matches!(self.lists.last(), Some(Some(_))) {
                    "+"
                } else {
                    "-"
                };
                let _ = write!(self.out, "{}{marker} ", "  ".repeat(depth));
            }
            Event::End(TagEnd::Item) if !self.out.ends_with('\n') => self.out.push('\n'),
            Event::TaskListMarker(done) => self.out.push_str(if *done { "☒ " } else { "☐ " }),
            Event::Rule => self.out.push_str("#line(length: 100%)\n\n"),
            Event::Start(Tag::Paragraph) | Event::End(TagEnd::Paragraph | TagEnd::Item) => {}
            _ => return false,
        }
        true
    }

    /// A table and its cells; whether it was one.
    fn table(&mut self, event: &Event<'_>) -> bool {
        match event {
            Event::Start(Tag::Table(_)) => self.table = Some(Vec::new()),
            Event::Start(Tag::TableHead | Tag::TableRow) => {
                if matches!(event, Event::Start(Tag::TableHead)) {
                    self.header = true;
                }
                if let Some(rows) = self.table.as_mut() {
                    rows.push(Vec::new());
                }
            }
            Event::End(TagEnd::TableHead) => self.header = false,
            Event::Start(Tag::TableCell) => self.cell = Some(String::new()),
            Event::End(TagEnd::TableCell) => {
                let header = self.header;
                if let (Some(rows), Some(text)) = (self.table.as_mut(), self.cell.take())
                    && let Some(row) = rows.last_mut()
                {
                    row.push(if header {
                        format!("#strong[{text}]")
                    } else {
                        text
                    });
                }
            }
            Event::End(TagEnd::Table) => {
                let rows = self.table.take().unwrap_or_default();
                let columns = rows.first().map_or(1, Vec::len).max(1);
                let cells: Vec<String> = rows.iter().flatten().map(|c| format!("[{c}]")).collect();
                let _ = writeln!(
                    self.out,
                    "#table(columns: {columns}, {})\n",
                    cells.join(", ")
                );
            }
            Event::End(TagEnd::TableRow) => {}
            _ => return false,
        }
        true
    }

    /// A footnote's definition, kept in `notes` instead of written, and a
    /// reference to one: the note at its first, the same note's number at
    /// each later one, and its text if it names none; whether it was one.
    fn footnote(&mut self, event: &Event<'_>) -> bool {
        match event {
            Event::Start(Tag::FootnoteDefinition(label)) => {
                let before = std::mem::take(&mut self.out);
                self.note = Some((label.to_string(), before));
            }
            Event::End(TagEnd::FootnoteDefinition) => {
                if let Some((label, before)) = self.note.take() {
                    let text = std::mem::replace(&mut self.out, before);
                    self.notes
                        .entry(label)
                        .or_insert_with(|| text.trim().to_owned());
                }
            }
            Event::FootnoteReference(label) => {
                let label = label.to_string();
                let name = format!("<note-{}>", label_of(&label));
                match self.notes.get(&label) {
                    Some(_) if self.noted.contains(&label) => {
                        self.put(&format!("#footnote({name})"));
                    }
                    Some(text) => {
                        let written = format!("#footnote[{text}]{name}");
                        self.noted.insert(label);
                        self.put(&written);
                    }
                    None => self.put(&escaped(&format!("[^{label}]"))),
                }
            }
            _ => return false,
        }
        true
    }

    /// Text and its markup.
    fn inline(&mut self, event: Event<'_>) {
        match event {
            Event::Start(Tag::Emphasis) => self.put("#emph["),
            Event::Start(Tag::Strong) => self.put("#strong["),
            Event::Start(Tag::Strikethrough) => self.put("#strike["),
            Event::End(
                TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough | TagEnd::Link,
            ) => {
                self.put("]");
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                let to = Reference::parse(&dest_url)
                    .map_or_else(|| dest_url.to_string(), |r| (self.link_of)(&r));
                self.put(&format!("#link({})[", string(&to)));
            }
            Event::Text(t) | Event::Html(t) | Event::InlineHtml(t) => self.put(&escaped(&t)),
            Event::Code(t) => self.put(&format!("#raw({})", string(&t))),
            Event::SoftBreak => self.put(" "),
            Event::HardBreak => self.put(" \\\n"),
            _ => {}
        }
    }
}

/// A footnote's label as a typst label: letters, digits, `-` and `_`,
/// each other character as its code point, so that labels stay apart.
fn label_of(label: &str) -> String {
    label
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c.to_string()
            } else {
                format!("_{:x}_", u32::from(c))
            }
        })
        .collect()
}

fn writer(link_of: &dyn Fn(&Reference) -> String) -> Writer<'_> {
    Writer {
        out: String::new(),
        link_of,
        lists: Vec::new(),
        code: None,
        table: None,
        cell: None,
        header: false,
        notes: HashMap::new(),
        noted: HashSet::new(),
        note: None,
    }
}

/// `text`, a `markdown` field's value, as a typst document: `link_of`
/// gives the address a reference links to in the app. Footnotes are read
/// first, so that each is written where it is first referred to.
// r[impl pdf.render]
#[must_use]
pub fn markup(text: &str, link_of: &dyn Fn(&Reference) -> String) -> String {
    let mut reading = writer(link_of);
    for event in Parser::new_ext(text, crate::markdown::options()) {
        reading.event(event);
    }
    let mut writing = writer(link_of);
    writing.notes = reading.notes;
    writing.out = format!("{PREAMBLE}\n");
    for event in Parser::new_ext(text, crate::markdown::options()) {
        writing.event(event);
    }
    writing.out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(r: &Reference) -> String {
        format!(
            "https://kuria.example/akt/{}#{}",
            r.number,
            r.heading.clone().unwrap_or_default()
        )
    }

    fn body(text: &str) -> String {
        markup(text, &link)
            .trim_start_matches(PREAMBLE)
            .trim()
            .to_owned()
    }

    // r[verify pdf.render]
    #[test]
    fn markdown_becomes_typst() {
        let cases: [(&str, &str); 13] = [
            (
                "# Install {#install}\n\nRun *it* **now** ~~not~~.",
                "= Install\n\nRun #emph[it] #strong[now] #strike[not].",
            ),
            (
                "Use `kuria ls` and 1 * 2 = #3 <x> @y.",
                "Use #raw(\"kuria ls\") and 1 \\* 2 \\= \\#3 \\<x\\> \\@y.",
            ),
            ("- one\n- two\n  - nested\n", "- one\n- two\n  - nested"),
            ("1. first\n2. second\n", "+ first\n+ second"),
            ("- [ ] to do\n- [x] done\n", "- ☐ to do\n- ☒ done"),
            ("> Quoted.", "#quote(block: true)[\nQuoted.\n\n]"),
            (
                "```rust\nlet x = \"a\";\n```",
                "#raw(block: true, lang: \"rust\", \"let x = \\\"a\\\";\")",
            ),
            (
                "| A | B |\n| --- | --- |\n| 1 | *2* |",
                "#table(columns: 2, [#strong[A]], [#strong[B]], [1], [#emph[2]])",
            ),
            (
                "Above\n\n---\n\nBelow",
                "Above\n\n#line(length: 100%)\n\nBelow",
            ),
            (
                "[setup](kuria:D-4#install) and [web](https://example.com)",
                "#link(\"https://kuria.example/akt/D-4#install\")[setup] and #link(\"https://example.com\")[web]",
            ),
            (
                "Said[^1], twice[^1].\n\n[^1]: By *her*.",
                "Said#footnote[By #emph[her].]<note-1>, twice#footnote(<note-1>).",
            ),
            (
                "Before[^a b] and [^gone].\n\n[^a b]: A [link](kuria:D-4#install).\n\n[^unused]: Never.",
                "Before#footnote[A #link(\"https://kuria.example/akt/D-4#install\")[link].]<note-a_20_b> and \\[^gone\\].",
            ),
            (
                "Later[^n].\n\n[^n]: Note.\n\n[^m]: Never.",
                "Later#footnote[Note.]<note-n>.",
            ),
        ];
        for (markdown, typst) in cases {
            assert_eq!(body(markdown), typst, "{markdown}");
        }
    }
}
