//! Buffered, width-aware GFM table layout.
//!
//! Adapted from tui-markdown 0.3.10, renderer/table.rs (Josh McKinney).
//! See LICENSE-tui-markdown for the upstream MIT license. ChaOS supplies its own
//! inline styles, OSC 8 metadata, indentation, and stream holdback.

use pulldown_cmark::{Alignment, Event};
use ratatui::style::Style;
use ratatui::text::{Line, Span, StyledGrapheme};

use super::writer::Writer;

impl<'a, I: Iterator<Item = Event<'a>>> Writer<'a, I> {
    pub(super) fn start_table(&mut self, alignments: Vec<Alignment>) {
        self.flush_current_line();
        if self.needs_newline {
            self.push_blank_line();
        }
        // There is no document line while cells are buffered. Do not inherit the
        // style of a previously flushed blockquote outside this table.
        self.current_line_style = if self
            .indent_stack
            .iter()
            .any(|context| context.prefix.iter().any(|span| span.content.contains('>')))
        {
            self.styles.blockquote
        } else {
            Style::default()
        };
        self.table = Some(TableBuilder::new(alignments));
        self.needs_newline = false;
    }

    pub(super) fn end_table(&mut self) {
        if let Some(table) = self.table.take() {
            let initial_width: usize = self
                .prefix_spans(self.pending_marker_line)
                .iter()
                .map(Span::width)
                .sum();
            let continuation_width: usize = self.prefix_spans(false).iter().map(Span::width).sum();
            let width = self
                .wrap_width
                .map(|width| width.saturating_sub(initial_width.max(continuation_width)));
            for line in table.render(width, self.styles.table_border) {
                self.push_line(line);
                // Cells are already wrapped. Wrapping the complete row would break its borders.
                self.current_line_preformatted = true;
            }
            // Finish before a surrounding list/quote can pop its indentation context.
            self.flush_current_line();
            self.needs_newline = true;
        }
    }
}

pub(super) struct TableBuilder {
    alignments: Vec<Alignment>,
    header: Vec<Cell>,
    rows: Vec<Vec<Cell>>,
    row: Vec<Cell>,
    cell: Cell,
}

impl TableBuilder {
    fn new(alignments: Vec<Alignment>) -> Self {
        Self {
            alignments,
            header: Vec::new(),
            rows: Vec::new(),
            row: Vec::new(),
            cell: Cell::default(),
        }
    }

    pub(super) fn push_span(&mut self, span: Span<'static>) {
        self.cell.spans.push(span);
    }

    pub(super) fn finish_cell(&mut self) {
        self.row.push(std::mem::take(&mut self.cell));
    }

    pub(super) fn finish_header(&mut self) {
        self.header = std::mem::take(&mut self.row);
    }

    pub(super) fn finish_row(&mut self) {
        self.rows.push(std::mem::take(&mut self.row));
    }

    fn render(self, width: Option<usize>, border_style: Style) -> Vec<Line<'static>> {
        // The parser normalizes body rows to the header's column count.
        let count = self.alignments.len();
        let mut widths = vec![1; count];
        let mut minimums = vec![1; count];
        for row in std::iter::once(&self.header).chain(&self.rows) {
            for (column, cell) in row.iter().enumerate() {
                widths[column] = widths[column].max(cell.width());
                minimums[column] = minimums[column].max(cell.minimum_width());
            }
        }
        if let Some(width) = width {
            // Two padding spaces and a right border per column, plus one left border.
            let budget = width.saturating_sub(3 * count + 1);
            if widths.iter().sum::<usize>() > budget {
                let natural = widths;
                widths = minimums;
                // Never split a grapheme or discard content, even in an impossibly narrow pane.
                let remaining = budget.saturating_sub(widths.iter().sum());
                for _ in 0..remaining {
                    let column = (0..count)
                        .filter(|&column| widths[column] < natural[column])
                        .min_by_key(|&column| widths[column]);
                    let Some(column) = column else { break };
                    widths[column] += 1;
                }
            }
        }
        let mut lines = vec![border(&widths, ['┌', '┬', '┐'], border_style)];
        lines.extend(render_row(
            &self.header,
            &widths,
            &self.alignments,
            Style::new().bold(),
            border_style,
        ));
        lines.push(border(&widths, ['├', '┼', '┤'], border_style));
        for row in &self.rows {
            lines.extend(render_row(
                row,
                &widths,
                &self.alignments,
                Style::default(),
                border_style,
            ));
        }
        lines.push(border(&widths, ['└', '┴', '┘'], border_style));
        lines
    }
}

