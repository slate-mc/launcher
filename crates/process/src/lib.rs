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
    child: Child,
    log_path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StartedProcess {
    pub session_id: SessionId,
    pub pid: u32,
    pub log_path: PathBuf,
}

impl ProcessSupervisor {
    pub fn start(
        &self,
        instance_id: InstanceId,
        session_id: SessionId,
        plan: &LaunchPlan,
        log_path: PathBuf,
    ) -> Result<StartedProcess, ProcessError> {
        let mut children = self.children.lock().map_err(|_| ProcessError::LockPoisoned)?;
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
        let child = tokio::process::Command::from(command).kill_on_drop(false).spawn()?;
        let pid = child.id().ok_or(ProcessError::PidUnavailable)?;
        children.insert(
            instance_id,
            SupervisedProcess {
                session_id,
                child,
                log_path: log_path.clone(),
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
        let mut children = self.children.lock().map_err(|_| ProcessError::LockPoisoned)?;
        if let Some(mut process) = children.remove(&instance_id) {
            process.child.start_kill()?;
        }
        Ok(())
    }

    pub fn refresh(&self) -> Result<Vec<ExitedProcess>, ProcessError> {
        let mut children = self.children.lock().map_err(|_| ProcessError::LockPoisoned)?;
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
                });
            }
        }
        for process in &exited {
            children.remove(&process.instance_id);
        }
        Ok(exited)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExitedProcess {
    pub instance_id: InstanceId,
    pub session_id: SessionId,
    pub exit_code: Option<i32>,
    pub log_path: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum ProcessError {
    #[error("that instance already has a running game process")]
    InstanceAlreadyRunning,
    #[error("process supervisor lock is unavailable")]
    LockPoisoned,
    #[error("session log path has no parent")]
    LogParentMissing,
    #[error("the operating system did not return a process id")]
    PidUnavailable,
    #[error("process I/O failed")]
    Io(#[from] std::io::Error),
}
