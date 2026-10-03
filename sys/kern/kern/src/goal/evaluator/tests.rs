use super::*;
use serde_json::json;

#[test]
fn all_requirements_must_pass_confidently() {
    let questions = questions(1);
    let pass = json!({"type":"choice","choice":"pass","confidence":0.95,
        "probabilities":{"pass":0.95,"fail":0.03,"unknown":0.02}});
    let mut response: SystemOneResponse = serde_json::from_value(json!({
        "model":"jev-test", "answers":{"request":pass, "criterion_0":pass}
    }))
    .expect("response");
    assert_eq!(verdict(&response, &questions), Verdict::Complete);
    response.answers.remove("request");
    assert_eq!(
        verdict(&response, &questions),
        Verdict::Unknown("invalid_evaluation")
    );
}

#[test]
fn fail_unknown_and_low_confidence_are_distinct() {
    for (choice, confidence, probability, expected) in [
        (
            "fail",
            0.95,
            0.95,
            Verdict::Incomplete(vec!["request".into()]),
        ),
        (
            "unknown",
            0.95,
            0.95,
            Verdict::Unknown("insufficient_evidence"),
        ),
        ("pass", 0.5, 0.95, Verdict::Unknown("low_confidence")),
        ("pass", 0.95, 0.5, Verdict::Unknown("low_confidence")),
    ] {
        let response = serde_json::from_value(json!({
            "model":"jev-test", "answers":{"request":{
                "type":"choice","choice":choice,"confidence":confidence,"probabilities":{choice:probability}
            }}
        })).expect("response");
        assert_eq!(verdict(&response, &questions(0)), expected);
    }
}

#[tokio::test]
async fn deadline_and_cancellation_do_not_return_success() {
    use chaos_epoll::OrCancelExt;
    use tokio_util::sync::CancellationToken;

    let pending = std::future::pending::<Result<Verdict, &'static str>>();
    assert_eq!(
        with_deadline(pending, Duration::from_millis(1)).await,
        Verdict::Unknown("evaluation_timeout"),
    );
    let token = CancellationToken::new();
    token.cancel();
    let pending = std::future::pending::<Result<Verdict, &'static str>>();
    assert!(
        with_deadline(pending, Duration::from_secs(30))
            .or_cancel(&token)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn mock_jev_transport_checks_payload_and_fails_closed() {
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    let snapshot = Checkpoint {
        state: json!({"original_user_request":"synthetic test only","host_recorded_tool_evidence":[]}),
        criteria_count: 0,
    };
    let client = JevClient::new(
        crate::default_client::build_http_client(),
        server.uri(),
        "test-key",
    )
    .with_retry(1, Duration::ZERO);
    for (status, body, expected) in [
        (
            200,
            json!({"model":"jev-test","answers":{"request":{
                "type":"choice","choice":"pass","confidence":0.95,
                "probabilities":{"pass":0.95,"fail":0.03,"unknown":0.02}
            }}}),
            Verdict::Complete,
        ),
        (
            200,
            json!({"model":"jev-test","answers":{}}),
            Verdict::Unknown("evaluator_unavailable"),
        ),
        (
            503,
            json!({"error":"private upstream details"}),
            Verdict::Unknown("evaluator_unavailable"),
        ),
    ] {
        server.reset().await;
        Mock::given(method("POST"))
            .and(path("/v1/systemone"))
            .and(header("authorization", "Bearer test-key"))
            .and(body_json(json!({
                "state":snapshot.state,"model":"jev-latest","questions":questions(0)
            })))
            .respond_with(ResponseTemplate::new(status).set_body_json(body))
            .expect(1)
            .mount(&server)
            .await;
        assert_eq!(
            with_deadline(assess(&client, &snapshot), Duration::from_secs(2)).await,
            expected,
        );
        server.verify().await;
    }
}
