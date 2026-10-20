//! Markdown to lines of styled text, wrapped to a width. Pure: no plugin API in here.

use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use std::rc::Rc;

use unicode_width::UnicodeWidthChar;

/// The theme slots the preview is drawn in: the syntax slots of markup, so that a
/// theme that colors markdown colors the preview in the same way.
pub mod slot {
    pub const TEXT: &str = "explorer.file";
    pub const HEADING: &str = "syntax.text.title";
    pub const EMPHASIS: &str = "syntax.text.emphasis";
    pub const STRONG: &str = "syntax.text.strong";
    pub const CODE: &str = "syntax.text.literal";
    pub const LINK: &str = "syntax.text.uri";
    pub const IMAGE: &str = "syntax.text.reference";
    pub const QUOTE: &str = "syntax.comment";
    pub const MARK: &str = "syntax.punctuation";
    pub const RULE: &str = "explorer.guide";
}

/// A piece of a line with one style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub slot: &'static str,
}

/// One line of the page.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Line {
    pub spans: Vec<Span>,
    /// Where the first link on the line leads, as it is written in the document.
    pub link: Option<String>,
}

impl Line {
    /// The text of the line without its styles.
    pub fn text(&self) -> String {
        self.spans.iter().map(|span| span.text.as_str()).collect()
    }
}

/// A rendered document.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Doc {
    pub lines: Vec<Line>,
    /// The headings, as the anchor a link `#anchor` names and the line it is on.
    pub anchors: Vec<(String, usize)>,
}

/// A word of running text, with how it is styled and where it leads.
#[derive(Debug, Clone)]
struct Word {
    text: String,
    slot: &'static str,
    link: Option<Rc<str>>,
    /// Whether there was a space before it.
    space: bool,
}

/// What `render` has under way for the block it is in.
#[derive(Default)]
struct Block {
    words: Vec<Word>,
}

/// The list levels the reader is in: the next number of an ordered one.
#[derive(Debug, Clone, Copy)]
struct ListLevel {
    next: Option<u64>,
}

/// One table being collected.
#[derive(Default)]
struct Table {
    alignments: Vec<Alignment>,
    rows: Vec<Vec<String>>,
    header_rows: usize,
    row: Vec<String>,
    cell: String,
    in_head: bool,
}

struct Renderer {
    width: usize,
    doc: Doc,
    block: Block,
    /// Styles that are on, innermost last.
    styles: Vec<&'static str>,
    links: Vec<Rc<str>>,
    lists: Vec<ListLevel>,
    /// What starts the first line of the block and the lines after it: for an item
    /// its bullet and indentation, for a quote its bar.
    quotes: usize,
    item_marker: Option<String>,
    table: Option<Table>,
    code: Option<Vec<String>>,
    /// The text of a heading, for its anchor.
    heading: Option<(HeadingLevel, String)>,
    pending_space: bool,
}

/// Renders `markdown` in lines of at most `width` columns.
pub fn render(markdown: &str, width: usize) -> Doc {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES;
    let mut renderer = Renderer {
        width: width.max(10),
        doc: Doc::default(),
        block: Block::default(),
        styles: Vec::new(),
        links: Vec::new(),
        lists: Vec::new(),
        quotes: 0,
        item_marker: None,
        table: None,
        code: None,
        heading: None,
        pending_space: false,
    };
    for event in Parser::new_ext(markdown, options) {
        renderer.event(event);
    }
    renderer.finish()
}

impl Renderer {
    fn finish(mut self) -> Doc {
        self.flush();
        while self
            .doc
            .lines
            .last()
            .is_some_and(|line| line.spans.is_empty())
        {
            self.doc.lines.pop();
        }
        self.doc
    }

