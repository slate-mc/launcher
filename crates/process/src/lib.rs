//! Supervised game process ownership for slate.

use slate_client_protocol::{ClientHandshake, ExpectedClient};
use slate_domain::{InstanceId, SessionId};
use slate_minecraft::LaunchPlan;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::process::Child;

const LOG_SNAPSHOT_BYTES: u64 = 256 * 1024;
const LOG_CHUNK_BYTES: u64 = 32 * 1024;

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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ChildProcessPriority {
    Low,
    BelowNormal,
    #[default]
    Normal,
    AboveNormal,
    High,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogChunkKind {
    Snapshot,
    Append,
    Reset,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogChunk {
    pub kind: LogChunkKind,
    pub offset: u64,
    pub truncated: bool,
    pub text: String,
}

#[derive(Clone, Debug)]
pub struct SessionLogTail {
    path: PathBuf,
    offset: u64,
}

impl SessionLogTail {
    #[must_use]
    pub const fn new(path: PathBuf) -> Self {
        Self { path, offset: 0 }
    }

    #[must_use]
    pub const fn offset(&self) -> u64 {
        self.offset
    }

    pub async fn snapshot(&mut self) -> Result<LogChunk, ProcessError> {
        let length = tokio::fs::metadata(&self.path).await?.len();
        let start = length.saturating_sub(LOG_SNAPSHOT_BYTES);
        let (text, bytes_read) = read_log_range(&self.path, start, LOG_SNAPSHOT_BYTES).await?;
        self.offset = start.saturating_add(bytes_read);
        Ok(LogChunk {
            kind: LogChunkKind::Snapshot,
            offset: self.offset,
            truncated: start > 0,
            text,
        })
    }

    pub async fn next_chunk(&mut self) -> Result<Option<LogChunk>, ProcessError> {
        let length = tokio::fs::metadata(&self.path).await?.len();
        let (kind, start) = if length < self.offset {
            (LogChunkKind::Reset, 0)
        } else if length == self.offset {
            return Ok(None);
        } else {
            (LogChunkKind::Append, self.offset)
        };
        let (text, bytes_read) = read_log_range(&self.path, start, LOG_CHUNK_BYTES).await?;
        self.offset = start.saturating_add(bytes_read);
        Ok(Some(LogChunk {
            kind,
            offset: self.offset,
            truncated: false,
            text,
        }))
    }
}

async fn read_log_range(
    path: &std::path::Path,
    start: u64,
    limit: u64,
) -> Result<(String, u64), ProcessError> {
    let mut file = tokio::fs::File::open(path).await?;
    file.seek(std::io::SeekFrom::Start(start)).await?;
    let mut bytes = Vec::new();
    file.take(limit).read_to_end(&mut bytes).await?;
    let bytes_read = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    Ok((String::from_utf8_lossy(&bytes).into_owned(), bytes_read))
}

impl ProcessSupervisor {
    pub fn start(
        &self,
        instance_id: InstanceId,
        session_id: SessionId,
        plan: &LaunchPlan,
        log_path: PathBuf,
        priority: ChildProcessPriority,
        cpu_affinity: &[u16],
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
        apply_process_priority(&mut command, priority);
        command.stdout(Stdio::from(log)).stderr(Stdio::from(stderr));
        let mut child = tokio::process::Command::from(command)
            .kill_on_drop(false)
            .spawn()?;
        let pid = child.id().ok_or(ProcessError::PidUnavailable)?;
        if let Err(error) = apply_process_affinity(pid, cpu_affinity) {
            let _ = child.start_kill();
            return Err(error);
        }
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

    pub fn active_for_session(
        &self,
        session_id: SessionId,
    ) -> Result<Option<ActiveProcess>, ProcessError> {
        let children = self
            .children
            .lock()
            .map_err(|_| ProcessError::LockPoisoned)?;
        Ok(children.iter().find_map(|(instance_id, process)| {
            (process.session_id == session_id).then(|| ActiveProcess {
                instance_id: *instance_id,
                session_id: process.session_id,
                pid: process.pid,
                log_path: process.log_path.clone(),
                state: process.state,
            })
        }))
    }

    pub fn validate_client_handshake(
        &self,
        instance_id: InstanceId,
        handshake_path: &std::path::Path,
        mut expected: ExpectedClient,
    ) -> Result<ClientHandshake, ProcessError> {
        let children = self
            .children
            .lock()
            .map_err(|_| ProcessError::LockPoisoned)?;
        let process = children
            .get(&instance_id)
            .ok_or(ProcessError::SessionNotFound)?;
        expected.process_id = u64::from(process.pid);
        let handshake = ClientHandshake::read(handshake_path)?;
        handshake.validate(&expected)?;
        Ok(handshake)
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

#[cfg(windows)]
fn apply_process_priority(command: &mut std::process::Command, priority: ChildProcessPriority) {
    use std::os::windows::process::CommandExt;

    const IDLE_PRIORITY_CLASS: u32 = 0x0000_0040;
    const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
    const NORMAL_PRIORITY_CLASS: u32 = 0x0000_0020;
    const ABOVE_NORMAL_PRIORITY_CLASS: u32 = 0x0000_8000;
    const HIGH_PRIORITY_CLASS: u32 = 0x0000_0080;
    let flags = match priority {
        ChildProcessPriority::Low => IDLE_PRIORITY_CLASS,
        ChildProcessPriority::BelowNormal => BELOW_NORMAL_PRIORITY_CLASS,
        ChildProcessPriority::Normal => NORMAL_PRIORITY_CLASS,
        ChildProcessPriority::AboveNormal => ABOVE_NORMAL_PRIORITY_CLASS,
        ChildProcessPriority::High => HIGH_PRIORITY_CLASS,
    };
    command.creation_flags(flags);
}

#[cfg(not(windows))]
fn apply_process_priority(_: &mut std::process::Command, _: ChildProcessPriority) {}

#[cfg(windows)]
fn apply_process_affinity(pid: u32, cpu_affinity: &[u16]) -> Result<(), ProcessError> {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    if cpu_affinity.is_empty() {
        return Ok(());
    }
    let mut mask = 0_u64;
    for cpu in cpu_affinity {
        let bit = u32::from(*cpu);
        if bit >= 64 {
            return Err(ProcessError::AffinityUnavailable);
        }
        mask |= 1_u64 << bit;
    }
    let signed_mask = mask as i64;
    let script = format!(
        "$process = Get-Process -Id {pid} -ErrorAction Stop; $process.ProcessorAffinity = [IntPtr]({signed_mask})"
    );
    let mut command = std::process::Command::new("powershell.exe");
    command
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &script,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW);
    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(ProcessError::AffinityUnavailable)
    }
}

#[cfg(not(windows))]
fn apply_process_affinity(_: u32, cpu_affinity: &[u16]) -> Result<(), ProcessError> {
    if cpu_affinity.is_empty() {
        Ok(())
    } else {
        Err(ProcessError::AffinityUnavailable)
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
    #[error("the operating system could not apply the requested CPU affinity")]
    AffinityUnavailable,
    #[error("process I/O failed")]
    Io(#[from] std::io::Error),
    #[error("Slate Client did not complete its verified handshake")]
    ClientHandshake(#[from] slate_client_protocol::ClientHandshakeError),
}

#[cfg(test)]
mod tests {
    use super::{
        ChildProcessPriority, LOG_SNAPSHOT_BYTES, LogChunkKind, ProcessError, ProcessState,
        ProcessSupervisor, SessionLogTail,
    };
    use slate_client_protocol::{ClientLoader, ExpectedClient};
    use slate_domain::{InstanceId, SessionId};
    use slate_minecraft::{LaunchArgument, LaunchPlan};
    use std::time::Duration;

    #[tokio::test]
    async fn log_tail_snapshots_appends_and_recovers_from_truncation()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("session.log");
        tokio::fs::write(&path, "booting\n").await?;
        let mut tail = SessionLogTail::new(path.clone());

        let snapshot = tail.snapshot().await?;
        assert_eq!(snapshot.kind, LogChunkKind::Snapshot);
        assert!(!snapshot.truncated);
        assert_eq!(snapshot.text, "booting\n");
        assert!(tail.next_chunk().await?.is_none());

        let mut file = tokio::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .await?;
        tokio::io::AsyncWriteExt::write_all(&mut file, b"ready\n").await?;
        drop(file);
        let appended = tail
            .next_chunk()
            .await?
            .ok_or("expected appended log data")?;
        assert_eq!(appended.kind, LogChunkKind::Append);
        assert_eq!(appended.text, "ready\n");

        tokio::fs::write(&path, "restarted\n").await?;
        let reset = tail.next_chunk().await?.ok_or("expected reset log data")?;
        assert_eq!(reset.kind, LogChunkKind::Reset);
        assert_eq!(reset.text, "restarted\n");
        Ok(())
    }

    #[tokio::test]
    async fn log_snapshot_is_bounded_and_retains_the_file_offset()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("large-session.log");
        let extra = 4096_u64;
        let file_length = LOG_SNAPSHOT_BYTES + extra;
        let contents = vec![b'x'; usize::try_from(file_length)?];
        tokio::fs::write(&path, contents).await?;
        let mut tail = SessionLogTail::new(path);

        let snapshot = tail.snapshot().await?;

        assert_eq!(snapshot.offset, file_length);
        assert!(snapshot.truncated);
        assert_eq!(u64::try_from(snapshot.text.len())?, LOG_SNAPSHOT_BYTES);
        Ok(())
    }

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
            ChildProcessPriority::Normal,
            &[],
        )?;

        assert_eq!(
            supervisor
                .active_for_instance(instance_id)?
                .map(|value| value.pid),
            Some(started.pid)
        );
        assert_eq!(
            supervisor
                .active_for_session(session_id)?
                .map(|value| value.instance_id),
            Some(instance_id)
        );
        assert!(matches!(
            supervisor.start(
                instance_id,
                SessionId::new(),
                &plan,
                directory.path().join("duplicate.log"),
                ChildProcessPriority::Normal,
                &[],
            ),
            Err(ProcessError::InstanceAlreadyRunning)
        ));
        let handshake_path = directory.path().join("client-handshake.json");
        std::fs::write(
            &handshake_path,
            format!(
                r#"{{
                    "schema": 1,
                    "protocolVersion": 1,
                    "processId": {},
                    "startedAtEpochMillis": 1000,
                    "adapterStatus": "contract_only",
                    "target": {{
                        "minecraftVersion": "1.21.1",
                        "loader": "fabric",
                        "loaderVersion": "0.19.5",
                        "javaMajor": 21
                    }},
                    "modules": []
                }}"#,
                started.pid
            ),
        )?;
        let handshake = supervisor.validate_client_handshake(
            instance_id,
            &handshake_path,
            ExpectedClient {
                process_id: 0,
                started_at_epoch_millis_minimum: 999,
                minecraft_version: "1.21.1".to_owned(),
                loader: ClientLoader::Fabric,
                loader_version: "0.19.5".to_owned(),
                java_major: 21,
                require_active_adapter: false,
            },
        )?;
        assert_eq!(handshake.process_id, u64::from(started.pid));
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
