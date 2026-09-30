use ratatui::text::Line;
use std::path::Path;
use std::path::PathBuf;

use crate::markdown;
use pulldown_cmark::{Event, Parser, Tag};

/// Newline-gated accumulator that commits only stable Markdown blocks/lines.
pub struct MarkdownStreamCollector {
    buffer: String,
    committed_line_count: usize,
    width: Option<usize>,
    cwd: PathBuf,
}

impl MarkdownStreamCollector {
    /// Create a collector that renders markdown using `cwd` for local file-link display.
    ///
    /// The collector snapshots `cwd` into owned state because stream commits can happen long after
    /// construction. The same `cwd` should be reused for the entire stream lifecycle; mixing
    /// different working directories within one stream would make the same link render with
    /// different path prefixes across incremental commits.
    pub fn new(width: Option<usize>, cwd: &Path) -> Self {
        Self {
            buffer: String::new(),
            committed_line_count: 0,
            width,
            cwd: cwd.to_path_buf(),
        }
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.committed_line_count = 0;
    }

    pub fn push_delta(&mut self, delta: &str) {
        tracing::trace!("push_delta: {delta:?}");
        self.buffer.push_str(delta);
    }

    /// Render the full buffer and return only the newly completed logical lines
    /// since the last commit. When the buffer does not end with a newline, the
    /// final rendered line is considered incomplete and is not emitted.
    ///
    /// A table still taking on rows is incomplete in the same sense even though
    /// its rows end in newlines: the renderer sizes columns from every row it
    /// can see, so a later row can change how earlier ones should be drawn.
    /// Those rows are withheld until the table closes or the stream finalizes.
    /// Trailing prose/list/quote blocks are also held: a later definition-list
    /// description can turn their last paragraph into a styled term. Footnote
    /// documents wait for finalization because definitions can resolve earlier
    /// references, even across otherwise-complete paragraphs.
    pub fn commit_complete_lines(&mut self) -> Vec<Line<'static>> {
        let source = self.buffer.clone();
        let last_newline_idx = source.rfind('\n');
        let source = if let Some(last_newline_idx) = last_newline_idx {
            source[..=last_newline_idx].to_string()
        } else {
            return Vec::new();
        };
        if source.contains("[^") {
            // Conservative even for a literal marker in code: never commit a
            // reference before knowing whether a later definition resolves it.
            return Vec::new();
        }
        let boundary = stable_block_boundary(&source)
            .min(crate::table_detect::table_holdback_boundary(&source).unwrap_or(source.len()));
        let source = source[..boundary].to_string();
        let mut rendered: Vec<Line<'static>> = Vec::new();
        markdown::append_markdown(&source, self.width, Some(self.cwd.as_path()), &mut rendered);
        let mut complete_line_count = rendered.len();
        if complete_line_count > 0
            && crate::render::line_utils::is_blank_line_spaces_only(
                &rendered[complete_line_count - 1],
            )
        {
            complete_line_count -= 1;
        }

        if self.committed_line_count >= complete_line_count {
            return Vec::new();
        }

        let out_slice = &rendered[self.committed_line_count..complete_line_count];

        let out = out_slice.to_vec();
        self.committed_line_count = complete_line_count;
        out
    }

    /// Finalize the stream: emit all remaining lines beyond the last commit.
    /// If the buffer does not end with a newline, a temporary one is appended
    /// for rendering.
    pub fn finalize_and_drain(&mut self) -> Vec<Line<'static>> {
        let raw_buffer = self.buffer.clone();
        let mut source: String = raw_buffer.clone();
        if !source.ends_with('\n') {
            source.push('\n');
        }
        tracing::debug!(
            raw_len = raw_buffer.len(),
            source_len = source.len(),
            "markdown finalize (raw length: {}, rendered length: {})",
            raw_buffer.len(),
            source.len()
        );
        tracing::trace!("markdown finalize (raw source):\n---\n{source}\n---");

        let mut rendered: Vec<Line<'static>> = Vec::new();
        markdown::append_markdown(&source, self.width, Some(self.cwd.as_path()), &mut rendered);

        let out = if self.committed_line_count >= rendered.len() {
            Vec::new()
        } else {
            rendered[self.committed_line_count..].to_vec()
        };

        // Reset collector state for next stream.
        self.clear();
        out
    }
}

/// A trailing paragraph may become a definition-list title. Preserve its whole
/// enclosing block so list/quote prefixes and paragraph spacing remain stable.
/// Other block kinds (notably code) keep the existing newline-gated behavior.
fn stable_block_boundary(source: &str) -> usize {
    // A leading rule can become front matter when its closing delimiter arrives.
    if let Some(marker @ ("---" | "+++")) = source.lines().next()
        && !source
            .lines()
            .skip(1)
            .any(|line| line == marker || (marker == "---" && line == "..."))
    {
        return 0;
    }
    let mut depth = 0usize;
    let mut trailing = None;
    for (event, range) in
        Parser::new_ext(source, crate::markdown_render::parser_options()).into_offset_iter()
    {
        match event {
            Event::Start(tag) => {
                if depth == 0 {
                    trailing = Some((
                        range.start,
                        match tag {
                            Tag::Paragraph
                            | Tag::List(_)
                            | Tag::BlockQuote(_)
                            | Tag::DefinitionList => true,
                            Tag::Table(_) => {
                                !source.ends_with("\n\n") && !source.ends_with("\r\n\r\n")
                            }
                            _ => false,
                        },
                    ));
                }
                depth += 1;
            }
            Event::End(_) => depth -= 1,
            _ if depth == 0 => trailing = Some((range.start, false)),
            _ => {}
        }
    }
    match trailing {
        Some((start, true)) => start,
        _ => source.len(),
    }
}

#[cfg(test)]
fn test_cwd() -> PathBuf {
    // These tests only need a stable absolute cwd; using temp_dir() avoids baking Unix- or
    // Windows-specific root semantics into the fixtures.
    std::env::temp_dir()
}

#[cfg(test)]
pub fn simulate_stream_markdown_for_tests(deltas: &[&str], finalize: bool) -> Vec<Line<'static>> {
    let mut collector = MarkdownStreamCollector::new(None, &test_cwd());
    let mut out = Vec::new();
    for d in deltas {
        collector.push_delta(d);
        if d.contains('\n') {
            out.extend(collector.commit_complete_lines());
        }
    }
    if finalize {
        out.extend(collector.finalize_and_drain());
    }
    out
}

#[cfg(test)]
pub(crate) mod tests;
