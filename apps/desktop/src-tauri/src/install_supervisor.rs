use slate_domain::JobId;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

#[derive(Clone, Debug, Default)]
pub(super) struct InstallSupervisor {
    cancellations: Arc<Mutex<BTreeMap<JobId, oneshot::Sender<()>>>>,
}

impl InstallSupervisor {
    pub(super) fn register(&self, job_id: JobId) -> oneshot::Receiver<()> {
        let (sender, receiver) = oneshot::channel();
        self.cancellations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(job_id, sender);
        receiver
    }

    pub(super) fn cancel(&self, job_id: JobId) -> bool {
        self.cancellations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&job_id)
            .is_some_and(|sender| sender.send(()).is_ok())
    }

    pub(super) fn finish(&self, job_id: JobId) {
        self.cancellations
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
        let receiver = supervisor.register(job_id);

        assert!(supervisor.cancel(job_id));
        assert!(!supervisor.cancel(job_id));
        assert!(receiver.await.is_ok());
    }
}
