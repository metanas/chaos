use env_flags::env_flags;

env_flags! {
    /// Fixture path for offline tests (see client.rs).
    pub CHAOS_RS_SSE_FIXTURE: Option<&str> = None;

    /// Disable with 0; supported providers use session-local Responses WS v2.
    pub CHAOS_OPENAI_WEBSOCKET: bool = true;
}
