use super::TerminalCapture;

#[test]
fn terminal_capture_answers_fragmented_cursor_queries_at_the_query_position() {
    let mut terminal = TerminalCapture::new();
    assert!(terminal.process(b"\x1b[3;5H\x1b[").is_empty());
    assert_eq!(
        terminal.process(b"6n\x1b[7;9H\x1b[6n"),
        vec![b"\x1b[3;5R".to_vec(), b"\x1b[7;9R".to_vec()]
    );
    assert!(terminal.process(b"ordinary output").is_empty());
}
