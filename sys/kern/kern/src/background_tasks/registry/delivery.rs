//! Delivery is orthogonal to execution: a result can be read before the exit
//! observer reports completion, and origin history must precede notifications.
use chaos_ipc::background_tasks::BackgroundTask;
use state_machines::state_machine;

state_machine! {
    name: TaskDelivery,
    dynamic: true,
    initial: OriginPending,
    states: [OriginPending, Ready, OriginPendingDelivered, Delivered],
    events {
        bind_origin {
            transition: { from: OriginPending, to: OriginPending }
            transition: { from: Ready, to: OriginPending }
            transition: { from: OriginPendingDelivered, to: OriginPendingDelivered }
            transition: { from: Delivered, to: OriginPendingDelivered }
        }
        commit_origin {
            transition: { from: OriginPending, to: Ready }
            transition: { from: Ready, to: Ready }
            transition: { from: OriginPendingDelivered, to: Delivered }
            transition: { from: Delivered, to: Delivered }
        }
        deliver {
            transition: { from: OriginPending, to: OriginPendingDelivered }
            transition: { from: Ready, to: Delivered }
            transition: { from: OriginPendingDelivered, to: OriginPendingDelivered }
            transition: { from: Delivered, to: Delivered }
        }
    }
}

fn state(task: &BackgroundTask) -> TaskDeliveryState {
    match (task.ready, task.delivered) {
        (false, false) => TaskDeliveryState::OriginPending,
        (true, false) => TaskDeliveryState::Ready,
        (false, true) => TaskDeliveryState::OriginPendingDelivered,
        (true, true) => TaskDeliveryState::Delivered,
    }
}

pub(super) fn apply(task: &mut BackgroundTask, event: TaskDeliveryEvent) {
    let mut machine = DynamicTaskDelivery::new_init_state((), state(task));
    assert!(
        machine.handle(event).is_ok(),
        "task delivery transition is valid"
    );
    (task.ready, task.delivered) = match machine.current_state() {
        TaskDeliveryState::OriginPending => (false, false),
        TaskDeliveryState::Ready => (true, false),
        TaskDeliveryState::OriginPendingDelivered => (false, true),
        TaskDeliveryState::Delivered => (true, true),
    };
}

pub(super) fn pending(task: &BackgroundTask) -> bool {
    task.state.is_terminal() && task.notify && state(task) == TaskDeliveryState::Ready
}
