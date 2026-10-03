use std::time::Duration;
use std::time::Instant;

use crate::app_event::AppEvent;

use super::ChatWidget;

const DEFAULT_TERMINAL_TITLE: &str = "new session";
const WORKING_ICONS: &[&str] = &["◰", "◱", "◲", "◳"];
const ATTENTION_ICONS: &[&str] = &["◻", "❏"];
const FRAME_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TerminalTitleState {
    Idle,
    Working,
    Attention,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct TerminalTitleAnimation {
    state: TerminalTitleState,
    frame: usize,
    next_frame_at: Instant,
}

impl ChatWidget {
    fn requires_user_attention(&self) -> bool {
        self.bottom_pane.requires_user_attention() || self.interrupts.requires_user_attention()
    }

    /// Synchronize request transitions without rewriting the title on every keypress.
    pub(super) fn refresh_terminal_title_if_attention_changed(&self) {
        if self.terminal_title_attention.get() != self.requires_user_attention() {
            self.refresh_terminal_title();
        }
    }

    pub(super) fn refresh_terminal_title(&self) {
        self.refresh_terminal_title_at(Instant::now());
    }

    fn terminal_title_state(&self) -> TerminalTitleState {
        let needs_attention = self.requires_user_attention();
        if needs_attention {
            TerminalTitleState::Attention
        } else if self.agent_turn_running || self.mcp_startup_status.is_some() {
            TerminalTitleState::Working
        } else {
            TerminalTitleState::Idle
        }
    }

    fn refresh_terminal_title_at(&self, now: Instant) {
        let state = self.terminal_title_state();
        self.terminal_title_attention
            .set(state == TerminalTitleState::Attention);
        if self.config.terminal_title == chaos_kern::config::TerminalTitleMode::Off {
            self.terminal_title_animation.set(None);
            return;
        }
        let frame = if state != TerminalTitleState::Idle {
            let mut animation = self
                .terminal_title_animation
                .get()
                .filter(|animation| animation.state == state)
                .unwrap_or(TerminalTitleAnimation {
                    state,
                    frame: 0,
                    next_frame_at: now + FRAME_INTERVAL,
                });
            if now >= animation.next_frame_at {
                let frame_count = match state {
                    TerminalTitleState::Working => WORKING_ICONS.len(),
                    TerminalTitleState::Attention => ATTENTION_ICONS.len(),
                    TerminalTitleState::Idle => 1,
                };
                animation.frame = (animation.frame + 1) % frame_count;
                animation.next_frame_at = now + FRAME_INTERVAL;
            }
            self.terminal_title_animation.set(Some(animation));
            self.frame_requester
                .schedule_frame_in(animation.next_frame_at.saturating_duration_since(now));
            animation.frame
        } else {
            self.terminal_title_animation.set(None);
            0
        };
        let title = terminal_title_text(
            self.process_name.as_deref(),
            state,
            self.config.tui_terminal_title_icon.as_deref(),
            frame,
        );
        self.app_event_tx
            .send(AppEvent::SetTerminalTitle(Some(title)));
    }

    pub(super) fn refresh_terminal_title_animation(&self) {
        self.refresh_terminal_title_animation_at(Instant::now());
    }

    fn refresh_terminal_title_animation_at(&self, now: Instant) {
        let animation = self.terminal_title_animation.get();
        let state = self.terminal_title_state();
        if self.config.terminal_title == chaos_kern::config::TerminalTitleMode::Off
            || state == TerminalTitleState::Idle
        {
            if animation.is_some() {
                self.refresh_terminal_title_at(now);
            }
            return;
        }
        match animation {
            Some(animation) if animation.state == state && now < animation.next_frame_at => {
                // The shared scheduler coalesces deadlines, so an earlier redraw
                // must re-arm this animation's deadline.
                self.frame_requester
                    .schedule_frame_in(animation.next_frame_at.saturating_duration_since(now));
            }
            _ => self.refresh_terminal_title_at(now),
        }
    }
}

fn terminal_title_text(
    process_name: Option<&str>,
    state: TerminalTitleState,
    idle_icon: Option<&str>,
    frame: usize,
) -> String {
    let process_name = process_name.unwrap_or(DEFAULT_TERMINAL_TITLE);
    let icon = match state {
        TerminalTitleState::Idle => idle_icon,
        TerminalTitleState::Working => Some(WORKING_ICONS[frame % WORKING_ICONS.len()]),
        TerminalTitleState::Attention => Some(ATTENTION_ICONS[frame % ATTENTION_ICONS.len()]),
    };
    match icon {
        Some(icon) => format!("{icon} {process_name}"),
        None => process_name.to_string(),
    }
}

#[cfg(test)]
pub(super) mod tests;
