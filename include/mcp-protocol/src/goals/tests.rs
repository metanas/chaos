use super::*;

#[test]
fn version_and_payload_are_strict() {
    assert!(Request::parse(json!({"version":2,"operation":"restore"})).is_ok());
    for invalid in [
        json!({"version":1,"operation":"restore"}),
        json!({"version":2,"operation":"complete"}),
        json!({"version":2,"operation":"restore","conversation":"another"}),
        json!({"version":2,"operation":"evaluate","revision":1,"claim":"done","evidence":[]}),
        json!({"version":2,"operation":"evaluate","revision":1,"claim":"x".repeat(8193)}),
    ] {
        assert!(Request::parse(invalid).is_err());
    }
}
