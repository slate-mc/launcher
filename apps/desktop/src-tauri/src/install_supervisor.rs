use slate_domain::JobId;
use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};
use tokio::sync::{oneshot, watch};

#[derive(Debug)]
struct InstallControl {
    cancellation: Option<oneshot::Sender<()>>,
    pause: watch::Sender<bool>,
    start: Option<oneshot::Sender<()>>,
}

#[derive(Debug, Default)]
struct InstallScheduler {
    active: Option<JobId>,
    queue: VecDeque<JobId>,
    controls: BTreeMap<JobId, InstallControl>,
}

#[derive(Debug)]
pub(super) struct InstallRegistration {
    pub(super) cancellation: oneshot::Receiver<()>,
    pub(super) pause: watch::Receiver<bool>,
    pub(super) start: oneshot::Receiver<()>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct InstallSupervisor {
    scheduler: Arc<Mutex<InstallScheduler>>,
}

impl InstallSupervisor {
    pub(super) fn register(&self, job_id: JobId) -> InstallRegistration {
        let (cancellation, cancellation_receiver) = oneshot::channel();
        let (pause, pause_receiver) = watch::channel(false);
        let (start, start_receiver) = oneshot::channel();
        let mut scheduler = self
            .scheduler
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        scheduler.controls.insert(
            job_id,
            InstallControl {
                cancellation: Some(cancellation),
                pause,
                start: Some(start),
            },
        );
        if scheduler.active.is_none() {
            scheduler.active = Some(job_id);
            start_job(&mut scheduler, job_id);
        } else {
            scheduler.queue.push_back(job_id);
        }
        InstallRegistration {
            cancellation: cancellation_receiver,
            pause: pause_receiver,
            start: start_receiver,
        }
    }

    pub(super) fn cancel(&self, job_id: JobId) -> bool {
        self.scheduler
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .controls
            .get_mut(&job_id)
            .and_then(|control| control.cancellation.take())
            .is_some_and(|cancellation| cancellation.send(()).is_ok())
    }

    pub(super) fn set_paused(&self, job_id: JobId, paused: bool) -> bool {
        self.scheduler
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .controls
            .get(&job_id)
            .is_some_and(|control| control.pause.send(paused).is_ok())
    }

    pub(super) fn queue_position(&self, job_id: JobId) -> Option<u32> {
        self.scheduler
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .queue
            .iter()
            .position(|candidate| *candidate == job_id)
            .and_then(|position| u32::try_from(position + 1).ok())
    }

    pub(super) fn move_queued(&self, job_id: JobId, direction: QueueDirection) -> bool {
        let mut scheduler = self
            .scheduler
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(position) = scheduler
            .queue
            .iter()
            .position(|candidate| *candidate == job_id)
        else {
            return false;
        };
        let target = match direction {
            QueueDirection::Up if position > 0 => position - 1,
            QueueDirection::Down if position + 1 < scheduler.queue.len() => position + 1,
            QueueDirection::Up | QueueDirection::Down => return false,
        };
        scheduler.queue.swap(position, target);
        true
    }

    pub(super) fn finish(&self, job_id: JobId) {
        let mut scheduler = self
            .scheduler
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        scheduler.controls.remove(&job_id);
        scheduler.queue.retain(|candidate| *candidate != job_id);
        if scheduler.active == Some(job_id) {
            scheduler.active = None;
            while let Some(next) = scheduler.queue.pop_front() {
                if scheduler.controls.contains_key(&next) {
                    scheduler.active = Some(next);
                    start_job(&mut scheduler, next);
                    break;
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum QueueDirection {
    Up,
    Down,
}

fn start_job(scheduler: &mut InstallScheduler, job_id: JobId) {
    if let Some(start) = scheduler
        .controls
        .get_mut(&job_id)
        .and_then(|control| control.start.take())
    {
        let _ = start.send(());
    }
}

#[cfg(test)]
mod tests {
    use super::{InstallSupervisor, QueueDirection};
    use slate_domain::JobId;
    use std::time::Duration;

    #[tokio::test]
    async fn registered_installs_can_be_cancelled_once() {
        let supervisor = InstallSupervisor::default();
        let job_id = JobId::new();
        let registration = supervisor.register(job_id);

        assert!(supervisor.cancel(job_id));
        assert!(!supervisor.cancel(job_id));
        assert!(registration.cancellation.await.is_ok());
    }

    #[tokio::test]
    async fn registered_installs_can_be_paused_and_resumed() {
        let supervisor = InstallSupervisor::default();
        let job_id = JobId::new();
        let mut registration = supervisor.register(job_id);

        assert!(supervisor.set_paused(job_id, true));
        assert!(registration.pause.changed().await.is_ok());
        assert!(*registration.pause.borrow());
        assert!(supervisor.set_paused(job_id, false));
        assert!(registration.pause.changed().await.is_ok());
        assert!(!*registration.pause.borrow());
    }

    #[tokio::test]
    async fn queued_installs_can_be_reordered_before_they_start() {
        let supervisor = InstallSupervisor::default();
        let first_id = JobId::new();
        let second_id = JobId::new();
        let third_id = JobId::new();
        let first = supervisor.register(first_id);
        let mut second = supervisor.register(second_id);
        let third = supervisor.register(third_id);

        assert!(first.start.await.is_ok());
        assert_eq!(supervisor.queue_position(second_id), Some(1));
        assert_eq!(supervisor.queue_position(third_id), Some(2));
        assert!(supervisor.move_queued(third_id, QueueDirection::Up));
        assert_eq!(supervisor.queue_position(third_id), Some(1));
        assert_eq!(supervisor.queue_position(second_id), Some(2));
        assert!(!supervisor.move_queued(third_id, QueueDirection::Up));
        assert!(
            tokio::time::timeout(Duration::from_millis(10), &mut second.start)
                .await
                .is_err()
        );

        supervisor.finish(first_id);
        assert!(third.start.await.is_ok());
        supervisor.finish(third_id);
        assert!(second.start.await.is_ok());
    }
}
