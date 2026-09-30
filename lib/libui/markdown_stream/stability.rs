//! Stable Markdown prefixes for irreversible scrollback commits.
//!
//! Resolve links against the complete source before withholding mutable blocks.
//! Definitions need not be inside the emitted prefix: they can be in a later
//! paragraph, list, or quote that is itself still waiting for more input.

use std::ops::Range;

use pulldown_cmark::{BrokenLink, Event, Options, Parser, Tag};

pub(super) fn stable_events(source: &str) -> impl Iterator<Item = Event<'_>> {
    let mut first_unresolved = source.len();
    let events: Vec<_> = if pending_metadata(source) || contains_footnotes(source) {
        Vec::new()
    } else {
        // Let pulldown-cmark identify full, collapsed, and shortcut references,
        // including images. Brackets in code or escaped text are not references.
        let mut unresolved = |link: BrokenLink<'_>| {
            first_unresolved = first_unresolved.min(link.span.start);
            None
        };
        Parser::new_with_broken_link_callback(
            source,
            crate::markdown_render::parser_options(),
            Some(&mut unresolved),
        )
        .into_offset_iter()
        .collect()
    };
    let boundary = stable_block_boundary(source, &events, first_unresolved);
    events
        .into_iter()
        .take_while(move |(_, range)| range.start < boundary)
        .map(|(event, _)| event)
}

/// A leading rule can become front matter when its closing delimiter arrives.
fn pending_metadata(source: &str) -> bool {
    let Some(marker @ ("---" | "+++")) = source.lines().next() else {
        return false;
    };
    !source
        .lines()
        .skip(1)
        .any(|line| line == marker || (marker == "---" && line == "..."))
}

fn contains_footnotes(source: &str) -> bool {
    if !source.contains("[^") {
        return false;
    }
    // GFM emits undefined footnotes as ordinary text, so it cannot by itself
    // distinguish a pending reference from a literal marker. The old footnote
    // grammar emits dangling references too, while still respecting escapes and
    // literal code/math/HTML/metadata. Use it only for conservative detection;
    // rendering always uses the normal GFM grammar.
    Parser::new_ext(
        source,
        crate::markdown_render::parser_options() | Options::ENABLE_OLD_FOOTNOTES,
    )
    .any(|event| {
        matches!(
            event,
            Event::FootnoteReference(_) | Event::Start(Tag::FootnoteDefinition(_))
        )
    })
}

/// Preserve entire enclosing blocks so styles, prefixes, and spacing stay
/// balanced. Other block kinds (notably code) retain newline-gated streaming.
fn stable_block_boundary(
    source: &str,
    events: &[(Event<'_>, Range<usize>)],
    first_unresolved: usize,
) -> usize {
    let table_boundary = crate::table_detect::table_holdback_boundary(source);
    let mut boundary = source.len();
    let mut depth = 0usize;
    let mut trailing = None;
    for (event, range) in events {
        match event {
            Event::Start(tag) => {
                if depth == 0 {
                    let mutable = match tag {
                        Tag::Paragraph
                        | Tag::List(_)
                        | Tag::BlockQuote(_)
                        | Tag::DefinitionList => true,
                        Tag::Table(_) => !source.ends_with("\n\n") && !source.ends_with("\r\n\r\n"),
                        _ => false,
                    };
                    trailing = Some((range.start, mutable));
                    // The raw table detector is deliberately conservative. Do
                    // not apply its guess inside parser-confirmed literal blocks,
                    // and never cut through the middle of a balanced event stream.
                    let pending_table = !matches!(
                        tag,
                        Tag::CodeBlock(_) | Tag::HtmlBlock | Tag::MetadataBlock(_)
                    ) && table_boundary
                        .is_some_and(|offset| range.contains(&offset));
                    if range.contains(&first_unresolved) || pending_table {
                        boundary = boundary.min(range.start);
                    }
                }
                depth += 1;
            }
            Event::End(_) => depth -= 1,
            _ if depth == 0 => trailing = Some((range.start, false)),
            _ => {}
        }
    }
    if let Some((start, true)) = trailing {
        boundary = boundary.min(start);
    }
    boundary
}
