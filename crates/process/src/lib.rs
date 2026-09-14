//! Supervised game process ownership for slate.

use slate_domain::{InstanceId, SessionId};
use slate_minecraft::LaunchPlan;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use tokio::process::Child;

#[derive(Clone, Debug, Default)]
pub struct ProcessSupervisor {
    children: Arc<Mutex<BTreeMap<InstanceId, SupervisedProcess>>>,
}

#[derive(Debug)]
struct SupervisedProcess {
    session_id: SessionId,
    pid: u32,
    child: Child,
    log_path: PathBuf,
    state: ProcessState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StartedProcess {
    pub session_id: SessionId,
    pub pid: u32,
    pub log_path: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessState {
    Running,
    Stopping,
}

impl ProcessState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Stopping => "stopping",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveProcess {
    pub instance_id: InstanceId,
    pub session_id: SessionId,
    pub pid: u32,
    pub log_path: PathBuf,
    pub state: ProcessState,
}

impl ProcessSupervisor {
    pub fn start(
        &self,
        instance_id: InstanceId,
        session_id: SessionId,
        plan: &LaunchPlan,
        log_path: PathBuf,
    ) -> Result<StartedProcess, ProcessError> {
        let mut children = self
            .children
            .lock()
            .map_err(|_| ProcessError::LockPoisoned)?;
        if children.contains_key(&instance_id) {
            return Err(ProcessError::InstanceAlreadyRunning);
        }
        let parent = log_path.parent().ok_or(ProcessError::LogParentMissing)?;
        std::fs::create_dir_all(parent)?;
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;
        let stderr = log.try_clone()?;
        let mut command = plan.command();
        command.stdout(Stdio::from(log)).stderr(Stdio::from(stderr));
        let child = tokio::process::Command::from(command)
            .kill_on_drop(false)
            .spawn()?;
        let pid = child.id().ok_or(ProcessError::PidUnavailable)?;
        children.insert(
            instance_id,
            SupervisedProcess {
                session_id,
                pid,
                child,
                log_path: log_path.clone(),
                state: ProcessState::Running,
            },
        );
        Ok(StartedProcess {
            session_id,
            pid,
            log_path,
        })
    }

    pub fn terminate_after_tracking_failure(
        &self,
        instance_id: InstanceId,
    ) -> Result<(), ProcessError> {
        let mut children = self
            .children
            .lock()
            .map_err(|_| ProcessError::LockPoisoned)?;
        if let Some(mut process) = children.remove(&instance_id) {
            process.child.start_kill()?;
        }
        Ok(())
    }

    pub fn refresh(&self) -> Result<Vec<ExitedProcess>, ProcessError> {
        let mut children = self
            .children
            .lock()
            .map_err(|_| ProcessError::LockPoisoned)?;
        let ids = children.keys().copied().collect::<Vec<_>>();
        let mut exited = Vec::new();
        for instance_id in ids {
            let Some(process) = children.get_mut(&instance_id) else {
                continue;
            };
            if let Some(status) = process.child.try_wait()? {
                exited.push(ExitedProcess {
                    instance_id,
                    session_id: process.session_id,
                    exit_code: status.code(),
                    log_path: process.log_path.clone(),
                    force_stopped: process.state == ProcessState::Stopping,
                });
            }
        }
        for process in &exited {
            children.remove(&process.instance_id);
        }
        Ok(exited)
    }

    pub fn active(&self) -> Result<Vec<ActiveProcess>, ProcessError> {
        let children = self
            .children
            .lock()
            .map_err(|_| ProcessError::LockPoisoned)?;
        Ok(children
            .iter()
            .map(|(instance_id, process)| ActiveProcess {
                instance_id: *instance_id,
                session_id: process.session_id,
                pid: process.pid,
                log_path: process.log_path.clone(),
                state: process.state,
            })
            .collect())
    }

    pub fn active_for_instance(
        &self,
        instance_id: InstanceId,
    ) -> Result<Option<ActiveProcess>, ProcessError> {
        let children = self
            .children
            .lock()
            .map_err(|_| ProcessError::LockPoisoned)?;
        Ok(children.get(&instance_id).map(|process| ActiveProcess {
            instance_id,
            session_id: process.session_id,
            pid: process.pid,
            log_path: process.log_path.clone(),
            state: process.state,
        }))
    }

    pub fn force_stop(&self, session_id: SessionId) -> Result<ActiveProcess, ProcessError> {
        let mut children = self
            .children
            .lock()
            .map_err(|_| ProcessError::LockPoisoned)?;
        let (instance_id, process) = children
            .iter_mut()
            .find(|(_, process)| process.session_id == session_id)
            .ok_or(ProcessError::SessionNotFound)?;
        if process.state == ProcessState::Running {
            process.child.start_kill()?;
            process.state = ProcessState::Stopping;
        }
        Ok(ActiveProcess {
            instance_id: *instance_id,
            session_id: process.session_id,
            pid: process.pid,
            log_path: process.log_path.clone(),
            state: process.state,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExitedProcess {
    pub instance_id: InstanceId,
    pub session_id: SessionId,
    pub exit_code: Option<i32>,
    pub log_path: PathBuf,
    pub force_stopped: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ProcessError {
    #[error("that instance already has a running game process")]
    InstanceAlreadyRunning,
    #[error("that game session is no longer running")]
    SessionNotFound,
    #[error("process supervisor lock is unavailable")]
    LockPoisoned,
    #[error("session log path has no parent")]
    LogParentMissing,
    #[error("the operating system did not return a process id")]
    PidUnavailable,
    #[error("process I/O failed")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::{ProcessError, ProcessState, ProcessSupervisor};
    use slate_domain::{InstanceId, SessionId};
    use slate_minecraft::{LaunchArgument, LaunchPlan};
    use std::time::Duration;

    #[test]
    #[ignore]
    fn supervised_test_child_waits_for_termination() {
        std::thread::sleep(Duration::from_secs(30));
    }

    #[tokio::test]
    async fn tracks_rejects_duplicates_and_force_stops_a_process()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let mut plan = LaunchPlan::new(std::env::current_exe()?, directory.path())?;
        plan.push_argument(LaunchArgument::public("--exact"));
        plan.push_argument(LaunchArgument::public(
            "tests::supervised_test_child_waits_for_termination",
        ));
        plan.push_argument(LaunchArgument::public("--ignored"));
        let instance_id = InstanceId::new();
        let session_id = SessionId::new();
        let supervisor = ProcessSupervisor::default();
        let started = supervisor.start(
            instance_id,
            session_id,
            &plan,
            directory.path().join("session.log"),
        )?;

        assert_eq!(
            supervisor
                .active_for_instance(instance_id)?
                .map(|value| value.pid),
            Some(started.pid)
        );
        assert!(matches!(
            supervisor.start(
                instance_id,
                SessionId::new(),
                &plan,
                directory.path().join("duplicate.log")
            ),
            Err(ProcessError::InstanceAlreadyRunning)
        ));
        assert_eq!(
            supervisor.force_stop(session_id)?.state,
            ProcessState::Stopping
        );

        for _ in 0..50 {
            let exited = supervisor.refresh()?;
            if let Some(exited) = exited.into_iter().next() {
                assert!(exited.force_stopped);
                assert_eq!(exited.session_id, session_id);
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        Err("supervised child did not exit after force stop".into())
    }
}