    fn event(&mut self, event: Event<'_>) {
        if self.table.is_some() {
            self.table_event(event);
            return;
        }
        if let Some(code) = &mut self.code {
            match event {
                Event::Text(text) => {
                    for (n, piece) in text.split('\n').enumerate() {
                        match (n, code.last_mut()) {
                            (0, Some(last)) => last.push_str(piece),
                            _ => code.push(piece.to_owned()),
                        }
                    }
                }
                Event::End(TagEnd::CodeBlock) => self.end_code(),
                _ => {}
            }
            return;
        }
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => self.text(&text),
            Event::Code(text) => self.word_run(&text, slot::CODE),
            Event::Html(text) | Event::InlineHtml(text) => self.word_run(&text, slot::QUOTE),
            Event::SoftBreak => self.pending_space = true,
            Event::HardBreak => {
                self.flush();
            }
            Event::Rule => {
                self.flush();
                let rule = "─".repeat(self.width.saturating_sub(self.prefix_width()).min(60));
                self.push_prefixed(vec![span(&rule, slot::RULE)], None);
                self.blank();
            }
            Event::TaskListMarker(done) => {
                self.item_marker = self
                    .item_marker
                    .take()
                    .map(|marker| marker.replace('•', if done { "☑" } else { "☐" }));
            }
            Event::FootnoteReference(name) => self.word_run(&format!("[{name}]"), slot::LINK),
            _ => {}
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => {}
            Tag::Heading { level, .. } => {
                self.flush();
                self.heading = Some((level, String::new()));
                self.styles.push(slot::HEADING);
            }
            Tag::BlockQuote(_) => {
                self.flush();
                self.quotes += 1;
            }
            Tag::CodeBlock(kind) => {
                self.flush();
                let _ = matches!(kind, CodeBlockKind::Fenced(_));
                self.code = Some(vec![String::new()]);
            }
            Tag::List(first) => {
                self.flush();
                self.lists.push(ListLevel { next: first });
            }
            Tag::Item => {
                self.flush();
                let marker = match self.lists.last_mut() {
                    Some(ListLevel { next: Some(n) }) => {
                        let marker = format!("{n}.");
                        *n += 1;
                        marker
                    }
                    _ => "•".to_owned(),
                };
                self.item_marker = Some(marker);
            }
            Tag::Emphasis => self.styles.push(slot::EMPHASIS),
            Tag::Strong => self.styles.push(slot::STRONG),
            Tag::Strikethrough => self.styles.push(slot::QUOTE),
            Tag::Link { dest_url, .. } => {
                self.links.push(Rc::from(dest_url.as_ref()));
                self.styles.push(slot::LINK);
            }
            Tag::Image { dest_url, .. } => {
                self.links.push(Rc::from(dest_url.as_ref()));
                self.styles.push(slot::IMAGE);
                self.word_run("[image:", slot::IMAGE);
                self.pending_space = true;
            }
            Tag::Table(alignments) => {
                self.flush();
                self.table = Some(Table {
                    alignments,
                    ..Table::default()
                });
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => {
                self.flush();
                if self.lists.is_empty() {
                    self.blank();
                }
            }
            TagEnd::Heading(level) => {
                self.styles.pop();
                let text = self
                    .heading
                    .take()
                    .map(|(_, text)| text)
                    .unwrap_or_default();
                self.flush();
                let line = self.doc.lines.len().saturating_sub(1);
                self.doc.anchors.push((slug(&text), line));
                if matches!(level, HeadingLevel::H1 | HeadingLevel::H2) {
                    let ch = if level == HeadingLevel::H1 {
                        "═"
                    } else {
                        "─"
                    };
                    let wide = text.chars().map(cell_width).sum::<usize>();
                    let rule = ch.repeat(
                        wide.clamp(1, self.width.saturating_sub(self.prefix_width()).max(1)),
                    );
                    self.push_prefixed(vec![span(&rule, slot::RULE)], None);
                }
                self.blank();
            }
            TagEnd::BlockQuote(_) => {
                self.flush();
                // The gap after the last paragraph in the quote is not part of it.
                while self.doc.lines.last().is_some_and(|line| {
                    !line.spans.is_empty() && line.spans.iter().all(|span| span.text.trim() == "▎")
                }) {
                    self.doc.lines.pop();
                }
                self.quotes = self.quotes.saturating_sub(1);
                self.blank();
            }
            TagEnd::List(_) => {
                self.flush();
                self.lists.pop();
                if self.lists.is_empty() {
                    self.blank();
                }
            }
            TagEnd::Item => self.flush(),
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {
                self.styles.pop();
            }
            TagEnd::Link => {
                self.styles.pop();
                // The link stays on the words it was on: it was copied to them.
                self.links.pop();
            }
            TagEnd::Image => {
                self.word_run("]", slot::IMAGE);
                self.styles.pop();
                self.links.pop();
            }
            _ => {}
        }
    }

