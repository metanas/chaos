use super::*;
use pretty_assertions::assert_eq;
use ratatui::style::{Modifier, Style};
use ratatui::text::Text;

fn plain(text: &Text<'_>) -> Vec<String> {
    text.lines
        .iter()
        .map(std::string::ToString::to_string)
        .collect()
}

pub(super) fn backport_suite() {
    tables_preserve_cell_styles_links_and_math();
    tables_wrap_graphemes_without_losing_content();
    tables_preserve_nested_prefixes_and_spacing();
    tables_handle_empty_short_and_escaped_cells();
    footnotes_and_definitions_keep_prefixes_and_style_scope();
    image_fallbacks_capture_all_inline_content();
    metadata_attributes_and_scripts_render_as_text();
    new_blocks_do_not_restyle_following_prose();
}

fn tables_preserve_cell_styles_links_and_math() {
    let source = "| **Header** | Value |\n| --- | --- |\n\
                  | [source](/work/src/lib.rs#L12) | $\\alpha^2$ |\n\
                  | `code` | ![**image**](diagram.png) |\n";
    let text = render_markdown_text_with_width_and_cwd(source, Some(60), Some(Path::new("/work")));
    let rendered = text.to_string();
    assert!(rendered.contains("src/lib.rs:12"), "{rendered}");
    assert!(rendered.contains("α²"), "{rendered}");
    assert!(rendered.contains("[img] image"), "{rendered}");
    let target = text
        .lines
        .iter()
        .flat_map(|line| &line.spans)
        .find(|span| span.content == "src/lib.rs:12")
        .unwrap();
    assert_eq!(
        target.style.underline_color,
        Some(crate::osc8::register("file:///work/src/lib.rs"))
    );
    let header = text
        .lines
        .iter()
        .flat_map(|line| &line.spans)
        .find(|span| span.content == "Header")
        .unwrap();
    assert!(header.style.add_modifier.contains(Modifier::BOLD));
    assert!(
        text.lines
            .iter()
            .all(|line| line.width() == text.lines[0].width())
    );

    let styled = render_markdown_with_prose_style(
        source,
        Some(60),
        Some(Path::new("/work")),
        Style::new().red().italic(),
    );
    let code = styled
        .lines
        .iter()
        .flat_map(|line| &line.spans)
        .find(|span| span.content == "code")
        .unwrap();
    assert_eq!(code.style, Style::new().fg(crate::theme::accent_color()));
}

fn tables_wrap_graphemes_without_losing_content() {
    let source = "| H |\n| - |\n| **👩‍💻e\u{301}界** |\n";
    for width in [0, 1, 6, 10] {
        let text = render_markdown_text_with_width(source, Some(width));
        let content: String = text
            .lines
            .iter()
            .skip(3)
            .take(text.lines.len() - 4)
            .flat_map(|line| &line.spans)
            .filter(|span| span.style.add_modifier.contains(Modifier::BOLD))
            .map(|span| span.content.as_ref())
            .collect();
        assert_eq!(content, "👩‍💻e\u{301}界");
        assert!(
            text.lines
                .iter()
                .all(|line| line.width() == text.lines[0].width())
        );
        assert!(text.lines[0].width() <= width.max(6));
    }
    let linked = render_markdown_text_with_width_and_cwd(
        "| File |\n| --- |\n| [x](/work/long_file.rs#L9) |\n",
        Some(10),
        Some(Path::new("/work")),
    );
    let sentinel = Some(crate::osc8::register("file:///work/long_file.rs"));
    let target: String = linked
        .lines
        .iter()
        .flat_map(|line| &line.spans)
        .filter(|span| span.style.underline_color == sentinel)
        .map(|span| span.content.as_ref())
        .collect();
    assert_eq!(target, "long_file.rs:9");
}

fn tables_preserve_nested_prefixes_and_spacing() {
    for (source, first_prefix, later_prefix) in [
        ("- | H |\n  | --- |\n  | long content |\n", "- ", "  "),
        ("> | H |\n> | --- |\n> | long content |\n", "> ", "> "),
        (
            "> - | H |\n>   | --- |\n>   | long content |\n",
            "> - ",
            ">   ",
        ),
        (
            "12. | H |\n    | --- |\n    | long content |\n",
            "12. ",
            "    ",
        ),
    ] {
        let text = render_markdown_text_with_width(source, Some(20));
        let lines = plain(&text);
        assert!(
            lines[0].starts_with(&format!("{first_prefix}┌")),
            "{lines:?}"
        );
        assert!(
            lines
                .iter()
                .skip(1)
                .all(|line| line.starts_with(later_prefix)),
            "{lines:?}"
        );
        assert!(
            text.lines.iter().all(|line| line.width() <= 20),
            "{lines:?}"
        );
    }
    let text = render_markdown_text("Before\n\n| H |\n| --- |\n| x |\n\nAfter");
    let lines = plain(&text);
    assert_eq!(&lines[..2], ["Before", ""]);
    assert_eq!(&lines[lines.len() - 2..], ["", "After"]);
    let fenced = render_markdown_text("```text\n| H |\n| --- |\n```");
    assert_eq!(plain(&fenced), ["| H |", "| --- |"]);
}

