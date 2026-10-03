use super::{BacktrackState, PendingBacktrackRollback};
use state_machines::state_machine;

state_machine! {
    name: BacktrackNavigation,
    dynamic: true,
    initial: Idle,
    states: [
        superstate Navigation {
            state Idle, state Primed, state Preview, state UnprimedPreview,
        }
    ],
    events {
        prime {
            transition: { from: Idle, to: Primed }
            transition: { from: Primed, to: Primed }
            transition: { from: Preview, to: Preview }
            transition: { from: UnprimedPreview, to: Preview }
        }
        preview {
            transition: { from: Idle, to: UnprimedPreview }
            transition: { from: Primed, to: Preview }
            transition: { from: Preview, to: Preview }
            transition: { from: UnprimedPreview, to: UnprimedPreview }
        }
        unprime {
            transition: { from: Idle, to: Idle }
            transition: { from: Primed, to: Idle }
            transition: { from: Preview, to: UnprimedPreview }
            transition: { from: UnprimedPreview, to: UnprimedPreview }
        }
        close_preview {
            transition: { from: Idle, to: Idle }
            transition: { from: Primed, to: Primed }
            transition: { from: Preview, to: Primed }
            transition: { from: UnprimedPreview, to: Idle }
        }
    }
}

pub(super) struct NavigationWorkflow {
    machine: DynamicBacktrackNavigation<()>,
}

impl Default for NavigationWorkflow {
    fn default() -> Self {
        Self {
            machine: DynamicBacktrackNavigation::new(()),
        }
    }
}

impl NavigationWorkflow {
    pub(super) fn primed(&self) -> bool {
        matches!(
            self.machine.current_state(),
            BacktrackNavigationState::Primed | BacktrackNavigationState::Preview
        )
    }
    pub(super) fn preview_active(&self) -> bool {
        matches!(
            self.machine.current_state(),
            BacktrackNavigationState::Preview | BacktrackNavigationState::UnprimedPreview
        )
    }
    pub(super) fn apply(&mut self, event: BacktrackNavigationEvent) {
        assert!(
            self.machine.handle(event).is_ok(),
            "backtrack navigation event is valid"
        );
    }
}

mod rollback {
    use super::*;
    state_machine! {
        name: RollbackAcknowledgement,
        dynamic: true,
        initial: Idle,
        states: [Idle, Pending],
        events {
            request { transition: { from: Idle, to: Pending } }
            resolve {
                transition: { from: Pending, to: Idle }
                transition: { from: Idle, to: Idle }
            }
        }
    }

    impl BacktrackState {
        pub(super) fn rollback_event(&mut self, event: RollbackAcknowledgementEvent) -> bool {
            let state = if self.pending_rollback.is_some() {
                RollbackAcknowledgementState::Pending
            } else {
                RollbackAcknowledgementState::Idle
            };
            DynamicRollbackAcknowledgement::new_init_state((), state)
                .handle(event)
                .is_ok()
        }
    }
}

impl BacktrackState {
    pub(crate) fn primed(&self) -> bool {
        self.navigation.primed()
    }
    pub(crate) fn preview_active(&self) -> bool {
        self.navigation.preview_active()
    }
    pub(crate) fn prime(&mut self) {
        self.navigation.apply(BacktrackNavigationEvent::Prime);
    }
    pub(crate) fn preview(&mut self) {
        self.navigation.apply(BacktrackNavigationEvent::Preview);
    }
    pub(crate) fn unprime(&mut self) {
        self.navigation.apply(BacktrackNavigationEvent::Unprime);
    }
    pub(crate) fn close_preview(&mut self) {
        self.navigation
            .apply(BacktrackNavigationEvent::ClosePreview);
    }
    pub(crate) fn request_rollback(&mut self, pending: PendingBacktrackRollback) -> bool {
        if !self.rollback_event(rollback::RollbackAcknowledgementEvent::Request) {
            return false;
        }
        self.pending_rollback = Some(pending);
        true
    }
    pub(crate) fn take_rollback(&mut self) -> Option<PendingBacktrackRollback> {
        self.rollback_event(rollback::RollbackAcknowledgementEvent::Resolve);
        self.pending_rollback.take()
    }
}

#[cfg(test)]
mod tests;
