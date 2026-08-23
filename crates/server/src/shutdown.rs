//! The process-wide shutdown signal: one `watch` channel, cloned everywhere a
//! task needs to know the process is stopping.

use tokio::sync::watch;

#[derive(Clone, Debug)]
pub struct Shutdown {
    receiver: watch::Receiver<bool>,
}

#[derive(Debug)]
pub struct ShutdownTrigger {
    sender: watch::Sender<bool>,
}

#[must_use]
pub fn channel() -> (ShutdownTrigger, Shutdown) {
    let (sender, receiver) = watch::channel(false);
    (ShutdownTrigger { sender }, Shutdown { receiver })
}

impl ShutdownTrigger {
    /// Idempotent: firing twice cannot corrupt the drain.
    pub fn fire(&self) {
        let _ = self.sender.send(true);
    }
}

impl Shutdown {
    /// Resolves once shutdown has been signalled — or if the trigger was
    /// dropped without firing, which would otherwise strand waiters forever.
    pub async fn wait(&self) {
        let mut receiver = self.receiver.clone();
        if *receiver.borrow_and_update() {
            return;
        }
        let _ = receiver.changed().await;
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    #[tokio::test]
    async fn wait_resolves_after_fire() {
        let (trigger, shutdown) = super::channel();
        let waiter = tokio::spawn(async move { shutdown.wait().await });
        trigger.fire();
        tokio::time::timeout(Duration::from_secs(5), waiter)
            .await
            .expect("wait must resolve once fired")
            .expect("waiter must not panic");
    }

    #[tokio::test]
    async fn handle_cloned_after_fire_still_observes_it() {
        let (trigger, shutdown) = super::channel();
        trigger.fire();
        let late = shutdown.clone();
        tokio::time::timeout(Duration::from_secs(5), async move { late.wait().await })
            .await
            .expect("a handle cloned after the signal must not hang");
    }

    #[tokio::test]
    async fn dropping_the_trigger_releases_waiters() {
        let (trigger, shutdown) = super::channel();
        drop(trigger);
        tokio::time::timeout(Duration::from_secs(5), async move { shutdown.wait().await })
            .await
            .expect("a dropped trigger must not strand waiters");
    }

    #[tokio::test]
    async fn firing_twice_is_harmless() {
        let (trigger, shutdown) = super::channel();
        trigger.fire();
        trigger.fire();
        tokio::time::timeout(Duration::from_secs(5), async move { shutdown.wait().await })
            .await
            .expect("a repeated fire must still resolve waiters");
    }
}
