use super::{ActiveJournalWriter, PendingJournalConfig, RolloutItem};
use state_machines::state_machine;

#[derive(Debug)]
pub(super) struct PendingBatch {
    pub(super) config: PendingJournalConfig,
    pub(super) items: Vec<RolloutItem>,
}

state_machine! {
    name: JournalStorage,
    dynamic: true,
    initial: Pending,
    states: [
        Disabled,
        Pending(Option<PendingBatch>),
        Active(Option<ActiveJournalWriter>),
    ],
    events {
        connect { transition: { from: Pending, to: Active } }
        disable {
            transition: { from: Pending, to: Disabled }
            transition: { from: Active, to: Disabled }
            transition: { from: Disabled, to: Disabled }
        }
    }
}
