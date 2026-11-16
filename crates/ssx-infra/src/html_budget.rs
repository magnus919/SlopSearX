//! A05 owned-allocation accounting. This is not an OS RSS limit.
use std::sync::Arc;
use tokio::{
    sync::{OwnedSemaphorePermit, Semaphore},
    time::Instant,
};

pub(crate) const MAX_SEARCH_BYTES: usize = 256 * 1024 * 1024;
pub(crate) const RESOURCE_ERROR: &str = "HTML resource budget exceeded";
// Intended aggregate: four searches, each at most256MiB. Complete allocation
// charging and supervisor-owned cancellation leases are still being integrated.
// Native allocator/transport/platform overhead still requires qualification.
static ADMISSION: std::sync::OnceLock<Arc<Semaphore>> = std::sync::OnceLock::new();

#[derive(Clone)]
pub(crate) struct Budget(pub(crate) ssx_owned_budget::Budget);
pub(crate) type Charge = ssx_owned_budget::Charge;
pub(crate) struct Operation {
    pub(crate) budget: Budget,
    pub(crate) deadline: Instant,
    _query: Charge,
    _admission: OwnedSemaphorePermit,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum OperationError {
    Resource,
    Deadline,
    Closed,
}
impl OperationError {
    pub(crate) fn message(&self) -> &'static str {
        match self {
            Self::Resource => RESOURCE_ERROR,
            Self::Deadline => "HTML parser deadline exceeded",
            Self::Closed => "HTML parser admission closed",
        }
    }
}
impl Operation {
    pub(crate) async fn start(
        limit: usize,
        query_bytes: usize,
        deadline: Instant,
    ) -> Result<Arc<Self>, OperationError> {
        if limit == 0 || limit > MAX_SEARCH_BYTES {
            return Err(OperationError::Resource);
        }
        // Reject impossible input before waiting or copying queue-owned data.
        if query_bytes > limit {
            return Err(OperationError::Resource);
        }
        if Instant::now() >= deadline {
            return Err(OperationError::Deadline);
        }
        let semaphore = ADMISSION
            .get_or_init(|| Arc::new(Semaphore::new(4)))
            .clone();
        let permit = tokio::time::timeout_at(deadline, semaphore.acquire_owned())
            .await
            .map_err(|_| OperationError::Deadline)?
            .map_err(|_| OperationError::Closed)?;
        let budget =
            Budget(ssx_owned_budget::Budget::new(limit).map_err(|_| OperationError::Resource)?);
        let query = budget
            .charge(query_bytes)
            .map_err(|_| OperationError::Resource)?;
        Ok(Arc::new(Self {
            budget,
            deadline,
            _query: query,
            _admission: permit,
        }))
    }
}
impl Budget {
    pub(crate) fn charge(&self, bytes: usize) -> Result<Charge, &'static str> {
        self.0.reserve(bytes).map_err(|_| RESOURCE_ERROR)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn overlap_is_charged_until_owner_drop_and_invalid_limits_fail_closed() {
        let operation = Operation::start(16, 4, Instant::now() + std::time::Duration::from_secs(1))
            .await
            .unwrap();
        let first = operation.budget.charge(8).unwrap();
        assert!(operation.budget.charge(5).is_err());
        assert!(operation.budget.charge(usize::MAX).is_err());
        drop(first);
        let second = operation.budget.charge(12).unwrap();
        assert!(operation.budget.charge(1).is_err());
        drop(second);
        assert!(Operation::start(0, 0, Instant::now()).await.is_err());
        assert!(
            Operation::start(MAX_SEARCH_BYTES + 1, 0, Instant::now())
                .await
                .is_err()
        );
        assert!(Operation::start(3, 4, Instant::now()).await.is_err());
    }
}
