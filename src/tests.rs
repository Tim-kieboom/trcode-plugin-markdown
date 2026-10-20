use trcode_plugin_api::{FromPlugin, Plugin, Refusal, ToPlugin, WireView};

use crate::render::{Doc, slot, slug};
use crate::viewer::resolve;
use crate::{Viewer, render};

fn text_of(doc: &Doc) -> Vec<String> {
    doc.lines.iter().map(|line| line.text()).collect()
}

// --- rendering ---

#[test]
fn a_heading_is_one_line_with_a_rule_under_it_and_an_anchor() {
    let doc = render("# Hello World\n\ntext\n", 40);
    assert_eq!(text_of(&doc), ["Hello World", "═══════════", "", "text"]);
    assert_eq!(doc.lines[0].spans[0].slot, slot::HEADING);
    assert_eq!(doc.anchors, [("hello-world".to_owned(), 0)]);
    let second = render("## Part\n", 40);
    assert_eq!(text_of(&second), ["Part", "────"]);
}

#[test]
fn a_paragraph_wraps_at_the_width_on_spaces() {
    let doc = render("one two three four five six", 12);
    assert_eq!(text_of(&doc), ["one two", "three four", "five six"]);
    assert!(
        doc.lines
            .iter()
            .all(|line| line.text().chars().count() <= 12)
    );
    // A soft break in the source is a space.
    assert_eq!(text_of(&render("one\ntwo", 40)), ["one two"]);
    // A hard break is a new line.
    assert_eq!(text_of(&render("one  \ntwo", 40)), ["one", "two"]);
}

#[test]
fn a_word_longer_than_a_line_is_cut() {
    let doc = render("abcdefghijklmnopqrstuvwxyz", 10);
    assert_eq!(text_of(&doc), ["abcdefghij", "klmnopqrst", "uvwxyz"]);
}

#[test]
fn wide_characters_count_two_columns() {
    let doc = render("日本語 日本語 日本語", 10);
    assert_eq!(text_of(&doc), ["日本語", "日本語", "日本語"]);
}

#[test]
fn emphasis_strong_code_and_strikethrough_have_their_own_slots() {
    let doc = render("a *b* **c** `d` ~~e~~ f", 40);
    let slots: Vec<(&str, &str)> = doc.lines[0]
        .spans
        .iter()
        .map(|span| (span.text.as_str(), span.slot))
        .collect();
    assert_eq!(
        slots,
        [
            ("a ", slot::TEXT),
            ("b", slot::EMPHASIS),
            (" ", slot::TEXT),
            ("c", slot::STRONG),
            (" ", slot::TEXT),
            ("d", slot::CODE),
            (" ", slot::TEXT),
            ("e", slot::QUOTE),
            (" f", slot::TEXT),
        ]
    );
}

#[test]
fn a_link_shows_its_words_and_is_found_on_its_line() {
    let doc = render("See [the guide](docs/guide.md#setup) now\n\nplain", 60);
    assert_eq!(doc.lines[0].text(), "See the guide now");
    assert_eq!(doc.lines[0].link.as_deref(), Some("docs/guide.md#setup"));
    assert_eq!(doc.lines[2].link, None);
    let span = doc.lines[0]
        .spans
        .iter()
        .find(|span| span.text.contains("guide"))
        .unwrap();
    assert_eq!(span.slot, slot::LINK);
}

#[test]
fn an_image_is_a_line_that_says_so() {
    let doc = render("![a cat](cat.png)", 40);
    assert_eq!(doc.lines[0].text(), "[image: a cat]");
}

#[test]
fn lists_are_bulleted_numbered_nested_and_hang_when_wrapped() {
    let doc = render("- one\n- two\n  - inner\n\n1. first\n2. second\n", 40);
    assert_eq!(
        text_of(&doc),
        ["• one", "• two", "  • inner", "", "1. first", "2. second"]
    );
    let wrapped = render("- aaa bbb ccc ddd", 10);
    assert_eq!(text_of(&wrapped), ["• aaa bbb", "  ccc ddd"]);
}

#[test]
fn task_list_items_have_a_box() {
    let doc = render("- [x] done\n- [ ] todo\n", 40);
    assert_eq!(text_of(&doc), ["☑ done", "☐ todo"]);
}

#[test]
fn a_code_block_is_kept_as_it_is_and_cut_where_it_does_not_fit() {
    let doc = render("```rust\nfn main() {\n    println!(\"hi\");\n}\n```\n", 12);
    assert_eq!(doc.lines[0].text(), "  fn main()…");
    assert_eq!(doc.lines[1].text(), "      print…");
    assert_eq!(doc.lines[2].text(), "  }");
    assert!(
        doc.lines
            .iter()
            .all(|line| line.spans.iter().all(|s| s.slot == slot::CODE))
    );
}

