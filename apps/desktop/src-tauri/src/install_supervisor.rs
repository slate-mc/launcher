use slate_domain::JobId;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use tokio::sync::{oneshot, watch};

#[derive(Debug)]
struct InstallControl {
    cancellation: oneshot::Sender<()>,
    pause: watch::Sender<bool>,
}

#[derive(Debug)]
pub(super) struct InstallRegistration {
    pub(super) cancellation: oneshot::Receiver<()>,
    pub(super) pause: watch::Receiver<bool>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct InstallSupervisor {
    controls: Arc<Mutex<BTreeMap<JobId, InstallControl>>>,
}

impl InstallSupervisor {
    pub(super) fn register(&self, job_id: JobId) -> InstallRegistration {
        let (cancellation, cancellation_receiver) = oneshot::channel();
        let (pause, pause_receiver) = watch::channel(false);
        self.controls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(
                job_id,
                InstallControl {
                    cancellation,
                    pause,
                },
            );
        InstallRegistration {
            cancellation: cancellation_receiver,
            pause: pause_receiver,
        }
    }

    pub(super) fn cancel(&self, job_id: JobId) -> bool {
        self.controls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&job_id)
            .is_some_and(|control| control.cancellation.send(()).is_ok())
    }

    pub(super) fn set_paused(&self, job_id: JobId, paused: bool) -> bool {
        self.controls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&job_id)
            .is_some_and(|control| control.pause.send(paused).is_ok())
    }

    pub(super) fn finish(&self, job_id: JobId) {
        self.controls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&job_id);
    }
}

#[cfg(test)]
mod tests {
    use super::InstallSupervisor;
    use slate_domain::JobId;

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
}
