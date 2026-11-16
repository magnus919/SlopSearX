//! Explicit component-only manifest ownership. No handler/output/quota profile
//! selects this path until separate composition and admission review.
pub mod replay;
use super::{
    Cancellation, Failure,
    encoding::{Error, Prefix},
    service::{NON_VERIFICATION_NOTE, ResolvedManifestSource, error},
};
use crate::lineage::{
    self, Resolver,
    capture::shared::manifest::{Finished, ManifestCapture},
};
use sha2::{Digest, Sha256};
use ssx_domain::{ArtifactKind, ArtifactRef, TenantId, WallClockTimestamp};
use ssx_json::{Value, json};
use std::collections::BTreeSet;

/// Semantic error envelopes remain ordinary values. A successful captured
/// manifest has exactly one immutable owner and cannot be cloned into Values.
pub enum Outcome {
    Value(Value),
    Captured(CapturedManifest),
}
impl Outcome {
    pub(super) fn value(self) -> Result<Value, Failure> {
        match self {
            Self::Value(value) => Ok(value),
            Self::Captured(_) => Err(Failure::Malformed),
        }
    }
}
pub(super) enum Walks {
    Values {
        ids: Vec<String>,
        seen: BTreeSet<String>,
        cuts: Vec<Value>,
    },
    Captured(ManifestCapture),
}
impl Walks {
    pub(super) fn new(
        captured: bool,
        roots: usize,
        ids: Vec<String>,
        seen: BTreeSet<String>,
        cancel: &Cancellation,
    ) -> Result<Self, Failure> {
        if captured {
            Ok(Self::Captured(
                ManifestCapture::new(roots, ids, cancel).map_err(map_lineage)?,
            ))
        } else {
            Ok(Self::Values {
                seen,
                ids,
                cuts: Vec::new(),
            })
        }
    }
    pub(super) async fn append(
        &mut self,
        resolver: &dyn Resolver,
        tenant: &TenantId,
        request: lineage::Request,
        cancel: &Cancellation,
    ) -> Result<(), Failure> {
        match self {
            Self::Values { ids, seen, cuts } => {
                let graph = lineage::traverse(resolver, tenant, request, cancel)
                    .await
                    .map_err(map_lineage)?;
                for node in &graph.nodes {
                    if node.status == lineage::Status::Live
                        && node.artifact.kind() == ArtifactKind::Result
                        && seen.insert(node.artifact.id().into())
                    {
                        ids.push(node.artifact.id().into());
                    }
                }
                cuts.push(graph.to_value());
            }
            Self::Captured(capture) => capture
                .append(resolver, tenant, request, cancel)
                .await
                .map_err(map_lineage)?,
        }
        Ok(())
    }
    pub(super) fn finish(self, cancel: &Cancellation) -> Result<(Vec<String>, Assembly), Failure> {
        match self {
            Self::Values { ids, cuts, .. } => {
                let items = Vec::with_capacity(ids.len());
                Ok((ids, Assembly::Values { cuts, items }))
            }
            Self::Captured(capture) => {
                let mut cuts = capture.finish_after_walks();
                let ids = cuts.take_result_ids();
                // Keep the26th sentinel so the original selection error occurs
                // before any item reads; all requested roots already completed.
                let mut items = Prefix::new();
                items.raw(b"[", cancel);
                Ok((
                    ids,
                    Assembly::Captured {
                        cuts,
                        items,
                        raw_items: Vec::new(),
                        count: 0,
                        raw_fault: None,
                    },
                ))
            }
        }
    }
}
fn map_lineage(error: lineage::Failure) -> Failure {
    match error {
        lineage::Failure::Cancelled => Failure::Cancelled,
        lineage::Failure::Closed => Failure::Closed,
        lineage::Failure::Unavailable => Failure::Unavailable,
        lineage::Failure::Malformed => Failure::Malformed,
    }
}
pub(super) enum Assembly {
    Values {
        cuts: Vec<Value>,
        items: Vec<Value>,
    },
    Captured {
        cuts: Finished,
        items: Prefix,
        raw_items: Vec<Box<str>>,
        count: usize,
        raw_fault: Option<Failure>,
    },
}
impl Assembly {
    pub(super) fn item(&mut self, item: Value, cancel: &Cancellation) -> Result<(), Failure> {
        match self {
            Self::Values { items, .. } => items.push(item),
            Self::Captured {
                cuts,
                items,
                raw_items,
                count,
                raw_fault,
            } => {
                if *count != 0 {
                    items.raw(b",", cancel);
                }
                items.value(&item, 2, cancel);
                *count += 1;
                if items.fault() == Some(Error::Cancelled) {
                    return Err(Failure::Cancelled);
                }
                if items.fault().is_some() || cuts.canonical_cuts().is_err() {
                    cuts.discard_raw();
                    raw_items.clear();
                } else if raw_fault.is_none() {
                    // Exact-sized lossless fragment, no input-parser reentry or
                    // new semantic byte cap. The global charged admission seam
                    // remains a separate unactivated prerequisite.
                    match lineage::capture::fragment(&item, cancel, usize::MAX) {
                        Ok(raw) => raw_items.push(raw),
                        Err(lineage::Failure::Cancelled) => return Err(Failure::Cancelled),
                        Err(error) => {
                            *raw_fault = Some(map_lineage(error));
                            cuts.discard_raw();
                            raw_items.clear();
                        }
                    }
                }
            }
        }
        Ok(())
    }
    pub(super) fn finish(
        self,
        ids: Vec<String>,
        source: Option<ResolvedManifestSource>,
        now: WallClockTimestamp,
        cancel: &Cancellation,
    ) -> Result<Outcome, Failure> {
        match self {
            Self::Values { cuts, items } => {
                let mut manifest = json!({"contract":"slopsearx.research_manifest","version":1,"generated_at":now.seconds(),"items":null,"lineage_cuts":null,"observations_verified":false,"verification_note":NON_VERIFICATION_NOTE});
                manifest["items"] = Value::Array(items);
                manifest["lineage_cuts"] = Value::Array(cuts);
                if let Some(source) = &source {
                    manifest["source"] = source.stored.clone();
                }
                let preimage = match super::encoding::encode(&manifest, cancel) {
                    Ok(bytes) => bytes,
                    Err(error) => return canonical_failure(error),
                };
                let reference = reference(&preimage)?;
                drop(preimage);
                manifest["artifact"] = reference.to_value();
                manifest["lineage"] = edges(&reference, &ids, source.as_ref())?;
                if let Err(error) = super::encoding::encode(&manifest, cancel) {
                    return canonical_failure(error);
                }
                cancel.check()?;
                Ok(Outcome::Value(manifest))
            }
            Self::Captured {
                mut cuts,
                mut items,
                raw_items,
                count,
                raw_fault,
            } => {
                items.raw(b"]", cancel);
                let preimage = canonical(&items, &cuts, source.as_ref(), now, None, cancel);
                let bytes = match preimage.complete() {
                    Ok(bytes) => bytes,
                    Err(error) => return canonical_failure(error),
                };
                let reference = reference(bytes)?;
                drop(preimage);
                // Keep late Result ArtifactRef validation after preimage success.
                let lineage = edges(&reference, &ids, source.as_ref())?;
                let final_encoding = canonical(
                    &items,
                    &cuts,
                    source.as_ref(),
                    now,
                    Some((&reference, &lineage)),
                    cancel,
                );
                if let Err(error) = final_encoding.complete() {
                    return canonical_failure(error);
                }
                drop(final_encoding);
                cancel.check()?;
                if let Some(error) = raw_fault {
                    return Err(error);
                }
                if raw_items.len() != count {
                    return Err(Failure::Malformed);
                }
                let graphs = cuts.take_raw().ok_or(Failure::Malformed)?;
                // Source is encoded only after ALL reads and semantic/final-ID
                // checks, retaining lexical JSON numbers and field insertion order.
                let raw_source = source
                    .as_ref()
                    .map(|source| lineage::capture::fragment(&source.stored, cancel, usize::MAX))
                    .transpose()
                    .map_err(map_lineage)?;
                let header = json!({"contract":"slopsearx.research_manifest","version":1,"generated_at":now.seconds()});
                let header = ssx_json::to_string(&header)
                    .map_err(|_| Failure::Malformed)?
                    .into_boxed_str();
                let artifact = ssx_json::to_string(&reference.to_value())
                    .map_err(|_| Failure::Malformed)?
                    .into_boxed_str();
                let lineage = lineage::capture::fragment(&lineage, cancel, usize::MAX)
                    .map_err(map_lineage)?;
                let note = ssx_json::to_string(NON_VERIFICATION_NOTE)
                    .map_err(|_| Failure::Malformed)?
                    .into_boxed_str();
                cancel.check()?;
                Ok(Outcome::Captured(CapturedManifest {
                    header,
                    note,
                    items: raw_items,
                    graphs,
                    source: raw_source,
                    artifact,
                    lineage,
                }))
            }
        }
    }
}
fn reference(preimage: &[u8]) -> Result<ArtifactRef, Failure> {
    let digest = Sha256::digest(preimage)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    ArtifactRef::new(
        ArtifactKind::ResearchManifest,
        format!("manifest-v1-{digest}"),
    )
    .map_err(|_| Failure::Malformed)
}
fn edges(
    reference: &ArtifactRef,
    ids: &[String],
    source: Option<&ResolvedManifestSource>,
) -> Result<Value, Failure> {
    let mut edges = ids.iter().map(|id|ArtifactRef::new(ArtifactKind::Result,id).map(|target|json!({"from":reference.to_value(),"relation":"contains","to":target.to_value()}))).collect::<Result<Vec<_>,_>>().map_err(|_|Failure::Malformed)?;
    if let Some(source) = source {
        edges.push(json!({"from":reference.to_value(),"relation":"derived_from","to":source.artifact.to_value()}));
    }
    Ok(Value::Array(edges))
}
fn canonical_failure(error: Error) -> Result<Outcome, Failure> {
    match error {
        Error::TooLarge => Ok(Outcome::Value(error_value())),
        Error::Malformed => Err(Failure::Malformed),
        Error::Cancelled => Err(Failure::Cancelled),
    }
}
fn error_value() -> Value {
    error("resource_limit", "encoded manifest exceeds 1048576 bytes")
}
fn canonical(
    items: &Prefix,
    cuts: &Finished,
    source: Option<&ResolvedManifestSource>,
    now: WallClockTimestamp,
    final_fields: Option<(&ArtifactRef, &Value)>,
    cancel: &Cancellation,
) -> Prefix {
    let mut output = Prefix::new();
    output.raw(b"{", cancel);
    if let Some((reference, _)) = final_fields {
        output.raw(b"\"artifact\":", cancel);
        output.value(&reference.to_value(), 1, cancel);
        output.raw(b",", cancel);
    }
    output.raw(
        b"\"contract\":\"slopsearx.research_manifest\",\"generated_at\":",
        cancel,
    );
    output.value(&json!(now.seconds()), 1, cancel);
    output.raw(b",\"items\":", cancel);
    output.absorb(items, cancel);
    if let Some((_, lineage)) = final_fields {
        output.raw(b",\"lineage\":", cancel);
        output.value(lineage, 1, cancel);
    }
    output.raw(b",\"lineage_cuts\":", cancel);
    cuts.append_canonical(&mut output, cancel);
    output.raw(b",\"observations_verified\":false", cancel);
    if let Some(source) = source {
        output.raw(b",\"source\":", cancel);
        output.value(&source.stored, 1, cancel);
    }
    output.raw(b",\"verification_note\":", cancel);
    output.text(NON_VERIFICATION_NOTE, cancel);
    output.raw(b",\"version\":1}", cancel);
    output
}
/// No Clone or public fragment mutation: all output is replayed from this owner.
/// Requested capacities are distinct from decoder/sort/boxing/allocator/RSS and
/// are NOT an admission waiver. No default output profile uses this component.
pub struct CapturedManifest {
    header: Box<str>,
    note: Box<str>,
    items: Vec<Box<str>>,
    graphs: lineage::capture::shared::CapturedSet,
    source: Option<Box<str>>,
    artifact: Box<str>,
    lineage: Box<str>,
}
impl CapturedManifest {
    pub fn owned_bytes(&self) -> Option<usize> {
        let mut total = std::mem::size_of::<Self>()
            .checked_add(
                self.items
                    .capacity()
                    .checked_mul(std::mem::size_of::<Box<str>>())?,
            )?
            .checked_add(
                self.graphs
                    .owned_bytes()?
                    .checked_sub(std::mem::size_of::<lineage::capture::shared::CapturedSet>())?,
            )?;
        for bytes in self.items.iter().chain(self.source.iter()).chain([
            &self.header,
            &self.note,
            &self.artifact,
            &self.lineage,
        ]) {
            total = total.checked_add(bytes.len())?;
        }
        Some(total)
    }
    /// Synchronous component equivalence visitation, not a transport cursor or
    /// cooperative deadline guarantee. Replay never rereads retained stores.
    pub fn visit_json(
        &self,
        output: &mut dyn FnMut(&[u8]) -> Result<(), Failure>,
    ) -> Result<(), Failure> {
        let mut position = replay::Position::new();
        while position.visit_next(self, output)? {}
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PortFuture;
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    struct Live;
    impl Resolver for Live {
        fn resolve<'a>(
            &'a self,
            _: &'a TenantId,
            reference: &'a ArtifactRef,
            _: &'a Cancellation,
        ) -> PortFuture<'a, Result<lineage::Resolved, lineage::Failure>> {
            Box::pin(async move {
                Ok(lineage::Resolved {
                    node: lineage::Node {
                        artifact: reference.clone(),
                        status: lineage::Status::Live,
                        created_at: None,
                        expires_at: None,
                        details: None,
                    },
                    edges: vec![],
                })
            })
        }
    }
    fn ready<F: Future>(future: F) -> F::Output {
        let mut future = std::pin::pin!(future);
        match future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("pure resolver must be ready"),
        }
    }
    #[test]
    fn cancellation_before_last_walk_still_stops_both_variants() {
        for captured in [false, true] {
            let cancel = Cancellation::default();
            let mut walks = Walks::new(captured, 1, vec![], BTreeSet::new(), &cancel).unwrap();
            cancel.cancel();
            let result = ready(walks.append(
                &Live,
                &TenantId::new("tenant").unwrap(),
                lineage::Request {
                    root: ArtifactRef::new(ArtifactKind::Result, "snapshot:0").unwrap(),
                    direction: lineage::Direction::Both,
                    max_depth: 0,
                    max_nodes: 100,
                },
                &cancel,
            ));
            assert!(matches!(result, Err(Failure::Cancelled)));
            println!("W07_MANIFEST_BEFORE_WALK_CANCEL captured={captured} stopped=true");
        }
    }
    #[test]
    fn completed_empty_selection_seals_before_the_original_clock_then_cancel_check() {
        for captured in [false, true] {
            let cancel = Cancellation::default();
            struct Missing;
            impl Resolver for Missing {
                fn resolve<'a>(
                    &'a self,
                    _: &'a TenantId,
                    root: &'a ArtifactRef,
                    _: &'a Cancellation,
                ) -> PortFuture<'a, Result<lineage::Resolved, lineage::Failure>> {
                    Box::pin(async move {
                        Ok(lineage::Resolved {
                            node: lineage::Node::missing(root.clone(), lineage::Status::Missing),
                            edges: vec![],
                        })
                    })
                }
            }
            let mut walks = Walks::new(captured, 1, vec![], BTreeSet::new(), &cancel).unwrap();
            ready(walks.append(
                &Missing,
                &TenantId::new("tenant").unwrap(),
                lineage::Request {
                    root: ArtifactRef::new(ArtifactKind::Snapshot, "snapshot").unwrap(),
                    direction: lineage::Direction::Both,
                    max_depth: 0,
                    max_nodes: 100,
                },
                &cancel,
            ))
            .unwrap();
            cancel.cancel();
            let (ids, assembly) = walks.finish(&cancel).unwrap_or_else(|error| {
                panic!("new sealing check hid original zero-item clock phase: {error:?}")
            });
            assert!(ids.is_empty());
            // Same service expression: clock.now is evaluated before finish.
            let mut clock_calls = 0;
            let now = {
                clock_calls += 1;
                WallClockTimestamp::new(1000.).unwrap()
            };
            assert!(matches!(
                assembly.finish(ids, None, now, &cancel),
                Err(Failure::Cancelled)
            ));
            assert_eq!(clock_calls, 1);
            println!("W07_MANIFEST_EMPTY_CANCEL captured={captured} clock_once_before_cancel=true");
        }
    }
    #[test]
    fn completed_walk_selection_error_precedes_new_finish_cancellation() {
        for captured in [false, true] {
            let cancel = Cancellation::default();
            let initial = (0..25).map(|n| format!("snapshot:{n}")).collect::<Vec<_>>();
            let seen = initial.iter().cloned().collect();
            let mut walks = Walks::new(captured, 1, initial, seen, &cancel).unwrap();
            ready(walks.append(
                &Live,
                &TenantId::new("tenant").unwrap(),
                lineage::Request {
                    root: ArtifactRef::new(ArtifactKind::Result, "snapshot:25").unwrap(),
                    direction: lineage::Direction::Both,
                    max_depth: 0,
                    max_nodes: 100,
                },
                &cancel,
            ))
            .unwrap();
            // Operation completed; cancellation becomes visible at precisely
            // the old no-await selection/assembly phase boundary.
            cancel.cancel();
            let result = walks.finish(&cancel);
            println!(
                "W07_MANIFEST_SELECTION_CANCEL captured={captured} completed_last_walk=true finish_cancelled={} selection_len={:?}",
                matches!(&result, Err(Failure::Cancelled)),
                result.as_ref().ok().map(|(ids, _)| ids.len())
            );
            let (ids, _) = result.unwrap_or_else(|error| {
                panic!("completed selection error was replaced by {error:?}")
            });
            assert_eq!(
                ids.len(),
                26,
                "existing service next returns resource_limit before any item read/clock"
            );
        }
    }
}