#[test]
fn a_quote_has_a_bar_in_front_of_each_line() {
    let doc = render("> one two three\n\nafter", 10);
    assert_eq!(text_of(&doc), ["▎ one two", "▎ three", "", "after"]);
}

#[test]
fn a_rule_is_a_line_and_a_table_is_drawn_with_aligned_columns() {
    let rule = render("a\n\n---\n\nb", 20);
    assert!(rule.lines.iter().any(|line| line.text().starts_with("──")));

    let table = render(
        "| Name | Qty |\n|:-----|----:|\n| apple | 3 |\n| fig | 12 |\n",
        40,
    );
    assert_eq!(
        text_of(&table),
        [
            "┌───────┬─────┐",
            "│ Name  │ Qty │",
            "├───────┼─────┤",
            "│ apple │   3 │",
            "│ fig   │  12 │",
            "└───────┴─────┘",
        ]
    );
}

#[test]
fn a_table_wider_than_the_pane_shrinks_its_widest_column() {
    let table = render(
        "| a | b |\n|---|---|\n| short | a very long cell indeed here |\n",
        24,
    );
    for line in text_of(&table) {
        assert!(line.chars().count() <= 24, "{line}");
    }
    assert!(text_of(&table).iter().any(|line| line.contains('…')));
}

#[test]
fn an_empty_document_has_no_lines() {
    assert!(render("", 40).lines.is_empty());
    assert!(render("\n\n", 40).lines.is_empty());
}

#[test]
fn the_anchor_of_a_heading_is_made_like_githubs() {
    assert_eq!(slug("Hello, World!"), "hello-world");
    assert_eq!(slug("  Under_score 2 "), "under_score-2");
}

// --- the viewer ---

struct Pane {
    viewer: Viewer,
    said: Vec<FromPlugin>,
}

impl Pane {
    fn new() -> Self {
        let mut pane = Self {
            viewer: Viewer::default(),
            said: Vec::new(),
        };
        pane.hear(ToPlugin::Start);
        pane
    }

    fn hear(&mut self, message: ToPlugin) {
        self.said = self.viewer.handle(message);
    }

    fn file(&mut self, path: &str, text: &str) {
        self.hear(ToPlugin::FileText {
            path: path.to_owned(),
            text: text.to_owned(),
            modified: false,
        });
    }

    fn act(&mut self, action: &str) {
        self.hear(ToPlugin::Action {
            action: action.to_owned(),
        });
    }

    /// The header lines and the rows with the one that is selected.
    fn view(&self) -> (Vec<String>, Vec<String>, Option<usize>) {
        let Some(FromPlugin::SetView {
            view: WireView::Column { parts },
        }) = self.said.last()
        else {
            panic!("a view with a header and rows");
        };
        let [
            WireView::Lines { lines: head },
            WireView::Rows { lines, selected },
        ] = &parts[..]
        else {
            panic!("lines then rows");
        };
        let text = |line: &trcode_plugin_api::WireLine| match line.spans.is_empty() {
            true => line.text.clone(),
            false => line.spans.iter().map(|span| span.text.as_str()).collect(),
        };
        (
            head.iter().map(text).collect(),
            lines.iter().map(text).collect(),
            *selected,
        )
    }

    fn requests(&self) -> Vec<(u64, String)> {
        self.said
            .iter()
            .filter_map(|message| match message {
                FromPlugin::OpenFile { request, path } => Some((*request, path.clone())),
                _ => None,
            })
            .collect()
    }
}

const GUIDE: &str = "# Guide\n\nSee [setup](docs/setup.md#install) and [home](#guide) and [web](https://example.org).\n\n[far](other/readme.md)\n\n## Install\n\nrun it\n";

#[test]
fn before_a_file_it_says_what_it_is_for() {
    let pane = Pane::new();
    let (head, rows, selected) = pane.view();
    assert_eq!(head[0], "Markdown preview");
    assert!(rows[0].contains("Open a markdown file"));
    assert_eq!(selected, None);
}

#[test]
fn a_file_is_shown_with_its_path() {
    let mut pane = Pane::new();
    pane.file("README.md", "# Hi\n\nthere");
    let (head, rows, selected) = pane.view();
    assert_eq!(head[0], "README.md");
    assert_eq!(rows, ["Hi", "══", "", "there"]);
    assert_eq!(selected, Some(0));
}

#[test]
fn the_pane_width_wraps_the_text_and_a_new_width_wraps_it_again() {
    let mut pane = Pane::new();
    pane.hear(ToPlugin::Size {
        width: 21,
        height: 10,
    });
    pane.file("a.md", "one two three four five six seven");
    // One column is left free for the edge.
    assert_eq!(pane.view().1, ["one two three four", "five six seven"]);
    pane.hear(ToPlugin::Size {
        width: 11,
        height: 10,
    });
    assert_eq!(
        pane.view().1,
        ["one two", "three four", "five six", "seven"]
    );
}

