use super::*;
use pretty_assertions::assert_eq;

pub(super) fn metadata_streaming_suite() {
    metadata_streams_like_full_render();
    pending_metadata_keeps_the_preceding_output_streaming();
    a_rule_followed_by_a_blank_line_is_not_pending_metadata();
}

fn metadata_streams_like_full_render() {
    for source in [
        "---  \ntitle: Demo\n---  \n\n# After\n",
        "+++ \ntitle = \"Demo\"\n+++ \n\n# After\n",
        "\u{feff}---\ntitle: Demo\n---\n\n# After\n",
        "# Before\n\n---\ntitle: Demo\n---\n\n# After\n",
        "# Before\n\n+++\ntitle = \"Demo\"\n+++\n\n# After\n",
        "# Before\n\n---\ntitle: Demo\n...\n\n# After\n",
        "---\n---\ntitle: Demo\n---\n\n# After\n",
        "---\t\ntitle: Demo\n...  \n\n# After\n",
        "---\ntitle: Demo\n---\n\n# Middle\n\n---\nsecond: Block\n---\n\n# After\n",
        "# Before\n\n---\ntitle: Demo\n\n# Literal heading\n---\n\n# After\n",
        "# Before\n\n> ---\n> title: Demo\n\n---\n\n# After\n",
        "# Before\n\n- ---\n  title: Demo\n\n---\n\n# After\n",
        "---\n\ntitle: Not metadata\n---\n",
        "```\n---\ntitle: Not metadata\n```\n",
        "# Before\n\n----\n\n# Not metadata\n",
        "# Before\n\n  ---\ntitle: Not metadata\n\n# After\n",
        "---\ntitle: Demo\npaths:\n  - src/lib.rs\n\n  # comment\n---\n\n# After\n",
    ] {
        for newline in ["\n", "\r\n"] {
            let source = source.replace('\n', newline);
            for width in [None, Some(12), Some(40)] {
                let cwd = super::test_cwd();
                let mut full = Vec::new();
                markdown::append_markdown(&source, width, Some(&cwd), &mut full);
                let mut collector = MarkdownStreamCollector::new(width, &cwd);
                let mut streamed = Vec::new();
                for character in source.chars() {
                    let mut bytes = [0; 4];
                    collector.push_delta(character.encode_utf8(&mut bytes));
                    streamed.extend(collector.commit_complete_lines());
                    assert!(
                        streamed.len() <= full.len(),
                        "extra lines: {source:?}, {width:?}"
                    );
                    assert_eq!(
                        streamed,
                        full[..streamed.len()],
                        "unstable metadata: {source:?}, {width:?}"
                    );
                }
                streamed.extend(collector.finalize_and_drain());
                assert_eq!(streamed, full, "{source:?}, {width:?}");
            }
        }
    }
}

fn pending_metadata_keeps_the_preceding_output_streaming() {
    for marker in ["---", "+++"] {
        let mut collector = MarkdownStreamCollector::new(None, &super::test_cwd());
        collector.push_delta(&format!("# Before\n\n{marker}  \n"));
        assert_eq!(
            collector
                .commit_complete_lines()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            ["# Before"]
        );
        collector.push_delta("title: Demo\n");
        assert!(collector.commit_complete_lines().is_empty());
        collector.push_delta(&format!("{marker}\n"));
        assert_eq!(
            collector
                .commit_complete_lines()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            ["", marker, "title: Demo", marker]
        );
        assert!(collector.finalize_and_drain().is_empty());
    }
}

fn a_rule_followed_by_a_blank_line_is_not_pending_metadata() {
    let mut collector = MarkdownStreamCollector::new(None, &super::test_cwd());
    collector.push_delta("---\n\n");
    assert_eq!(
        collector
            .commit_complete_lines()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["———"]
    );
}
