//! What the preview knows and what it shows: the document in front, how wide the pane
//! is, the line the reader is on, and the links.

use trcode_plugin_api::{FromPlugin, Plugin, ToPlugin, WireLine, WireSpan, WireView};

use crate::render::{Doc, Line, render, slot};

/// The most text the preview renders, in bytes. Rendering costs a few thousand
/// instructions for each byte of a dense document and the plugin has 16 MB of memory:
/// a larger file is told as too large, which costs nothing. (The editor sends up to 1 MB.)
pub const MAX_PREVIEW_BYTES: usize = 128 * 1024;

/// The width assumed until the editor says how wide the pane is.
const DEFAULT_WIDTH: usize = 80;
const DEFAULT_HEIGHT: usize = 24;
const HEADER_SLOT: &str = "explorer.dir";
const HINT_SLOT: &str = "explorer.hint";
const WARNING_SLOT: &str = "git.modified";
const HINT: &str = "tab: next link, enter: open it, esc: back to the editor";

#[derive(Debug)]
pub struct Viewer {
    /// The file shown, from the root of the repository.
    path: Option<String>,
    /// The markdown, for drawing it again when the pane changes size.
    text: String,
    /// What the editor said instead of the text (a file that is too large).
    notice: Option<String>,
    modified: bool,
    /// Whether the file shown is the one in front in the editor.
    in_front: bool,
    doc: Doc,
    width: usize,
    height: usize,
    /// The line the reader is on.
    selected: usize,
    next_request: u64,
    /// What went wrong with a link, until something else is done.
    problem: Option<String>,
    /// An anchor to go to when the file a link opened arrives.
    pending_anchor: Option<(String, String)>,
}

impl Default for Viewer {
    fn default() -> Self {
        Self {
            path: None,
            text: String::new(),
            notice: None,
            modified: false,
            in_front: true,
            doc: Doc::default(),
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            selected: 0,
            next_request: 1,
            problem: None,
            pending_anchor: None,
        }
    }
}

impl Plugin for Viewer {
    fn handle(&mut self, message: ToPlugin) -> Vec<FromPlugin> {
        let mut out = Vec::new();
        match message {
            ToPlugin::FileText {
                path,
                text,
                modified,
            } => self.show(path, text, modified),
            ToPlugin::FileCleared => self.in_front = false,
            ToPlugin::FileTooLarge { path, bytes } => self.too_large(path, bytes),
            ToPlugin::Size { width, height } => {
                let (width, height) = (usize::from(width).max(10), usize::from(height));
                if width != self.width && !self.text.is_empty() {
                    self.width = width;
                    self.draw();
                }
                self.width = width;
                self.height = height.max(1);
            }
            ToPlugin::Action { action } => self.action(&action, &mut out),
            ToPlugin::Click { row } => {
                self.selected = row.min(self.doc.lines.len().saturating_sub(1));
                self.open_link_here(&mut out);
            }
            ToPlugin::Refused { reason, .. } => {
                self.problem = Some(match reason {
                    trcode_plugin_api::Refusal::Failed { detail }
                    | trcode_plugin_api::Refusal::BadArgument { detail } => {
                        format!("Could not open it: {detail}")
                    }
                    trcode_plugin_api::Refusal::Denied { .. } => {
                        "Opening files is not allowed for this plugin.".to_owned()
                    }
                    _ => "Could not open it.".to_owned(),
                });
            }
            _ => {}
        }
        out.push(FromPlugin::SetView { view: self.view() });
        out
    }
}

impl Viewer {
    fn show(&mut self, path: String, text: String, modified: bool) {
        if text.len() > MAX_PREVIEW_BYTES {
            self.too_large(path, text.len() as u64);
            return;
        }
        let same = self.path.as_deref() == Some(path.as_str());
        self.notice = None;
        self.modified = modified;
        self.in_front = true;
        self.text = text;
        self.path = Some(path.clone());
        self.draw();
        self.selected = match same {
            true => self.selected,
            false => 0,
        };
        // A link to a place in the file this one is the answer to.
        if let Some((wanted, anchor)) = self.pending_anchor.take()
            && wanted == path
        {
            self.go_to_anchor(&anchor);
        }
        self.selected = self.selected.min(self.doc.lines.len().saturating_sub(1));
    }

    /// A file the preview does not render: it says how large it is and what the limit is.
    fn too_large(&mut self, path: String, bytes: u64) {
        self.path = Some(path);
        self.text.clear();
        self.doc = Doc::default();
        self.selected = 0;
        self.in_front = true;
        self.modified = false;
        self.notice = Some(format!(
            "This file has {} KB: the preview shows files of up to {} KB.",
            bytes.div_ceil(1024),
            MAX_PREVIEW_BYTES / 1024
        ));
    }

    fn draw(&mut self) {
        self.doc = render(&self.text, self.width.saturating_sub(1));
        self.selected = self.selected.min(self.doc.lines.len().saturating_sub(1));
    }

