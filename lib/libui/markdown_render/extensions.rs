//! Text-only Markdown extensions adapted from tui-markdown 0.3.10.
//!
//! See LICENSE-tui-markdown for upstream attribution. Definitions use ChaOS's
//! indentation stack; image descriptions use its styled span and hyperlink sinks.

use pulldown_cmark::{Event, MetadataBlockKind};
use ratatui::style::Style;
use ratatui::text::{Line, Span};

use super::writer::{IndentContext, Writer};

pub(super) struct PendingImage {
    destination: String,
    style: Style,
    pub(super) spans: Vec<Span<'static>>,
}

impl<'a, I: Iterator<Item = Event<'a>>> Writer<'a, I> {
    pub(super) fn footnote_reference(&mut self, label: &str) {
        if self.suppressing_local_link_label() {
            return;
        }
        if self.pending_marker_line && !self.capturing_inline() {
            self.push_line(Line::default());
        }
        let style = self
            .inline_styles
            .last()
            .copied()
            .unwrap_or_default()
            .dim()
            .italic();
        self.push_span(Span::styled(format!("[{label}]"), style));
    }

    pub(super) fn start_footnote_definition(&mut self, label: &str) {
        self.flush_current_line();
        if self.needs_newline {
            self.push_blank_line();
        }
        self.push_inline_style(Style::new().dim());
        self.start_definition_prefix(format!("[{label}]: "), Style::new().dim());
    }

    pub(super) fn end_footnote_definition(&mut self) {
        self.flush_current_line();
        self.indent_stack.pop();
        self.pop_inline_style();
        self.pending_marker_line = false;
        self.needs_newline = true;
    }

    pub(super) fn start_definition_list(&mut self) {
        self.flush_current_line();
        if self.needs_newline {
            self.push_blank_line();
        }
        self.needs_newline = false;
    }

    pub(super) fn start_definition_title(&mut self) {
        self.push_line(Line::default());
        self.push_inline_style(Style::new().bold());
        self.needs_newline = false;
    }

    pub(super) fn start_definition_description(&mut self) {
        if self.line_ends_with_local_link_target {
            // Preserve ChaOS's file-link + newline + colon convention even
            // when the parser now recognizes it as a definition list.
            self.indent_stack
                .push(IndentContext::new(Vec::new(), None, false));
            self.push_span(Span::raw(": "));
            self.line_ends_with_local_link_target = false;
            self.joined_definition_paragraph = true;
            self.needs_newline = false;
            return;
        }
        self.flush_current_line();
        self.start_definition_prefix(": ".to_string(), Style::default());
    }

    fn start_definition_prefix(&mut self, marker: String, style: Style) {
        let marker = Span::styled(marker, style);
        self.indent_stack.push(IndentContext::new(
            vec![Span::raw(" ".repeat(marker.width()))],
            Some(vec![marker]),
            false,
        ));
        self.pending_marker_line = true;
        self.needs_newline = false;
    }

    pub(super) fn end_definition_description(&mut self) {
        self.flush_current_line();
        self.indent_stack.pop();
        self.pending_marker_line = false;
        self.needs_newline = false;
    }

    pub(super) fn start_image(&mut self, destination: String) {
        if self.pending_marker_line && !self.capturing_inline() {
            self.push_line(Line::default());
        }
        self.push_inline_style(Style::new().dim().italic());
        self.images.push(PendingImage {
            destination,
            style: self.inline_styles.last().copied().unwrap_or_default(),
            spans: Vec::new(),
        });
    }

    pub(super) fn end_image(&mut self) {
        self.pop_inline_style();
        if let Some(mut image) = self.images.pop() {
            if self.suppressing_local_link_label() {
                return;
            }
            if image.spans.is_empty() && !image.destination.is_empty() {
                image
                    .spans
                    .push(Span::styled(image.destination, image.style));
            }
            let marker = if image.spans.is_empty() {
                "[img]"
            } else {
                "[img] "
            };
            self.push_span(Span::styled(marker, image.style));
            for span in image.spans {
                self.push_span(span);
            }
        }
    }

    pub(super) fn inline_capture_break(&mut self) {
        let style = self.inline_styles.last().copied().unwrap_or_default();
        self.push_span(Span::styled(" ", style));
    }

    pub(super) fn start_metadata(&mut self, kind: MetadataBlockKind) {
        self.flush_current_line();
        if self.needs_newline {
            self.push_blank_line();
        }
        self.push_inline_style(Style::new().dim());
        self.push_line(Line::from(Span::styled(
            metadata_marker(kind),
            Style::new().dim(),
        )));
        self.needs_newline = true;
    }

    pub(super) fn end_metadata(&mut self, kind: MetadataBlockKind) {
        self.push_line(Line::from(Span::styled(
            metadata_marker(kind),
            Style::new().dim(),
        )));
        self.pop_inline_style();
        self.needs_newline = true;
    }
}

fn metadata_marker(kind: MetadataBlockKind) -> &'static str {
    match kind {
        MetadataBlockKind::YamlStyle => "---",
        MetadataBlockKind::PlusesStyle => "+++",
    }
}