#[derive(Clone, Default)]
struct Cell {
    spans: Vec<Span<'static>>,
}

impl Cell {
    fn width(&self) -> usize {
        self.spans.iter().map(Span::width).sum()
    }

    fn minimum_width(&self) -> usize {
        self.spans
            .iter()
            .flat_map(|span| span.styled_graphemes(Style::default()))
            .map(|grapheme| Span::raw(grapheme.symbol).width())
            .max()
            .unwrap_or(1)
    }

    fn wrap(&self, width: usize) -> Vec<Self> {
        if self.width() <= width {
            return vec![self.clone()];
        }
        let graphemes: Vec<_> = self
            .spans
            .iter()
            .flat_map(|span| span.styled_graphemes(Style::default()))
            .collect();
        let mut remaining = graphemes.as_slice();
        let mut lines = Vec::new();
        while !remaining.is_empty() {
            let end = cell_line_end(remaining, width);
            let (line, rest) = remaining.split_at(end);
            let mut spans: Vec<Span<'static>> = Vec::new();
            for grapheme in line {
                if let Some(span) = spans.last_mut().filter(|span| span.style == grapheme.style) {
                    span.content.to_mut().push_str(grapheme.symbol);
                } else {
                    spans.push(Span::styled(grapheme.symbol.to_owned(), grapheme.style));
                }
            }
            lines.push(Self { spans });
            // Drop whitespace at the word-wrap boundary, not whitespace inside an unwrapped cell.
            let next_word = rest
                .iter()
                .position(|grapheme| !grapheme.symbol.chars().all(char::is_whitespace))
                .unwrap_or(rest.len());
            remaining = &rest[next_word..];
        }
        lines
    }
}

fn cell_line_end(graphemes: &[StyledGrapheme<'_>], width: usize) -> usize {
    let mut used = 0;
    let mut word_boundary = None;
    for (index, grapheme) in graphemes.iter().enumerate() {
        if index > 0 && grapheme.symbol.chars().all(char::is_whitespace) {
            word_boundary = Some(index);
        }
        used += Span::raw(grapheme.symbol).width();
        if used > width && index > 0 {
            return word_boundary.unwrap_or(index);
        }
    }
    graphemes.len()
}

fn border(widths: &[usize], glyphs: [char; 3], style: Style) -> Line<'static> {
    let mut text = glyphs[0].to_string();
    for (column, width) in widths.iter().enumerate() {
        text.push_str(&"─".repeat(width + 2));
        text.push(if column + 1 == widths.len() {
            glyphs[2]
        } else {
            glyphs[1]
        });
    }
    Line::from(Span::styled(text, style))
}

fn render_row(
    cells: &[Cell],
    widths: &[usize],
    alignments: &[Alignment],
    style: Style,
    border_style: Style,
) -> Vec<Line<'static>> {
    let empty = Cell::default();
    let wrapped: Vec<_> = widths
        .iter()
        .enumerate()
        .map(|(column, &width)| cells.get(column).unwrap_or(&empty).wrap(width))
        .collect();
    let height = wrapped.iter().map(Vec::len).max().unwrap_or(1);
    (0..height)
        .map(|row| {
            let mut spans = vec![Span::styled("│", border_style)];
            for (column, width) in widths.iter().enumerate() {
                let cell = wrapped[column].get(row).unwrap_or(&empty);
                let padding = width.saturating_sub(cell.width());
                let left = match alignments[column] {
                    Alignment::Right => padding,
                    Alignment::Center => padding / 2,
                    Alignment::Left | Alignment::None => 0,
                };
                spans.push(Span::styled(" ".repeat(left + 1), style));
                spans.extend(
                    cell.spans
                        .iter()
                        .cloned()
                        .map(|span| span.patch_style(style)),
                );
                spans.push(Span::styled(" ".repeat(padding - left + 1), style));
                spans.push(Span::styled("│", border_style));
            }
            Line::from(spans)
        })
        .collect()
}
