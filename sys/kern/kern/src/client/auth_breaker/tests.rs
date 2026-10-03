use super::*;

#[test]
fn missing_credentials_trip_then_close_on_auth() {
    let breaker = AuthBreaker::new("test-provider");

    // A fresh breaker always probes.
    std::assert_matches!(breaker.check(), AuthGate::Probe);

    // A missing-credentials probe opens it; subsequent turns within the
    // backoff window reject without probing again.
    breaker.record(false);
    std::assert_matches!(breaker.check(), AuthGate::RejectFastFail);

    // A confirmed-authenticated probe force-closes, restoring normal
    // probing without waiting for the wall-clock backoff.
    breaker.record(true);
    std::assert_matches!(breaker.check(), AuthGate::Probe);
}

#[test]
fn reset_reopens_probing_after_login() {
    let breaker = AuthBreaker::new("test-provider-reset");
    breaker.record(false);
    std::assert_matches!(breaker.check(), AuthGate::RejectFastFail);
    breaker.reset();
    std::assert_matches!(breaker.check(), AuthGate::Probe);
}

#[test]
fn authenticated_provider_keeps_probing() {
    let breaker = AuthBreaker::new("test-provider-present");
    std::assert_matches!(breaker.check(), AuthGate::Probe);
    breaker.record(true);
    std::assert_matches!(breaker.check(), AuthGate::Probe);
}