    fn text(&mut self, text: &str) {
        if let Some((_, heading)) = &mut self.heading {
            heading.push_str(text);
        }
        let style = self.styles.last().copied().unwrap_or(slot::TEXT);
        self.word_run(text, style);
    }

    /// Adds `text` as words in `style`, remembering whether spaces were between them.
    fn word_run(&mut self, text: &str, style: &'static str) {
        let link = self.links.last().cloned();
        let mut space =
            std::mem::take(&mut self.pending_space) || text.starts_with(char::is_whitespace);
        for part in text.split_whitespace() {
            self.block.words.push(Word {
                text: part.to_owned(),
                slot: style,
                link: link.clone(),
                space: space && !self.block.words.is_empty(),
            });
            space = true;
        }
        if text.ends_with(char::is_whitespace) {
            self.pending_space = true;
        }
    }

    /// The columns before the text of a block: quote bars and list indentation.
    fn prefix_width(&self) -> usize {
        self.quotes * 2
            + self.lists.len().saturating_sub(1) * 2
            + if self.lists.is_empty() { 0 } else { 2 }
    }

    /// The spans that start a line: quote bars, then indentation, then `marker`.
    fn prefix(&self, marker: Option<&str>) -> Vec<Span> {
        let mut spans = Vec::new();
        for _ in 0..self.quotes {
            spans.push(span("▎ ", slot::QUOTE));
        }
        let depth = self.lists.len();
        if depth > 0 {
            let indent = " ".repeat((depth - 1) * 2);
            match marker {
                Some(marker) => {
                    spans.push(span(&indent, slot::TEXT));
                    spans.push(span(marker, slot::MARK));
                    spans.push(span(" ", slot::TEXT));
                }
                None => spans.push(span(&format!("{indent}  "), slot::TEXT)),
            }
        }
        spans
    }

    /// Wraps the words of the block into lines and starts a new block.
    fn flush(&mut self) {
        let words = std::mem::take(&mut self.block.words);
        self.pending_space = false;
        let marker = self.item_marker.take();
        if words.is_empty() {
            // An item with nothing in it still shows its bullet.
            if let Some(marker) = marker {
                self.doc.lines.push(Line {
                    spans: self.prefix(Some(&marker)),
                    link: None,
                });
            }
            return;
        }
        let first = self.prefix(marker.as_deref());
        let rest = self.prefix(None);
        let room = self
            .width
            .saturating_sub(spans_width(&first).max(spans_width(&rest)))
            .max(4);
        let mut lines: Vec<Line> = Vec::new();
        let mut current = Line::default();
        let mut used = 0;
        for word in words {
            let mut text = word.text.as_str();
            loop {
                let wide = text.chars().map(cell_width).sum::<usize>();
                let gap = usize::from(used > 0 && word.space);
                if used > 0 && used + gap + wide > room {
                    lines.push(std::mem::take(&mut current));
                    used = 0;
                    continue;
                }
                if wide > room {
                    // A word longer than a line is cut where the line ends.
                    let (head, tail) = split_at_width(text, room);
                    push_word(&mut current, head, &word, used > 0 && word.space);
                    lines.push(std::mem::take(&mut current));
                    used = 0;
                    text = tail;
                    continue;
                }
                push_word(&mut current, text, &word, used > 0 && word.space);
                used += gap + wide;
                break;
            }
        }
        lines.push(current);
        for (n, mut line) in lines.into_iter().enumerate() {
            let mut spans = match n {
                0 => first.clone(),
                _ => rest.clone(),
            };
            spans.append(&mut line.spans);
            self.doc.lines.push(Line {
                spans,
                link: line.link,
            });
        }
    }

