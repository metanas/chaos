use super::Focus;
use state_machines::state_machine;

mod focus {
    use super::*;
    state_machine! {
        name: QuestionFocus,
        dynamic: true,
        initial: Options,
        states: [superstate Field { state Options, state Notes }],
        events {
            options { transition: { from: Field, to: Options } }
            notes { transition: { from: Field, to: Notes } }
        }
    }

    impl Focus {
        pub(super) fn apply(&mut self, event: QuestionFocusEvent) {
            let state = match self {
                Self::Options => QuestionFocusState::Options,
                Self::Notes => QuestionFocusState::Notes,
            };
            let mut machine = DynamicQuestionFocus::new_init_state((), state);
            assert!(
                machine.handle(event).is_ok(),
                "question focus event is valid"
            );
            *self = match machine.current_state() {
                QuestionFocusState::Options => Self::Options,
                QuestionFocusState::Notes => Self::Notes,
            };
        }
    }
}

impl Focus {
    pub(super) fn options(&mut self) {
        self.apply(focus::QuestionFocusEvent::Options);
    }
    pub(super) fn notes(&mut self) {
        self.apply(focus::QuestionFocusEvent::Notes);
    }
}

state_machine! {
    name: InputRequest,
    dynamic: true,
    initial: Editing,
    states: [Editing, Confirming, Submitting, Done],
    events {
        confirm { transition: { from: Editing, to: Confirming } }
        back {
            transition: { from: Confirming, to: Editing }
            transition: { from: Editing, to: Editing }
        }
        submit {
            transition: { from: Editing, to: Submitting }
            transition: { from: Confirming, to: Submitting }
        }
        next {
            transition: { from: Submitting, to: Editing }
            transition: { from: Editing, to: Editing }
        }
        finish {
            transition: { from: Submitting, to: Done }
            transition: { from: Editing, to: Done }
            transition: { from: Confirming, to: Done }
            transition: { from: Done, to: Done }
        }
    }
}

pub(super) struct RequestLifecycle {
    machine: DynamicInputRequest<()>,
}

impl Default for RequestLifecycle {
    fn default() -> Self {
        Self {
            machine: DynamicInputRequest::new(()),
        }
    }
}

impl RequestLifecycle {
    pub(super) fn done(&self) -> bool {
        self.machine.current_state() == InputRequestState::Done
    }
    pub(super) fn confirming(&self) -> bool {
        self.machine.current_state() == InputRequestState::Confirming
    }
    pub(super) fn apply(&mut self, event: InputRequestEvent) -> bool {
        self.machine.handle(event).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queued_requests_reopen_editing_but_finished_requests_cannot_submit_twice() {
        let mut request = RequestLifecycle::default();
        assert!(request.apply(InputRequestEvent::Confirm));
        assert!(request.confirming());
        assert!(request.apply(InputRequestEvent::Back));
        assert!(request.apply(InputRequestEvent::Submit));
        assert!(!request.apply(InputRequestEvent::Submit));
        assert!(request.apply(InputRequestEvent::Next));
        assert!(request.apply(InputRequestEvent::Submit));
        assert!(request.apply(InputRequestEvent::Finish));
        assert!(request.done());
        assert!(!request.apply(InputRequestEvent::Submit));
        assert!(!request.apply(InputRequestEvent::Next));
    }
}
