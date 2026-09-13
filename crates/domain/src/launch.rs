use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LaunchState {
    Queued,
    Preparing,
    Downloading,
    Verifying,
    Ready,
    Starting,
    Running,
    Exited,
    Failed,
    Crashed,
    Cancelled,
}

impl LaunchState {
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Queued, Self::Preparing | Self::Cancelled)
                | (
                    Self::Preparing,
                    Self::Downloading | Self::Verifying | Self::Failed | Self::Cancelled
                )
                | (
                    Self::Downloading,
                    Self::Verifying | Self::Failed | Self::Cancelled
                )
                | (
                    Self::Verifying,
                    Self::Ready | Self::Failed | Self::Cancelled
                )
                | (Self::Ready, Self::Starting)
                | (Self::Starting, Self::Running | Self::Failed)
                | (Self::Running, Self::Exited | Self::Crashed)
        )
    }

    pub fn transition_to(self, next: Self) -> Result<Self, TransitionError> {
        if self.can_transition_to(next) {
            Ok(next)
        } else {
            Err(TransitionError {
                from: self,
                to: next,
            })
        }
    }

    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Exited | Self::Failed | Self::Crashed | Self::Cancelled
        )
    }

    #[must_use]
    pub const fn as_storage_value(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Preparing => "preparing",
            Self::Downloading => "downloading",
            Self::Verifying => "verifying",
            Self::Ready => "ready",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Exited => "exited",
            Self::Failed => "failed",
            Self::Crashed => "crashed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("invalid launch transition from {from:?} to {to:?}")]
pub struct TransitionError {
    pub from: LaunchState,
    pub to: LaunchState,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LaunchReadiness {
    #[default]
    Unknown,
    ProcessStarted,
    ClientMenuObserved,
    Connecting,
    JoinedWorld,
    JoinedServer,
}

#[cfg(test)]
mod tests {
    use super::LaunchState;
    use proptest::prelude::*;

    #[test]
    fn happy_path_reaches_exit() -> Result<(), Box<dyn std::error::Error>> {
        let state = LaunchState::Queued
            .transition_to(LaunchState::Preparing)?
            .transition_to(LaunchState::Downloading)?
            .transition_to(LaunchState::Verifying)?
            .transition_to(LaunchState::Ready)?
            .transition_to(LaunchState::Starting)?
            .transition_to(LaunchState::Running)?
            .transition_to(LaunchState::Exited)?;

        assert!(state.is_terminal());
        Ok(())
    }

    proptest! {
        #[test]
        fn terminal_states_never_transition(next in any::<u8>()) {
            let all = [
                LaunchState::Queued,
                LaunchState::Preparing,
                LaunchState::Downloading,
                LaunchState::Verifying,
                LaunchState::Ready,
                LaunchState::Starting,
                LaunchState::Running,
                LaunchState::Exited,
                LaunchState::Failed,
                LaunchState::Crashed,
                LaunchState::Cancelled,
            ];
            let target = all[usize::from(next) % all.len()];

            for terminal in [
                LaunchState::Exited,
                LaunchState::Failed,
                LaunchState::Crashed,
                LaunchState::Cancelled,
            ] {
                prop_assert!(!terminal.can_transition_to(target));
            }
        }
    }
}
