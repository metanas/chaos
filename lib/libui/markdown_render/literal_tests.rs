use super::*;
use pretty_assertions::assert_eq;
use pulldown_cmark::{CodeBlockKind, Event, MetadataBlockKind, Tag, TagEnd};
use ratatui::style::{Modifier, Style};

pub(super) fn literal_block_suite() {
    metadata_preserves_lines_and_indentation();
    plain_code_preserves_lines_across_parser_chunks();
    literal_text_fragments_do_not_create_line_breaks();
}

fn plain(source: &str, width: Option<usize>) -> Vec<String> {
    render_markdown_text_with_width(source, width)
        .lines
        .iter()
        .map(ToString::to_string)
        .collect()
}

fn metadata_preserves_lines_and_indentation() {
    for marker in ["---", "+++"] {
        let source = format!(
            "{marker}\ntitle: Demo\npaths:\n  - src/lib.rs\n\n  # keep this comment\n{marker}\n\nAfter"
        );
        let expected = [
            marker,
            "title: Demo",
            "paths:",
            "  - src/lib.rs",
            "",
            "  # keep this comment",
            marker,
            "",
            "After",
        ];
        for newline in ["\r\n", "\n"] {
            let source = source.replace('\n', newline);
            for width in [None, Some(8)] {
                assert_eq!(plain(&source, width), expected, "{source:?}, {width:?}");
            }
        }

        let text = render_markdown_text(&source);
        for line in &text.lines[..7] {
            for span in &line.spans {
                assert!(span.style.add_modifier.contains(Modifier::DIM));
            }
        }
        assert_eq!(text.lines.last().unwrap().spans[0].style, Style::default());
    }
}

fn plain_code_preserves_lines_across_parser_chunks() {
    for (source, expected) in [
        ("```\nfirst\n\nsecond\n```\n", vec!["first", "", "second"]),
        (
            "```\n\n  first\n\nsecond\n\n```\n",
            vec!["", "  first", "", "second", ""],
        ),
        (
            "> ```\n> first\n> \n> second\n> ```\n",
            vec!["> first", "> ", "> second"],
        ),
        (
            "    first\n\n    second\n",
            vec!["    first", "    ", "    second"],
        ),
    ] {
        for newline in ["\r\n", "\n"] {
            let source = source.replace('\n', newline);
            assert_eq!(plain(&source, Some(4)), expected, "{source:?}");
        }
    }
}

fn literal_text_fragments_do_not_create_line_breaks() {
    let content = "first\n  café界\n\nlast\n";
    for (tag, end, expected) in [
        (
            Tag::CodeBlock(CodeBlockKind::Fenced("".into())),
            TagEnd::CodeBlock,
            vec!["first", "  café界", "", "last"],
        ),
        (
            Tag::MetadataBlock(MetadataBlockKind::YamlStyle),
            TagEnd::MetadataBlock(MetadataBlockKind::YamlStyle),
            vec!["---", "first", "  café界", "", "last", "---"],
        ),
    ] {
        for split in content
            .char_indices()
            .map(|(index, _)| index)
            .chain([content.len()])
        {
            let events = [
                Event::Start(tag.clone()),
                Event::Text(content[..split].into()),
                Event::Text(content[split..].into()),
                Event::End(end),
            ];
            let text = render_markdown_events_with_prose_style(
                events.into_iter(),
                Some(4),
                None,
                Style::default(),
            );
            let actual: Vec<_> = text.lines.iter().map(ToString::to_string).collect();
            assert_eq!(actual, expected, "{tag:?}, split={split}");
        }
    }
}