#[test]
fn keys_move_the_line_and_stop_at_the_ends() {
    let mut pane = Pane::new();
    pane.hear(ToPlugin::Size {
        width: 40,
        height: 6,
    });
    pane.file("a.md", "1\n\n2\n\n3\n\n4\n\n5\n\n6\n\n7\n");
    pane.act("up");
    assert_eq!(pane.view().2, Some(0));
    pane.act("down");
    assert_eq!(pane.view().2, Some(1));
    pane.act("page_down");
    assert_eq!(pane.view().2, Some(5));
    pane.act("bottom");
    let last = pane.view().1.len() - 1;
    assert_eq!(pane.view().2, Some(last));
    pane.act("down");
    assert_eq!(pane.view().2, Some(last));
    pane.act("page_up");
    assert_eq!(pane.view().2, Some(last - 4));
    pane.act("top");
    assert_eq!(pane.view().2, Some(0));
}

#[test]
fn tab_goes_between_the_lines_with_links_and_the_header_says_where_one_leads() {
    let mut pane = Pane::new();
    pane.file("README.md", GUIDE);
    pane.act("next_link");
    let (head, rows, selected) = pane.view();
    assert!(rows[selected.unwrap()].contains("setup"));
    assert_eq!(head[1], "→ docs/setup.md#install");
    pane.act("next_link");
    assert!(pane.view().1[pane.view().2.unwrap()].contains("far"));
    pane.act("previous_link");
    assert!(pane.view().1[pane.view().2.unwrap()].contains("setup"));
    pane.act("previous_link");
    assert!(
        pane.view().1[pane.view().2.unwrap()].contains("setup"),
        "no link before it"
    );
}

#[test]
fn a_link_to_a_file_asks_the_editor_to_open_it_from_the_files_own_folder() {
    let mut pane = Pane::new();
    pane.file("README.md", GUIDE);
    pane.act("next_link");
    pane.act("open_link");
    assert_eq!(pane.requests(), [(1, "docs/setup.md".to_owned())]);

    // The link in a file in a folder is from that folder.
    let mut nested = Pane::new();
    nested.file("docs/guide.md", "[up](../README.md) [sibling](./b.md)");
    nested.act("next_link");
    nested.hear(ToPlugin::Click { row: 0 });
    assert_eq!(nested.requests(), [(1, "README.md".to_owned())]);
}

#[test]
fn an_anchor_in_the_file_goes_to_the_heading_and_one_in_another_file_after_it_opens() {
    let mut pane = Pane::new();
    pane.file(
        "README.md",
        "[down](#install)\n\ntext\n\n## Install\n\nrun\n",
    );
    pane.hear(ToPlugin::Click { row: 0 });
    let (_, rows, selected) = pane.view();
    assert_eq!(rows[selected.unwrap()], "Install");
    assert!(pane.requests().is_empty());

    let mut across = Pane::new();
    across.file("README.md", GUIDE);
    across.act("next_link");
    across.act("open_link");
    assert_eq!(across.requests().len(), 1);
    // The editor opens the file and sends its text: the pane is at the heading.
    across.file("docs/setup.md", "# Setup\n\ntext\n\n## Install\n\nrun\n");
    let (_, rows, selected) = across.view();
    assert_eq!(rows[selected.unwrap()], "Install");
}

#[test]
fn an_outside_link_a_link_out_of_the_project_and_a_missing_heading_say_so() {
    let mut pane = Pane::new();
    pane.file(
        "a.md",
        "[web](https://example.org)\n\n[out](../../x.md)\n\n[none](#nowhere)",
    );
    pane.hear(ToPlugin::Click { row: 0 });
    assert!(
        pane.view().0[1].contains("An outside link"),
        "{:?}",
        pane.view().0
    );
    pane.hear(ToPlugin::Click { row: 2 });
    assert!(pane.view().0[1].contains("leaves the project"));
    pane.hear(ToPlugin::Click { row: 4 });
    assert!(pane.view().0[1].contains("no heading `#nowhere`"));
    assert!(pane.requests().is_empty());
}

#[test]
fn a_refused_open_is_said_in_the_header_until_the_next_key() {
    let mut pane = Pane::new();
    pane.file("a.md", "[x](gone.md)");
    pane.hear(ToPlugin::Click { row: 0 });
    pane.hear(ToPlugin::Refused {
        request: 1,
        reason: Refusal::Failed {
            detail: "`gone.md` is not a file".to_owned(),
        },
    });
    assert!(pane.view().0[1].contains("`gone.md` is not a file"));
    pane.act("down");
    assert!(!pane.view().0[1].contains("not a file"));
}

