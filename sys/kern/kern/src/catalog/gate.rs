use super::CatalogMutation;
use state_machines::state_machine;

state_machine! {
    name: CatalogGate,
    dynamic: true,
    initial: Staging,
    states: [Staging, Active, Retired],
    events {
        activate { transition: { from: Staging, to: Active } }
        retire {
            transition: { from: Staging, to: Retired }
            transition: { from: Active, to: Retired }
            transition: { from: Retired, to: Retired }
        }
    }
}

pub(super) struct Gate {
    pub(super) machine: DynamicCatalogGate<()>,
    pub(super) staged: Vec<CatalogMutation>,
}

impl Default for Gate {
    fn default() -> Self {
        Self {
            machine: DynamicCatalogGate::new(()),
            staged: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests;
