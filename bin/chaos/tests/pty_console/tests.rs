use super::harness::PtyConsole;
use anyhow::{Context, Result, ensure};
use core_test_support::responses::{
    ev_assistant_message, ev_completed, ev_message_item_added, ev_output_text_delta,
    ev_response_created, sse,
};
use core_test_support::streaming_sse::{
    StreamingSseChunk, StreamingSseServer, start_streaming_sse_server,
};
use serde_json::Value;
use tokio::sync::oneshot;

fn reply(id: &str, text: &str) -> Vec<StreamingSseChunk> {
    vec![StreamingSseChunk {
        gate: None,
        body: sse(vec![
            ev_response_created(id),
            ev_assistant_message(id, text),
            ev_completed(id),
        ]),
    }]
}

async fn requests(server: &StreamingSseServer) -> Result<Vec<Value>> {
    server
        .requests()
        .await
        .iter()
        .map(|body| serde_json::from_slice(body).map_err(Into::into))
        .collect()
}

fn last_user_text(request: &Value) -> Result<String> {
    let input = request["input"]
        .as_array()
        .context("missing request input")?;
    let message = input
        .iter()
        .rev()
        .find(|item| item["role"] == "user")
        .context("missing user message")?;
    let content = message["content"]
        .as_array()
        .context("missing user message content")?;
    Ok(content
        .iter()
        .filter_map(|item| item["text"].as_str())
        .collect())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resizing_a_live_reply_preserves_unicode_draft_and_exact_submission() -> Result<()> {
    let (finish_tx, finish_rx) = oneshot::channel();
    let (server, _completions) = start_streaming_sse_server(vec![
        vec![
            StreamingSseChunk {
                gate: None,
                body: sse(vec![
                    ev_response_created("stream"),
                    ev_message_item_added("stream-message", ""),
                    // A heading is stable before the message ends; trailing
                    // prose is deliberately buffered by the Markdown renderer.
                    ev_output_text_delta("# PTY_STREAM_STARTED\n\n"),
                ]),
            },
            StreamingSseChunk {
                gate: Some(finish_rx),
                body: sse(vec![
                    ev_output_text_delta("PTY_STREAM_FINISHED\n"),
                    ev_assistant_message(
                        "stream-message",
                        "# PTY_STREAM_STARTED\n\nPTY_STREAM_FINISHED\n",
                    ),
                    ev_completed("stream"),
                ]),
            },
        ],
        reply("second", "PTY_SECOND_REPLY"),
    ])
    .await;

    let result = async {
        let mut console = PtyConsole::start(&server, Some("PTY_FIRST_PROMPT")).await?;
        console.wait_screen("PTY_STREAM_STARTED").await?;
        let draft = "keep 界 🦀 e\u{301}\nsecond paragraph";
        console.paste(draft).await?;
        console.wait_screen("second paragraph").await?;

        // A storm of width and height changes while the provider is deliberately
        // paused. Completion is released by the test, never by a timing sleep.
        for _ in 0..20 {
            console.resize(18, 61).await?;
            console.resize(38, 100).await?;
        }
        console.resize_and_wait(39, 101).await?;
        console.type_text(" after resize").await?;
        console.wait_screen("second paragraph after resize").await?;
        ensure!(
            requests(&server).await?.len() == 1,
            "resizing or editing an unsent draft submitted a prompt"
        );

        finish_tx
            .send(())
            .map_err(|_| anyhow::anyhow!("provider stream closed before release"))?;
        console.wait_screen("PTY_STREAM_FINISHED").await?;
        console.wait_screen("second paragraph after resize").await?;
        console.enter().await?;
        console.wait_screen("PTY_SECOND_REPLY").await?;
        console.shutdown().await?;

        let requests = requests(&server).await?;
        ensure!(
            requests.len() == 2,
            "expected exactly two submissions: {requests:#?}"
        );
        assert_eq!(last_user_text(&requests[0])?, "PTY_FIRST_PROMPT");
        assert_eq!(
            last_user_text(&requests[1])?,
            format!("{draft} after resize")
        );
        Ok(())
    }
    .await;
    server.shutdown().await;
    result
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn large_unicode_paste_survives_resize_and_expands_exactly_once() -> Result<()> {
    let (server, _completions) =
        start_streaming_sse_server(vec![reply("paste", "PTY_PASTE_REPLY")]).await;
    let result = async {
        let mut console = PtyConsole::start(&server, None).await?;
        let draft = format!("BEGIN {}\nEND", "界 🦀 e\u{301} ".repeat(300));
        let placeholder = format!("[Pasted Content {} chars]", draft.chars().count());
        console.paste(&draft).await?;
        console.wait_screen(&placeholder).await?;
        for _ in 0..10 {
            console.resize(18, 61).await?;
            console.resize(38, 100).await?;
        }
        console.resize_and_wait(39, 101).await?;
        console.type_text(" tail").await?;
        console.wait_screen(" tail").await?;
        ensure!(
            requests(&server).await?.is_empty(),
            "paste or resize submitted before Enter"
        );
        console.enter().await?;
        console.wait_screen("PTY_PASTE_REPLY").await?;
        console.shutdown().await?;

        let requests = requests(&server).await?;
        ensure!(
            requests.len() == 1,
            "paste submitted more than once: {requests:#?}"
        );
        assert_eq!(last_user_text(&requests[0])?, format!("{draft} tail"));
        Ok(())
    }
    .await;
    server.shutdown().await;
    result
}
