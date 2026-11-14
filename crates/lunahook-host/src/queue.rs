//! Nonblocking ingress with explicit, sticky integrity-loss accounting.

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
    mpsc::{self, Receiver, SyncSender, TrySendError},
};

pub struct EventSender<T> {
    sender: SyncSender<T>,
    dropped: Arc<AtomicU64>,
}

impl<T> Clone for EventSender<T> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            dropped: Arc::clone(&self.dropped),
        }
    }
}

pub fn bounded<T>(capacity: usize) -> (EventSender<T>, Receiver<T>, Arc<AtomicU64>) {
    let (sender, receiver) = mpsc::sync_channel(capacity);
    let dropped = Arc::new(AtomicU64::new(0));
    (
        EventSender {
            sender,
            dropped: Arc::clone(&dropped),
        },
        receiver,
        dropped,
    )
}

impl<T> EventSender<T> {
    /// Never waits for a consumer. A failed send permanently marks data loss.
    pub fn try_send(&self, item: T) -> bool {
        match self.sender.try_send(item) {
            Ok(()) => true,
            Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => {
                self.dropped.fetch_add(1, Ordering::Relaxed);
                false
            }
        }
    }

    pub fn dropped_total(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// Record a payload that could not be copied, even if its diagnostic fits.
    pub fn record_loss(&self) {
        self.dropped.fetch_add(1, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slow_consumers_never_block_and_loss_stays_visible() {
        let (sender, receiver, dropped) = bounded(1);
        assert!(sender.try_send(1));
        assert!(!sender.try_send(2));
        assert_eq!(dropped.load(Ordering::Relaxed), 1);
        assert_eq!(receiver.recv().expect("queued item"), 1);
        assert!(sender.try_send(3));
        assert_eq!(sender.dropped_total(), 1);
        drop(receiver);
        assert!(!sender.try_send(4));
        assert_eq!(sender.dropped_total(), 2);
    }
}