    fn push_prefixed(&mut self, spans: Vec<Span>, link: Option<String>) {
        let mut all = self.prefix(None);
        all.extend(spans);
        self.doc.lines.push(Line { spans: all, link });
    }

    /// An empty line between blocks, but not two, and not at the start.
    fn blank(&mut self) {
        let last_empty = self
            .doc
            .lines
            .last()
            .is_none_or(|line| line.spans.is_empty());
        if !last_empty {
            // Inside a quote the bar goes on through the gap.
            let spans = match self.quotes {
                0 => Vec::new(),
                _ => self.prefix(None),
            };
            self.doc.lines.push(Line { spans, link: None });
        }
    }

    fn end_code(&mut self) {
        let lines = self.code.take().unwrap_or_default();
        let room = self.width.saturating_sub(self.prefix_width() + 2).max(4);
        let mut lines = lines;
        if lines.last().is_some_and(|line| line.is_empty()) {
            lines.pop();
        }
        for line in lines {
            let (shown, cut) = match split_at_width(&line.replace('\t', "    "), room) {
                (head, "") => (head.to_owned(), false),
                (head, _) => (format!("{}…", split_at_width(head, room - 1).0), true),
            };
            let _ = cut;
            self.push_prefixed(vec![span(&format!("  {shown}"), slot::CODE)], None);
        }
        self.blank();
    }

    fn table_event(&mut self, event: Event<'_>) {
        let Some(table) = &mut self.table else {
            return;
        };
        match event {
            Event::Start(Tag::TableHead) => table.in_head = true,
            Event::End(TagEnd::TableHead) => {
                table.in_head = false;
                table.header_rows = 1;
                table.rows.push(std::mem::take(&mut table.row));
            }
            Event::End(TagEnd::TableRow) => table.rows.push(std::mem::take(&mut table.row)),
            Event::End(TagEnd::TableCell) => table
                .row
                .push(std::mem::take(&mut table.cell).trim().to_owned()),
            Event::Text(text) | Event::Code(text) | Event::Html(text) => table.cell.push_str(&text),
            Event::SoftBreak | Event::HardBreak => table.cell.push(' '),
            Event::End(TagEnd::Table) => self.end_table(),
            _ => {}
        }
    }