#[test]
fn saving_again_keeps_the_place_and_another_file_starts_at_the_top() {
    let mut pane = Pane::new();
    pane.hear(ToPlugin::Size {
        width: 40,
        height: 6,
    });
    pane.file("a.md", "1\n\n2\n\n3\n\n4\n");
    pane.act("bottom");
    let at = pane.view().2;
    pane.file("a.md", "1\n\n2\n\n3\n\n4\n");
    assert_eq!(pane.view().2, at);
    pane.file("b.md", "x\n\ny\n");
    assert_eq!(pane.view().2, Some(0));
}

#[test]
fn it_says_when_the_file_is_not_the_one_in_front_or_has_unsaved_changes() {
    let mut pane = Pane::new();
    pane.hear(ToPlugin::FileText {
        path: "a.md".to_owned(),
        text: "hi".to_owned(),
        modified: true,
    });
    assert!(pane.view().0[0].contains("changes not saved are not shown"));
    pane.hear(ToPlugin::FileCleared);
    assert!(pane.view().0[0].contains("not the file in front"));
    assert_eq!(pane.view().1, ["hi"], "what was shown stays");
}

#[test]
fn a_file_that_is_too_large_says_so() {
    let mut pane = Pane::new();
    pane.hear(ToPlugin::FileTooLarge {
        path: "big.md".to_owned(),
        bytes: 3 * 1024 * 1024,
    });
    let (head, rows, _) = pane.view();
    assert_eq!(head[0], "big.md");
    assert!(rows[0].contains("3072 KB"), "{rows:?}");
    assert!(rows[0].contains("up to 128 KB"), "{rows:?}");
}

#[test]
fn text_over_the_limit_is_not_rendered_and_says_so() {
    use crate::viewer::MAX_PREVIEW_BYTES;
    let mut pane = Pane::new();
    pane.file("a.md", "# small\n");
    pane.file("big.md", &"word ".repeat(MAX_PREVIEW_BYTES / 5 + 10));
    let (head, rows, selected) = pane.view();
    assert_eq!(head[0], "big.md");
    assert!(rows[0].contains("up to 128 KB"), "{rows:?}");
    assert_eq!(rows.len(), 1);
    assert_eq!(selected, None);
    // And a file at the limit is rendered.
    pane.file("ok.md", &"x".repeat(MAX_PREVIEW_BYTES));
    assert!(pane.view().1[0].starts_with("xxx"));
}

#[test]
fn links_are_resolved_from_the_folder_of_the_file() {
    assert_eq!(
        resolve("README.md", "docs/a.md").as_deref(),
        Some("docs/a.md")
    );
    assert_eq!(resolve("docs/a.md", "b.md").as_deref(), Some("docs/b.md"));
    assert_eq!(resolve("docs/a.md", "../b.md").as_deref(), Some("b.md"));
    assert_eq!(
        resolve("docs/a.md", "./x/../b.md").as_deref(),
        Some("docs/b.md")
    );
    assert_eq!(resolve("docs/a.md", "/top.md").as_deref(), Some("top.md"));
    assert_eq!(resolve("a.md", "../b.md"), None);
    assert_eq!(resolve("a.md", ""), None);
}

#[test]
fn every_message_is_answered_with_a_view() {
    let mut pane = Pane::new();
    for message in [
        ToPlugin::Refresh,
        ToPlugin::FileCleared,
        ToPlugin::Size {
            width: 30,
            height: 9,
        },
        ToPlugin::Unknown,
    ] {
        pane.hear(message);
        assert!(matches!(pane.said.last(), Some(FromPlugin::SetView { .. })));
    }
}

#[test]
fn only_a_window_of_a_long_document_is_sent_and_a_click_on_it_is_the_right_line() {
    let mut pane = Pane::new();
    pane.hear(ToPlugin::Size {
        width: 40,
        height: 10,
    });
    let text: String = (0..500).map(|n| format!("line {n}\n\n")).collect();
    pane.file("long.md", &text);
    let (_, rows, selected) = pane.view();
    assert!(rows.len() <= 60, "{}", rows.len());
    assert_eq!(selected, Some(0));

    // Far down, the window is around the line and the selection is in it.
    for _ in 0..3 {
        pane.act("page_down");
    }
    pane.act("bottom");
    let (_, rows, selected) = pane.view();
    assert!(rows.len() <= 60);
    assert_eq!(rows[selected.unwrap()], "line 499");

    // A click on a row of the window is that line of the document.
    pane.act("top");
    for _ in 0..100 {
        pane.act("down");
    }
    let (_, rows, selected) = pane.view();
    let at = selected.unwrap();
    let row = rows.len() - 2;
    pane.hear(ToPlugin::Click { row });
    let (_, after, now) = pane.view();
    assert_eq!(
        after[now.unwrap()],
        rows[row],
        "the clicked line is the one selected"
    );
    let _ = at;
}
