use super::ConfiguredHandler;
use chaos_ipc::hooks::HookRegistration;

pub(crate) struct DiscoveryResult {
    pub handlers: Vec<ConfiguredHandler>,
    pub warnings: Vec<String>,
}

/// The kernel supplies a scope/trust-filtered snapshot. No filesystem discovery.
pub(crate) fn discover_handlers(registrations: &[HookRegistration]) -> DiscoveryResult {
    let mut registrations: Vec<_> = registrations
        .iter()
        .filter(|hook| hook.enabled && hook.approved)
        .collect();
    registrations.sort_by(|a, b| {
        (a.definition.project.is_some(), a.definition.order, &a.id).cmp(&(
            b.definition.project.is_some(),
            b.definition.order,
            &b.id,
        ))
    });
    let mut handlers = Vec::new();
    let mut warnings = Vec::new();
    for hook in registrations {
        let definition = &hook.definition;
        if definition.command.trim().is_empty()
            || !(1..=600).contains(&definition.timeout_sec)
            || definition
                .matcher
                .as_ref()
                .is_some_and(|m| regex::Regex::new(m).is_err())
        {
            warnings.push(format!("invalid hook definition: {}", hook.id));
            continue;
        }
        handlers.push(ConfiguredHandler {
            event_name: definition.event,
            matcher: definition.matcher.clone(),
            command: definition.command.clone(),
            timeout_sec: definition.timeout_sec,
            status_message: definition.status_message.clone(),
            // Kept as a display-only origin for the existing event wire format.
            source_path: format!("chaos://hooks/{}", hook.id).into(),
            display_order: handlers.len() as i64,
        });
    }
    DiscoveryResult { handlers, warnings }
}

#[cfg(test)]
mod tests;