    fn end_table(&mut self) {
        let Some(table) = self.table.take() else {
            return;
        };
        let columns = table.rows.iter().map(Vec::len).max().unwrap_or(0);
        if columns == 0 {
            return;
        }
        let measure = |text: &str| text.chars().map(cell_width).sum::<usize>();
        let mut widths: Vec<usize> = (0..columns)
            .map(|c| {
                table
                    .rows
                    .iter()
                    .filter_map(|row| row.get(c))
                    .map(|cell| measure(cell))
                    .max()
                    .unwrap_or(1)
                    .max(1)
            })
            .collect();
        // Borders and a space on each side of a cell.
        let overhead = columns * 3 + 1;
        let room = self.width.saturating_sub(self.prefix_width());
        // The widest column gives up room until the table fits, to 3 columns at least.
        while widths.iter().sum::<usize>() + overhead > room {
            let Some(widest) = (0..columns).max_by_key(|&c| widths[c]) else {
                break;
            };
            if widths[widest] <= 3 {
                break;
            }
            widths[widest] -= 1;
        }
        let line_of = |left: &str, mid: &str, right: &str, fill: &str| {
            let cells: Vec<String> = widths.iter().map(|w| fill.repeat(w + 2)).collect();
            format!("{left}{}{right}", cells.join(mid))
        };
        self.push_prefixed(vec![span(&line_of("┌", "┬", "┐", "─"), slot::RULE)], None);
        for (n, row) in table.rows.iter().enumerate() {
            let head = n < table.header_rows;
            let mut spans = vec![span("│", slot::RULE)];
            for (c, width) in widths.iter().enumerate() {
                let cell = row.get(c).map(String::as_str).unwrap_or("");
                let shown = fit(cell, *width);
                let pad = width.saturating_sub(measure(&shown));
                let (left, right) = match table.alignments.get(c) {
                    Some(Alignment::Right) => (pad, 0),
                    Some(Alignment::Center) => (pad / 2, pad - pad / 2),
                    _ => (0, pad),
                };
                let style = if head { slot::STRONG } else { slot::TEXT };
                spans.push(span(
                    &format!(" {}{shown}{} ", " ".repeat(left), " ".repeat(right)),
                    style,
                ));
                spans.push(span("│", slot::RULE));
            }
            self.push_prefixed(spans, None);
            if head {
                self.push_prefixed(vec![span(&line_of("├", "┼", "┤", "─"), slot::RULE)], None);
            }
        }
        self.push_prefixed(vec![span(&line_of("└", "┴", "┘", "─"), slot::RULE)], None);
        self.blank();
    }
}

fn span(text: &str, slot: &'static str) -> Span {
    Span {
        text: text.to_owned(),
        slot,
    }
}

fn spans_width(spans: &[Span]) -> usize {
    spans
        .iter()
        .flat_map(|span| span.text.chars())
        .map(cell_width)
        .sum()
}

/// Adds `text` to `line`, with a space before it if `space`, joining it to the span
/// before when the style is the same.
fn push_word(line: &mut Line, text: &str, word: &Word, space: bool) {
    if line.link.is_none() {
        line.link = word.link.as_deref().map(str::to_owned);
    }
    // The gap between two words is in the style of both when they share one, else
    // plain, so that a style that shows on spaces does not run on past its words.
    if space {
        let gap = match line.spans.last() {
            Some(last) if last.slot == word.slot => word.slot,
            _ => slot::TEXT,
        };
        add(line, " ", gap);
    }
    add(line, text, word.slot);
}

/// Adds `text` in `style`, joined to the span before it when that has the same style.
fn add(line: &mut Line, text: &str, style: &'static str) {
    match line.spans.last_mut() {
        Some(last) if last.slot == style => last.text.push_str(text),
        _ => line.spans.push(span(text, style)),
    }
}

/// `text` cut after at most `width` columns: what fits and the rest.
fn split_at_width(text: &str, width: usize) -> (&str, &str) {
    let mut used = 0;
    for (at, c) in text.char_indices() {
        let w = cell_width(c);
        if used + w > width {
            return (&text[..at], &text[at..]);
        }
        used += w;
    }
    (text, "")
}

/// `text` shortened to `width` columns with an ellipsis when it does not fit.
fn fit(text: &str, width: usize) -> String {
    match split_at_width(text, width) {
        (head, "") => head.to_owned(),
        (_, _) if width == 0 => String::new(),
        _ => format!("{}…", split_at_width(text, width - 1).0),
    }
}

/// The anchor GitHub makes of a heading: lower case, spaces as `-`, no punctuation.
pub fn slug(heading: &str) -> String {
    heading
        .trim()
        .to_lowercase()
        .chars()
        .filter_map(|c| match c {
            c if c.is_alphanumeric() || c == '-' || c == '_' => Some(c),
            ' ' => Some('-'),
            _ => None,
        })
        .collect()
}

/// How many columns `c` takes. Plain ASCII is looked at first: it is nearly all of a
/// document, and the table lookup of the rest costs a good deal in WebAssembly.
fn cell_width(c: char) -> usize {
    match c.is_ascii() {
        true => usize::from(c >= ' ' && c != '\x7f'),
        false => c.width().unwrap_or(0),
    }
}
