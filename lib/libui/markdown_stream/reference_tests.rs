use super::*;
use pretty_assertions::assert_eq;

pub(super) fn reference_streaming_suite() {
    late_reference_definitions_match_full_render();
    references_resume_once_the_destination_is_known();
    literal_footnote_markers_do_not_stall_streaming();
    actual_footnotes_still_wait_for_completion();
}

fn plain(lines: &[Line<'_>]) -> Vec<String> {
    lines.iter().map(ToString::to_string).collect()
}

fn full_render(source: &str, width: Option<usize>) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    markdown::append_markdown(source, width, Some(Path::new("/work")), &mut lines);
    lines
}

fn late_reference_definitions_match_full_render() {
    for source in [
        "Read [docs][site].\n\n# Next\n\n[site]: https://example.com/docs\n",
        "# [source]\n\n# Next\n\n[source]: /work/src/lib.rs#L12\n",
        "Read [source][].\n\n# Next\n\n[source]: /work/src/lib.rs#L12\n",
        "Read [DOCS][site].\n\n# Next\n\n[SITE]: https://example.com/docs\n",
        "Read [docs][two words].\n\n# Next\n\n[two   words]: https://example.com/docs\n",
        "| File |\n| - |\n| [source][src] |\n\n# Next\n\n[src]: /work/long_file.rs#L9\n",
        "![**diagram**][image]\n\n# Next\n\n[image]: diagram.png\n",
        "> - [source][src]\n\n# Next\n\n[src]: /work/src/lib.rs#L12\n",
        "[source][src]\n\n# Next\n\n[src]:\n  /work/src/lib.rs#L12\n",
        "[site]: https://example.com/first\n\n[site]\n\n# Next\n\n[site]: https://example.com/second\n",
        "# Intro\n\nUnresolved [source][src].\n\n# Next\n",
        "# Intro\n\n[ordinary brackets]\n\n# Next\n",
        "# Intro\n\n[docs][site]\n\n# Next\n\n[site]: https://example.com/docs\n\nTail\n",
        // Definitions can live inside a trailing block that is still withheld.
        "[source][src]\n\n# Next\n\n- pending\n\n  [src]: /work/src/lib.rs#L12\n",
        "[source][src]\n\n# Next\n\n> [src]: /work/src/lib.rs#L12\n",
        "Read [docs][site].\r\n\r\n# Next\r\n\r\n[site]: https://example.com/docs\r\n",
        "[known]: https://example.com/known\n\n# [known]\n\n[later]\n\n# Next\n\n[later]: https://example.com/later\n",
        "# Claim[^n]\n\n# Next\n\n[^n]: Evidence.\n",
        "[^n]: Evidence.\n\n# Next\n\nClaim[^n].\n",
        "> [^n]: Evidence.\n\n# Next\n\nClaim[^n].\n",
    ] {
        for width in [None, Some(12), Some(40)] {
            let full = full_render(source, width);
            let mut collector = MarkdownStreamCollector::new(width, Path::new("/work"));
            let mut streamed = Vec::new();
            for character in source.chars() {
                let mut bytes = [0; 4];
                collector.push_delta(character.encode_utf8(&mut bytes));
                streamed.extend(collector.commit_complete_lines());
                assert!(
                    streamed.len() <= full.len(),
                    "committed extra lines: width={width:?}, source={source:?}"
                );
                assert_eq!(
                    streamed,
                    full[..streamed.len()],
                    "unstable commit: width={width:?}, source={source:?}"
                );
            }
            streamed.extend(collector.finalize_and_drain());
            assert_eq!(streamed, full, "width={width:?}, source={source:?}");
        }
    }
}

fn references_resume_once_the_destination_is_known() {
    let mut collector = MarkdownStreamCollector::new(None, Path::new("/work"));
    collector.push_delta("# Intro\n\nRead [source][src].\n\n# Next\n");
    assert_eq!(plain(&collector.commit_complete_lines()), ["# Intro"]);

    collector.push_delta("\n- pending\n\n  [src]: /work/src/lib.rs#L12\n");
    let lines = collector.commit_complete_lines();
    assert_eq!(plain(&lines), ["", "Read src/lib.rs:12.", "", "# Next"]);
    let target = lines
        .iter()
        .flat_map(|line| &line.spans)
        .find(|span| span.content == "src/lib.rs:12")
        .expect("resolved reference should display the actual local target");
    assert_eq!(
        target.style.underline_color,
        Some(crate::osc8::register("file:///work/src/lib.rs"))
    );
    assert!(collector.commit_complete_lines().is_empty());
    assert_eq!(plain(&collector.finalize_and_drain()), ["", "- pending"]);
}

fn literal_footnote_markers_do_not_stall_streaming() {
    for source in [
        "```text\n[^literal]\n```\n",
        "```text\n[^literal]\n",
        "```markdown\n| H |\n| - |\n| [^literal] |\n",
        "```rust\n// [^literal]\nfn main() {}\n```\n",
        "```markdown\n[source][src]\n",
        "    [^literal]\n",
        "# `[^literal]`\n",
        "# `[source][src]`\n",
        "# \\[^literal]\n",
        "# \\[source]\n",
        "# $[^literal]$\n",
        "<pre>\n[^literal]\n</pre>\n\n# Next\n",
        "---\ntitle: '[^literal]'\n---\n\n# Next\n",
        "+++\ntitle = '[^literal]'\n+++\n\n# Next\n",
    ] {
        let mut collector = MarkdownStreamCollector::new(None, Path::new("/work"));
        collector.push_delta(source);
        let committed = collector.commit_complete_lines();
        assert_eq!(
            committed,
            full_render(source, None),
            "literal markers must not hold stable output: {source:?}"
        );
        assert!(collector.finalize_and_drain().is_empty());
    }
}

fn actual_footnotes_still_wait_for_completion() {
    for source in [
        "# Claim[^n]\n\n# Next\n",
        "[^n]: Evidence.\n\n# Next\n",
        "Claim[^n].\n\n[^n]: Evidence.\n\n# Next\n",
    ] {
        let mut collector = MarkdownStreamCollector::new(None, Path::new("/work"));
        collector.push_delta(source);
        assert!(collector.commit_complete_lines().is_empty(), "{source:?}");
        assert_eq!(collector.finalize_and_drain(), full_render(source, None));
    }
}