    fn action(&mut self, action: &str, out: &mut Vec<FromPlugin>) {
        let last = self.doc.lines.len().saturating_sub(1);
        let page = self.height.saturating_sub(2).max(1);
        self.problem = None;
        match action {
            "up" => self.selected = self.selected.saturating_sub(1),
            "down" => self.selected = (self.selected + 1).min(last),
            "page_up" => self.selected = self.selected.saturating_sub(page),
            "page_down" => self.selected = (self.selected + page).min(last),
            "top" => self.selected = 0,
            "bottom" => self.selected = last,
            "next_link" => {
                if let Some(at) = self.link_after(self.selected) {
                    self.selected = at;
                }
            }
            "previous_link" => {
                if let Some(at) = self.link_before(self.selected) {
                    self.selected = at;
                }
            }
            "open_link" => self.open_link_here(out),
            _ => {}
        }
    }

    fn link_after(&self, line: usize) -> Option<usize> {
        (line + 1..self.doc.lines.len()).find(|&at| self.doc.lines[at].link.is_some())
    }

    fn link_before(&self, line: usize) -> Option<usize> {
        (0..line)
            .rev()
            .find(|&at| self.doc.lines[at].link.is_some())
    }

    fn go_to_anchor(&mut self, anchor: &str) {
        match self.doc.anchors.iter().find(|(name, _)| name == anchor) {
            Some((_, line)) => self.selected = *line,
            None => self.problem = Some(format!("There is no heading `#{anchor}` here.")),
        }
    }

    /// Follows the link on the line the reader is on.
    fn open_link_here(&mut self, out: &mut Vec<FromPlugin>) {
        let Some(link) = self
            .doc
            .lines
            .get(self.selected)
            .and_then(|line| line.link.clone())
        else {
            return;
        };
        self.problem = None;
        if let Some(anchor) = link.strip_prefix('#') {
            self.go_to_anchor(&decode(anchor).to_lowercase());
            return;
        }
        if link.contains("://") || link.starts_with("mailto:") {
            self.problem = Some(format!("An outside link, not opened here: {link}"));
            return;
        }
        let (file, anchor) = match link.split_once('#') {
            Some((file, anchor)) => (file, Some(anchor)),
            None => (link.as_str(), None),
        };
        let Some(base) = &self.path else {
            return;
        };
        match resolve(base, &decode(file)) {
            Some(path) => {
                if let Some(anchor) = anchor {
                    self.pending_anchor = Some((path.clone(), decode(anchor).to_lowercase()));
                }
                let request = self.next_request;
                self.next_request += 1;
                out.push(FromPlugin::OpenFile { request, path });
            }
            None => self.problem = Some(format!("That link leaves the project: {link}")),
        }
    }

    fn view(&self) -> WireView {
        let title = match (&self.path, self.in_front, self.modified) {
            (Some(path), true, true) => format!("{path} · changes not saved are not shown"),
            (Some(path), true, false) => path.clone(),
            (Some(path), false, _) => format!("{path} · not the file in front"),
            (None, ..) => "Markdown preview".to_owned(),
        };
        let title_slot = match self.modified && self.in_front {
            true => WARNING_SLOT,
            false => HEADER_SLOT,
        };
        let link = self
            .doc
            .lines
            .get(self.selected)
            .and_then(|line| line.link.clone());
        let (second, second_slot) = match (&self.problem, link) {
            (Some(problem), _) => (problem.clone(), WARNING_SLOT),
            (None, Some(link)) => (format!("→ {link}"), slot::LINK),
            (None, None) => (HINT.to_owned(), HINT_SLOT),
        };
        let body = match (&self.notice, self.doc.lines.is_empty()) {
            (Some(notice), _) => vec![text_line(notice, HINT_SLOT)],
            (None, true) => vec![text_line(
                "Open a markdown file: it is shown here when it is saved.",
                HINT_SLOT,
            )],
            (None, false) => self.doc.lines.iter().map(wire_line).collect(),
        };
        let selected = (!self.doc.lines.is_empty()).then_some(self.selected);
        WireView::Column {
            parts: vec![
                WireView::lines([
                    text_line(&title, title_slot),
                    text_line(&second, second_slot),
                ]),
                WireView::rows(body, selected),
            ],
        }
    }
}

fn text_line(text: &str, slot: &str) -> WireLine {
    WireLine::new(text, slot)
}

fn wire_line(line: &Line) -> WireLine {
    WireLine::of_spans(
        line.spans
            .iter()
            .map(|span| WireSpan::new(span.text.as_str(), span.slot)),
    )
}

/// The path of the file `link` names from `base` (the path of the file with the link,
/// from the root of the repository), or `None` when it goes above the root.
pub fn resolve(base: &str, link: &str) -> Option<String> {
    let mut parts: Vec<&str> = match link.starts_with('/') {
        true => Vec::new(),
        false => base.split('/').collect(),
    };
    if !link.starts_with('/') {
        parts.pop();
    }
    for part in link.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            name => parts.push(name),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// `%20` and the like, read back.
fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let hex = |digit: u8| (digit as char).to_digit(16).map(|d| d as u8);
        match (
            bytes[at],
            bytes.get(at + 1).copied().and_then(hex),
            bytes.get(at + 2).copied().and_then(hex),
        ) {
            (b'%', Some(high), Some(low)) => {
                out.push(high * 16 + low);
                at += 3;
            }
            (byte, ..) => {
                out.push(byte);
                at += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}
