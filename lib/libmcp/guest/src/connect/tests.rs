use super::*;

#[cfg(feature = "stdio")]
#[test]
fn stdio_apps_advertisement_is_opt_in_and_preserves_other_capabilities() {
    assert!(
        !stdio("server", &[])
            .into_options()
            .capabilities
            .supports_mcp_apps()
    );
    let options = stdio("server", &[])
        .enable_roots(true)
        .enable_mcp_apps()
        .into_options();
    assert!(options.capabilities.supports_mcp_apps());
    assert!(options.capabilities.roots.is_some());
}

#[cfg(feature = "http")]
#[test]
fn http_apps_advertisement_is_opt_in_and_preserves_other_capabilities() {
    assert!(
        !http("https://example.test/mcp")
            .into_options()
            .capabilities
            .supports_mcp_apps()
    );
    let options = http("https://example.test/mcp")
        .enable_sampling()
        .enable_mcp_apps()
        .into_options();
    assert!(options.capabilities.supports_mcp_apps());
    assert!(options.capabilities.sampling.is_some());
}