fn tables_handle_empty_short_and_escaped_cells() {
    let text = render_markdown_text("| A | B |\n| --- | --- |\n| a\\|b |\n| | x | ignored |\n");
    let rendered = text.to_string();
    assert!(rendered.contains("│ a|b │   │"), "{rendered}");
    assert!(rendered.contains("│     │ x │"), "{rendered}");
    assert!(!rendered.contains("ignored"));
    assert!(
        text.lines
            .iter()
            .all(|line| line.width() == text.lines[0].width())
    );
    let header_only = render_markdown_text("| H |\n| --- |\n");
    assert_eq!(plain(&header_only), ["┌───┐", "│ H │", "├───┤", "└───┘"]);
    let html = render_markdown_text("| H |\n| - |\n| <b>a</b> |\n");
    assert!(html.to_string().contains("<b>a</b>"));
    assert!(
        html.lines
            .iter()
            .all(|line| line.width() == html.lines[0].width())
    );
}

fn footnotes_and_definitions_keep_prefixes_and_style_scope() {
    let text = render_markdown_text(
        "**Claim[^n]**.\n\n[^n]: First line\n    second line\n\n    Another paragraph.\n\nAfter\n",
    );
    assert_eq!(
        plain(&text),
        [
            "Claim[n].",
            "",
            "[n]: First line",
            "     second line",
            "     ",
            "     Another paragraph.",
            "",
            "After",
        ]
    );
    let reference = text.lines[0]
        .spans
        .iter()
        .find(|span| span.content == "[n]")
        .unwrap();
    assert!(
        reference
            .style
            .add_modifier
            .contains(Modifier::BOLD | Modifier::DIM | Modifier::ITALIC)
    );
    let last = text.lines.last().unwrap().spans.last().unwrap();
    assert_eq!(last.style, Style::default());
    let definitions =
        render_markdown_text("Term\n: First\n\n  Second paragraph.\n\nNext\n: Another\n");
    assert_eq!(
        plain(&definitions),
        [
            "Term",
            ": First",
            "  ",
            "  Second paragraph.",
            "Next",
            ": Another",
        ]
    );
    assert!(
        definitions.lines[0]
            .spans
            .iter()
            .any(|span| span.style.add_modifier.contains(Modifier::BOLD))
    );
}

fn image_fallbacks_capture_all_inline_content() {
    for (source, expected) in [
        (
            "Before ![**diagram** `x`](d.png) after",
            "Before [img] diagram x after",
        ),
        ("![](empty.png)", "[img] empty.png"),
        ("![]()", "[img]"),
        ("![first\nsecond](d.png)", "[img] first second"),
        ("![outer ![inner](i.png)](o.png)", "[img] outer [img] inner"),
        ("![<b> $\\alpha$](d.png)", "[img] <b> α"),
        (
            "[![icon](d.png)](/work/src/lib.rs#L3)",
            "/work/src/lib.rs:3",
        ),
    ] {
        assert_eq!(plain(&render_markdown_text(source)), [expected], "{source}");
    }
}

fn metadata_attributes_and_scripts_render_as_text() {
    for marker in ["---", "+++"] {
        let text = render_markdown_text(&format!("{marker}\ntitle: Demo\n{marker}\n\nBody"));
        assert_eq!(plain(&text), [marker, "title: Demo", marker, "", "Body"]);
        assert!(
            text.lines[1].spans[0]
                .style
                .add_modifier
                .contains(Modifier::DIM)
        );
        assert_eq!(text.lines[4].spans[0].style, Style::default());
    }
    let text = render_markdown_text("# Title {#id .class key=value}\n\nH ~2~ O and x ^2^");
    assert_eq!(
        plain(&text),
        ["# Title {#id .class key=value}", "", "H 2 O and x 2"]
    );
    assert!(
        text.lines[2]
            .spans
            .iter()
            .filter(|span| span.content == "2")
            .all(|span| span
                .style
                .add_modifier
                .contains(Modifier::DIM | Modifier::ITALIC))
    );
}

fn new_blocks_do_not_restyle_following_prose() {
    let text = render_markdown_with_prose_style(
        "> quote\n\n| H |\n| --- |\n| cell |\n\nAfter",
        None,
        None,
        Style::new().red(),
    );
    let cell = text
        .lines
        .iter()
        .flat_map(|line| &line.spans)
        .find(|span| span.content == "cell")
        .unwrap();
    assert_eq!(cell.style.fg, Some(ratatui::style::Color::Red));
    let after = text.lines.last().unwrap().spans.last().unwrap();
    assert_eq!(after.style, Style::new().red());
}
