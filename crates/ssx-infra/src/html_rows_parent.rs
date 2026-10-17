//! Parent rows phase. Its frame/input/child reservation share the search root;
//! the subsequent result projection and child tree heaps remain separate audits.
use super::{
    OperationFailure,
    row_wire::{Error, Layout},
    rows_borrowed::Rows,
    rows_worker::Request,
};
use crate::{
    adapters::RowLimit,
    html_budget::{Charge, Operation},
    process_pool::ProcessPool,
};
use ssx_owned_budget::OwnedVec;
use std::sync::{Arc, Mutex};
struct Lease {
    operation: Arc<Operation>,
    child: Mutex<Option<Charge>>,
    _reply: Charge,
    _header: Charge,
    _metadata: Charge,
}
impl Lease {
    fn release_child(&self) {
        drop(self.child.lock().unwrap_or_else(|e| e.into_inner()).take());
    }
}
pub(crate) struct OwnedRows {
    frame: Vec<u8>,
    lease: Arc<Lease>,
    layout: Layout,
}
fn check(operation: &Operation) -> Result<(), OperationFailure> {
    if tokio::time::Instant::now() >= operation.deadline {
        Err(OperationFailure::Deadline)
    } else {
        Ok(())
    }
}
fn resource(_: ssx_owned_budget::Error) -> OperationFailure {
    OperationFailure::Resource
}
fn wire(error: Error) -> OperationFailure {
    match error {
        Error::Deadline => OperationFailure::Deadline,
        Error::Overflow | Error::Size => OperationFailure::Resource,
        _ => OperationFailure::Invariant("HTML helper invalid rows response"),
    }
}
impl OwnedRows {
    pub(crate) async fn rows(&self) -> Result<Rows<'_>, OperationFailure> {
        Rows::validate(&self.frame, self.layout, &|| {
            check(&self.lease.operation).map_err(|_| Error::Deadline)
        })
        .await
        .map_err(wire)
    }
    pub(crate) async fn a10_rows(
        &self,
    ) -> Result<crate::adapters::vulnerability_wire::Rows<'_>, OperationFailure> {
        crate::adapters::vulnerability_wire::Rows::validate(
            &self.frame,
            self.layout,
            &self.lease.operation,
        )
        .await
    }
    pub(crate) fn operation(&self) -> &Arc<Operation> {
        &self.lease.operation
    }
}
pub(super) async fn parse(
    process: &ProcessPool,
    points: &[u32],
    layout: Layout,
    limit: RowLimit,
    operation: Arc<Operation>,
) -> Result<OwnedRows, OperationFailure> {
    check(&operation)?;
    if points.len() > super::MAX_POINTS {
        return Err(OperationFailure::Invariant(
            "HTML decoded input exceeds codec expansion bound",
        ));
    }
    let layout_meta = std::alloc::Layout::new::<[usize; 2]>()
        .extend(std::alloc::Layout::new::<Lease>())
        .map_err(|_| OperationFailure::Resource)?
        .0
        .pad_to_align();
    let metadata = operation
        .budget
        .0
        .reserve(layout_meta.size())
        .map_err(resource)?;
    let header_charge = operation
        .budget
        .0
        .reserve(super::rows_worker::HEADER)
        .map_err(resource)?;
    let size = points
        .len()
        .checked_mul(4)
        .ok_or(OperationFailure::Resource)?;
    let mut input = OwnedVec::with_capacity(&operation.budget.0, size).map_err(resource)?;
    for chunk in points.chunks(1024) {
        for point in chunk {
            check(&operation)?;
            input
                .try_extend_from_slice(&point.to_be_bytes())
                .map_err(resource)?;
        }
        tokio::task::yield_now().await;
        check(&operation)?;
    }
    let state = operation.budget.0.snapshot().map_err(resource)?;
    let allowance = state
        .limit
        .checked_sub(state.used)
        .ok_or(OperationFailure::Resource)?
        / 2;
    if allowance < size || allowance < 15 {
        return Err(OperationFailure::Resource);
    }
    let reply = operation.budget.0.reserve(allowance).map_err(resource)?;
    let child = operation.budget.0.reserve(allowance).map_err(resource)?;
    let request = Request {
        layout,
        limit,
        count: points.len(),
        allowance,
    };
    let fixed = request
        .encode()
        .map_err(|_| OperationFailure::Invariant("HTML rows request invariant"))?;
    let mut header = Vec::new();
    header
        .try_reserve_exact(fixed.len())
        .map_err(|_| OperationFailure::Resource)?;
    if header.capacity() > fixed.len() {
        operation.budget.0.fail_closed();
        return Err(OperationFailure::Resource);
    }
    header.extend(fixed);
    let lease = Arc::new(Lease {
        operation,
        child: Mutex::new(Some(child)),
        _reply: reply,
        _header: header_charge,
        _metadata: metadata,
    });
    let frame = process
        .exchange_with_lease(
            lease.operation.deadline,
            header,
            input,
            allowance,
            lease.clone(),
        )
        .await
        .map_err(OperationFailure::from)?;
    lease.release_child();
    if frame.capacity() > allowance {
        lease.operation.budget.0.fail_closed();
        return Err(OperationFailure::Resource);
    }
    check(&lease.operation)?;
    if frame.starts_with(b"A05E") {
        if frame.len() != 7 || frame[4] != 1 {
            return Err(OperationFailure::Invariant(
                "HTML helper invalid rows failure",
            ));
        }
        return Err(match (frame[5], frame[6]) {
            (1, 0) => OperationFailure::Resource,
            (2, code) => {
                OperationFailure::Parse(super::rows_worker::ERRORS.get(usize::from(code)).ok_or(
                    OperationFailure::Invariant("HTML helper invalid rows diagnostic"),
                )?)
            }
            (3, 0) => OperationFailure::Invariant("HTML helper rows invariant"),
            _ => OperationFailure::Invariant("HTML helper invalid rows failure"),
        });
    }
    let owned = OwnedRows {
        frame,
        lease,
        layout,
    };
    // Full validation before exposing even a row view. Reconstructing a later
    // borrowed view revalidates; no unchecked self-referential string exists.
    if layout.is_a10() {
        owned.a10_rows().await?;
    } else {
        owned.rows().await?;
    }
    Ok(owned)
}

#[cfg(test)]
pub(super) fn test_frame(
    frame: Vec<u8>,
    layout: Layout,
    operation: Arc<Operation>,
) -> Result<OwnedRows, OperationFailure> {
    let reply = operation
        .budget
        .0
        .reserve(frame.capacity())
        .map_err(resource)?;
    let layout_meta = std::alloc::Layout::new::<[usize; 2]>()
        .extend(std::alloc::Layout::new::<Lease>())
        .map_err(|_| OperationFailure::Resource)?
        .0
        .pad_to_align();
    let metadata = operation
        .budget
        .0
        .reserve(layout_meta.size())
        .map_err(resource)?;
    let header = operation.budget.0.reserve(0).map_err(resource)?;
    Ok(OwnedRows {
        frame,
        lease: Arc::new(Lease {
            operation,
            child: Mutex::new(None),
            _reply: reply,
            _header: header,
            _metadata: metadata,
        }),
        layout,
    })
}
