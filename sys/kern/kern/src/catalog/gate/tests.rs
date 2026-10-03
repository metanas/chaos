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
