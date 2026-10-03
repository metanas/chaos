use state_machines::state_machine;

state_machine! {
    name: WriterLease,
    dynamic: true,
    initial: Confirmed,
    states: [
        superstate Owned { state Confirmed, state Unconfirmed, state Expired },
        Fenced,
    ],
    events {
        refresh {
            transition: { from: Confirmed, to: Unconfirmed }
            transition: { from: Unconfirmed, to: Unconfirmed }
            transition: { from: Expired, to: Expired }
        }
        confirm { transition: { from: Unconfirmed, to: Confirmed } }
        expire { transition: { from: Owned, to: Expired } }
        reacquire { transition: { from: Expired, to: Unconfirmed } }
        fence {
            transition: { from: Owned, to: Fenced }
            transition: { from: Fenced, to: Fenced }
        }
    }
}

pub(super) struct Lease {
    machine: DynamicWriterLease<()>,
}

impl Default for Lease {
    fn default() -> Self {
        Self {
            machine: DynamicWriterLease::new(()),
        }
    }
}

impl Lease {
    pub(super) fn confirmed(&self) -> bool {
        self.machine.current_state() == WriterLeaseState::Confirmed
    }
    pub(super) fn fenced(&self) -> bool {
        self.machine.current_state() == WriterLeaseState::Fenced
    }
    pub(super) fn needs_reacquire(&self) -> bool {
        self.machine.current_state() == WriterLeaseState::Expired
    }
    pub(super) fn apply(&mut self, event: WriterLeaseEvent) {
        assert!(
            self.machine.handle(event).is_ok(),
            "journal lease operation respects ownership"
        );
    }
}

#[cfg(test)]
mod tests;
