use state_machines::state_machine;

state_machine! {
    name: MailboxDelivery,
    dynamic: true,
    initial: CurrentTurn,
    states: [superstate Delivery { state CurrentTurn, state NextTurn }],
    events {
        answer {
            payload: bool,
            guards: [mailbox_empty],
            transition: { from: Delivery, to: NextTurn }
        }
        reopen { transition: { from: Delivery, to: CurrentTurn } }
    }
}

impl<C, S> MailboxDelivery<C, S> {
    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "state-machines guards receive event payloads by reference"
    )]
    fn mailbox_empty(&self, _ctx: &C, empty: &bool) -> bool {
        *empty
    }
}

pub(super) struct Mailbox {
    machine: DynamicMailboxDelivery<()>,
}

impl Default for Mailbox {
    fn default() -> Self {
        Self {
            machine: DynamicMailboxDelivery::new(()),
        }
    }
}

impl Mailbox {
    pub(super) fn accepts_delivery(&self) -> bool {
        self.machine.current_state() == MailboxDeliveryState::CurrentTurn
    }

    pub(super) fn answer(&mut self, empty: bool) {
        // Rejection is intentional: steered input keeps delivery open.
        let _ = self.machine.handle(MailboxDeliveryEvent::Answer(empty));
    }

    pub(super) fn reopen(&mut self) {
        assert!(
            self.machine.handle(MailboxDeliveryEvent::Reopen).is_ok(),
            "delivery can always reopen"
        );
    }
}
