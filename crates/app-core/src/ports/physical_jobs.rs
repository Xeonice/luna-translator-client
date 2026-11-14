//! Neutral physical-work lifetime. Cancellation of an async caller does not
//! end the blocking closure, process wait or stdio job that owns its guard.
use crate::contracts::{AppError, AppResult};
use std::sync::{Arc, Mutex};
use tokio::sync::watch;

struct State {
    count: usize,
    open: bool,
    broken: bool,
}

pub struct PhysicalJobs {
    state: Mutex<State>,
    changes: watch::Sender<usize>,
}

fn failure(code: &str, message: &str) -> AppError {
    AppError::new(code, "physical_jobs", message)
}

impl Default for PhysicalJobs {
    fn default() -> Self {
        let (changes, _) = watch::channel(0);
        Self {
            state: Mutex::new(State {
                count: 0,
                open: true,
                broken: false,
            }),
            changes,
        }
    }
}

/// Non-Clone: the actual physical job owns one admission until it has exited.
pub struct PhysicalJob {
    owner: Arc<PhysicalJobs>,
}

impl std::fmt::Debug for PhysicalJob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PhysicalJob([owned])")
    }
}

impl Drop for PhysicalJob {
    fn drop(&mut self) {
        // A poisoned tracker must still release its physical count, but future
        // admission/drain checks remain failed closed through the poisoned lock.
        let mut state = self.owner.state.lock().unwrap_or_else(|p| p.into_inner());
        match state.count.checked_sub(1) {
            Some(count) => state.count = count,
            None => state.broken = true,
        }
        self.owner.changes.send_replace(state.count);
    }
}

impl PhysicalJobs {
    pub fn try_begin(self: &Arc<Self>) -> AppResult<PhysicalJob> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| failure("physical_jobs_state", "物理任务状态不可核验"))?;
        if state.broken {
            return Err(failure("physical_jobs_state", "物理任务状态不可核验"));
        }
        if !state.open {
            return Err(failure("physical_jobs_closed", "物理任务登记已关闭"));
        }
        state.count = state
            .count
            .checked_add(1)
            .ok_or_else(|| failure("physical_jobs_overflow", "物理任务数量无效"))?;
        self.changes.send_replace(state.count);
        Ok(PhysicalJob {
            owner: self.clone(),
        })
    }

    /// Call after every final I/O producer has stopped or registered its last
    /// job. Closing too early must not prevent required terminal SQL work.
    pub fn close_admission(&self) -> AppResult<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| failure("physical_jobs_state", "物理任务状态不可核验"))?;
        if state.broken {
            return Err(failure("physical_jobs_state", "物理任务状态不可核验"));
        }
        state.open = false;
        Ok(())
    }

    /// With open admission this preserves the original point-in-time drain
    /// behavior. Close admission first when the Owner needs a final drain.
    pub async fn wait_for_physical_drain(&self) -> AppResult<()> {
        let mut receiver = self.changes.subscribe();
        loop {
            {
                let state = self
                    .state
                    .lock()
                    .map_err(|_| failure("physical_jobs_state", "物理任务状态不可核验"))?;
                if state.broken {
                    return Err(failure("physical_jobs_state", "物理任务状态不可核验"));
                }
                if state.count == 0 {
                    return Ok(());
                }
            }
            receiver
                .changed()
                .await
                .map_err(|_| failure("physical_jobs_state", "物理任务无法收尾"))?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overflow_is_rejected_without_changing_the_existing_count() {
        let jobs = Arc::new(PhysicalJobs::default());
        jobs.state.lock().unwrap().count = usize::MAX;
        assert_eq!(jobs.try_begin().unwrap_err().code, "physical_jobs_overflow");
        assert_eq!(jobs.state.lock().unwrap().count, usize::MAX);
    }

    #[tokio::test]
    async fn poisoned_state_cannot_claim_admission_or_drain() {
        let jobs = Arc::new(PhysicalJobs::default());
        let other = jobs.clone();
        let _ = std::thread::spawn(move || {
            let _state = other.state.lock().unwrap();
            panic!("controlled physical tracker poison");
        })
        .join();
        assert_eq!(jobs.try_begin().unwrap_err().code, "physical_jobs_state");
        assert_eq!(
            jobs.close_admission().unwrap_err().code,
            "physical_jobs_state"
        );
        assert_eq!(
            jobs.wait_for_physical_drain().await.unwrap_err().code,
            "physical_jobs_state"
        );
    }
}
