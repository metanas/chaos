use super::SignInState;
use state_machines::state_machine;

state_machine! {
    name: AccountSignIn,
    dynamic: true,
    initial: PickProvider,
    states: [
        superstate View {
            state PickProvider, state PickMode, state BrowserWaiting,
            state DeviceWaiting, state XaiWaiting, state SuccessMessage,
            state Succeeded, state KeyEditing, state KeyConfigured,
        }
    ],
    events {
        providers { transition: { from: View, to: PickProvider } }
        modes { transition: { from: View, to: PickMode } }
        browser { transition: { from: View, to: BrowserWaiting } }
        device { transition: { from: View, to: DeviceWaiting } }
        xai { transition: { from: View, to: XaiWaiting } }
        edit_key { transition: { from: View, to: KeyEditing } }
        configure_key { transition: { from: View, to: KeyConfigured } }
        already_connected { transition: { from: View, to: Succeeded } }
        succeed {
            transition: { from: BrowserWaiting, to: SuccessMessage }
            transition: { from: DeviceWaiting, to: SuccessMessage }
            transition: { from: XaiWaiting, to: SuccessMessage }
        }
        acknowledge { transition: { from: SuccessMessage, to: Succeeded } }
    }
}

impl SignInState {
    fn phase(&self) -> AccountSignInState {
        match self {
            Self::PickProvider => AccountSignInState::PickProvider,
            Self::PickMode => AccountSignInState::PickMode,
            Self::ChatGptContinueInBrowser(_) => AccountSignInState::BrowserWaiting,
            Self::ChatGptDeviceCode(_) => AccountSignInState::DeviceWaiting,
            Self::XaiDeviceCode(_) => AccountSignInState::XaiWaiting,
            Self::ChatGptSuccessMessage => AccountSignInState::SuccessMessage,
            Self::ChatGptSuccess => AccountSignInState::Succeeded,
            Self::ApiKeyEntry(_) => AccountSignInState::KeyEditing,
            Self::ApiKeyConfigured(_) => AccountSignInState::KeyConfigured,
        }
    }

    pub(super) fn transition(&mut self, next: Self) -> bool {
        let event = match &next {
            Self::PickProvider => AccountSignInEvent::Providers,
            Self::PickMode => AccountSignInEvent::Modes,
            Self::ChatGptContinueInBrowser(_) => AccountSignInEvent::Browser,
            Self::ChatGptDeviceCode(_) => AccountSignInEvent::Device,
            Self::XaiDeviceCode(_) => AccountSignInEvent::Xai,
            Self::ChatGptSuccessMessage => AccountSignInEvent::Succeed,
            Self::ChatGptSuccess if matches!(self, Self::ChatGptSuccessMessage) => {
                AccountSignInEvent::Acknowledge
            }
            Self::ChatGptSuccess => AccountSignInEvent::AlreadyConnected,
            Self::ApiKeyEntry(_) => AccountSignInEvent::EditKey,
            Self::ApiKeyConfigured(_) => AccountSignInEvent::ConfigureKey,
        };
        if DynamicAccountSignIn::new_init_state((), self.phase())
            .handle(event)
            .is_err()
        {
            return false;
        }
        *self = next;
        true
    }
}

#[cfg(test)]
mod tests;
