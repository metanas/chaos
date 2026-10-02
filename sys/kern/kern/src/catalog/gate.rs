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
mod tests {
    use super::*;

    #[test]
    fn retirement_is_terminal_and_idempotent() {
        for activate in [false, true] {
            let mut gate = Gate::default();
            if activate {
                assert!(gate.machine.handle(CatalogGateEvent::Activate).is_ok());
            }
            assert!(gate.machine.handle(CatalogGateEvent::Retire).is_ok());
            assert!(gate.machine.handle(CatalogGateEvent::Retire).is_ok());
            assert!(gate.machine.handle(CatalogGateEvent::Activate).is_err());
            assert_eq!(gate.machine.current_state(), CatalogGateState::Retired);
        }
    }
}
