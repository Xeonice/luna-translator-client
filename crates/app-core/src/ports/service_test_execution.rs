//! In-memory facts from the real saved-service transport; never IPC or authority.
use crate::{
    contracts::{AppError, AppResult},
    ports::google_authorization::GoogleExecutionLeaseRecord,
};
use std::sync::Mutex;
pub struct ServiceTestExecutionObservation {
    facts: Mutex<ServiceTestExecutionFacts>,
    physical: std::sync::Arc<super::physical_jobs::PhysicalJobs>,
}
#[derive(Debug, Clone, Default)]
pub struct ServiceTestExecutionFacts {
    /// Actual transport invocation, not proof of remote delivery or billing.
    pub request_started: bool,
    pub credential_used: bool,
    pub google_lease: Option<GoogleExecutionLeaseRecord>,
}
impl Default for ServiceTestExecutionObservation {
    fn default() -> Self {
        Self {
            facts: Mutex::new(Default::default()),
            physical: std::sync::Arc::new(Default::default()),
        }
    }
}
pub struct ServiceTestPhysicalJob {
    _physical: super::physical_jobs::PhysicalJob,
    // Keep the original observation lifetime while its wrapper guard exists.
    _observation: std::sync::Arc<ServiceTestExecutionObservation>,
}
impl ServiceTestExecutionObservation {
    /// The blocking closure owns this guard until physical I/O has returned,
    /// independently of whether its async JoinHandle was dropped.
    pub fn begin_physical_job(self: &std::sync::Arc<Self>) -> AppResult<ServiceTestPhysicalJob> {
        let physical = self.physical.try_begin().map_err(|error| {
            AppError::new(
                "test_observation_state",
                "service_test",
                if error.code == "physical_jobs_overflow" {
                    "测试物理任务数量无效"
                } else {
                    "测试物理任务状态不可核验"
                },
            )
        })?;
        Ok(ServiceTestPhysicalJob {
            _physical: physical,
            _observation: self.clone(),
        })
    }
    pub async fn wait_for_physical_drain(&self) -> AppResult<()> {
        self.physical.wait_for_physical_drain().await.map_err(|_| {
            AppError::new(
                "test_observation_state",
                "service_test",
                "测试物理任务无法收尾",
            )
        })
    }

    pub fn snapshot(&self) -> AppResult<ServiceTestExecutionFacts> {
        self.facts.lock().map(|v| v.clone()).map_err(|_| {
            AppError::new(
                "test_observation_state",
                "service_test",
                "测试执行事实无法核验",
            )
        })
    }
    /// Confirmed material access/lease use is separate from starting a translation
    /// transport. False means no such observation, not proof no OS read occurred.
    pub fn mark_credential_used(&self) -> AppResult<()> {
        self.facts
            .lock()
            .map_err(|_| {
                AppError::new(
                    "test_observation_state",
                    "service_test",
                    "测试执行事实无法核验",
                )
            })?
            .credential_used = true;
        Ok(())
    }
    pub fn mark_transport_started(&self, credential_used: bool) -> AppResult<()> {
        let mut f = self.facts.lock().map_err(|_| {
            AppError::new(
                "test_observation_state",
                "service_test",
                "测试执行事实无法核验",
            )
        })?;
        f.request_started = true;
        f.credential_used |= credential_used;
        Ok(())
    }
    pub fn record_google_lease(&self, lease: GoogleExecutionLeaseRecord) -> AppResult<()> {
        let mut f = self.facts.lock().map_err(|_| {
            AppError::new(
                "test_observation_state",
                "service_test",
                "测试执行事实无法核验",
            )
        })?;
        if f.google_lease
            .as_ref()
            .is_some_and(|old| old.lease_id != lease.lease_id)
        {
            return Err(AppError::new(
                "test_lease_conflict",
                "service_test",
                "同次测试不能替换已取得的授权",
            ));
        }
        f.google_lease = Some(lease);
        Ok(())
    }
}
