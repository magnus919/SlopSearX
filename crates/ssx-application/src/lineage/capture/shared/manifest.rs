//! Manifest-only canonical observer; no service/output/quota activation.
use super::{Captured, CapturedSet, SharedBuild};
use crate::{
    lineage::{
        self, Failure, Node, Request, Resolver, Status,
        capture::{Build, IndexedEdge, relation_name},
    },
    receipts::{
        Cancellation,
        encoding::{Error, Prefix},
    },
};
use ssx_domain::{ArtifactKind, ArtifactRef, TenantId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanonicalFault {
    TooLarge,
    Malformed,
    Cancelled,
    SelectionOverflow,
}
impl From<Error> for CanonicalFault {
    fn from(value: Error) -> Self {
        match value {
            Error::TooLarge => Self::TooLarge,
            Error::Malformed => Self::Malformed,
            Error::Cancelled => Self::Cancelled,
        }
    }
}
/// A whole manifest's cuts share the same sealed raw pool. Canonical bytes are
/// bounded independently and never stand in for lossless raw output ownership.
pub struct ManifestCapture {
    raw: CapturedSet,
    raw_available: bool,
    cuts: Prefix,
    ids: Vec<Box<str>>,
    selection_overflow: bool,
    completed: usize,
}
impl ManifestCapture {
    /// Called only after the original explicit/source selection validation.
    pub fn new(
        max_roots: usize,
        initial_ids: Vec<String>,
        cancel: &Cancellation,
    ) -> Result<Self, Failure> {
        if initial_ids.len() > 25 {
            return Err(Failure::Malformed);
        }
        let mut cuts = Prefix::new();
        cuts.raw(b"[", cancel);
        Ok(Self {
            raw: CapturedSet::new(max_roots)?,
            raw_available: true,
            cuts,
            ids: initial_ids
                .into_iter()
                .map(String::into_boxed_str)
                .collect(),
            selection_overflow: false,
            completed: 0,
        })
    }
    pub async fn append(
        &mut self,
        resolver: &dyn Resolver,
        tenant: &TenantId,
        request: Request,
        cancel: &Cancellation,
    ) -> Result<(), Failure> {
        if self.raw.attempts >= self.raw.max_roots {
            return Err(Failure::Malformed);
        }
        self.raw.attempts += 1;
        let outcome = lineage::walk(resolver, tenant, request, cancel, |request| ObservedBuild {
            raw: SharedBuild::new(request, &mut self.raw.pool),
            prior: &mut self.raw.graphs,
            available: &mut self.raw_available,
            ids: &mut self.ids,
            overflow: &mut self.selection_overflow,
            nodes: Prefix::new(),
            node_count: 0,
            references: Vec::with_capacity(request.max_nodes),
            edges: Vec::new(),
        })
        .await?;
        if cancel.is_cancelled() {
            return Err(Failure::Cancelled);
        }
        self.completed += 1;
        if !self.selection_overflow && self.cuts.fault().is_none() {
            if self.completed > 1 {
                self.cuts.raw(b",", cancel);
            }
            append_graph(&mut self.cuts, &outcome, cancel);
        }
        if self.cuts.fault() == Some(Error::Cancelled) {
            return Err(Failure::Cancelled);
        }
        if self.cuts.fault().is_some() || self.selection_overflow {
            self.raw_available = false;
            self.raw.graphs.clear();
            self.raw.pool.0.clear();
        }
        if self.raw_available {
            let mut raw = outcome.raw;
            // Selected IDs are kept once for the whole manifest, not per root.
            raw.selected_results.clear();
            self.raw.graphs.push(raw);
        }
        Ok(())
    }
    /// Consume once; this cannot append a second closing bracket or render a
    /// partial graph as a successful canonical manifest cut.
    pub fn finish(mut self, cancel: &Cancellation) -> Result<Finished, Failure> {
        self.cuts.raw(b"]", cancel);
        if self.cuts.fault() == Some(Error::Cancelled) {
            return Err(Failure::Cancelled);
        }
        Ok(self.into_finished())
    }
    /// Service-only phase boundary: walks completed already; the original
    /// selection error/zero-item clock decision occurs before another cancel
    /// check. Existing public finish(cancel) retains its original contract.
    pub(crate) fn finish_after_walks(mut self) -> Finished {
        self.cuts.close_completed_array();
        self.into_finished()
    }
    fn into_finished(self) -> Finished {
        Finished {
            raw: self.raw_available.then_some(self.raw),
            cuts: self.cuts,
            ids: self.ids,
            selection_overflow: self.selection_overflow,
        }
    }
}
pub struct Finished {
    raw: Option<CapturedSet>,
    cuts: Prefix,
    ids: Vec<Box<str>>,
    selection_overflow: bool,
}
impl Finished {
    pub(crate) fn take_result_ids(&mut self) -> Vec<String> {
        std::mem::take(&mut self.ids)
            .into_iter()
            .map(String::from)
            .collect()
    }
    pub(crate) fn discard_raw(&mut self) {
        self.raw = None;
    }
    pub(crate) fn take_raw(&mut self) -> Option<CapturedSet> {
        self.raw.take()
    }
    pub(crate) fn append_canonical(&self, target: &mut Prefix, cancel: &Cancellation) {
        target.absorb(&self.cuts, cancel);
    }

    pub fn canonical_cuts(&self) -> Result<&[u8], CanonicalFault> {
        if self.selection_overflow {
            return Err(CanonicalFault::SelectionOverflow);
        }
        self.cuts.complete().map_err(Into::into)
    }
    pub fn selection_overflow(&self) -> bool {
        self.selection_overflow
    }
    pub fn result_ids(&self) -> impl Iterator<Item = &str> {
        self.ids.iter().map(AsRef::as_ref)
    }
    pub fn raw(&self) -> Option<&CapturedSet> {
        self.raw.as_ref()
    }
    pub fn canonical_capacity(&self) -> usize {
        self.cuts.requested_bytes()
    }
}

struct Observed {
    raw: Captured,
    nodes: Prefix,
    node_count: usize,
    references: Vec<Box<[u8]>>,
    edges: Vec<IndexedEdge>,
}
struct ObservedBuild<'a> {
    raw: SharedBuild<'a>,
    prior: &'a mut Vec<Captured>,
    available: &'a mut bool,
    ids: &'a mut Vec<Box<str>>,
    overflow: &'a mut bool,
    nodes: Prefix,
    node_count: usize,
    references: Vec<Box<[u8]>>,
    edges: Vec<IndexedEdge>,
}
impl ObservedBuild<'_> {
    fn discard(&mut self) {
        *self.available = false;
        self.prior.clear();
        self.raw.pool.0.clear();
        self.raw.graph.nodes.clear();
        self.raw.graph.references.clear();
        self.raw.graph.edges.clear();
        self.raw.graph.selected_results.clear();
    }
}
impl Build for ObservedBuild<'_> {
    type Output = Observed;
    fn admit(&mut self, reference: &ArtifactRef, index: u8) -> Result<(), Failure> {
        if usize::from(index) != self.references.len() {
            return Err(Failure::Malformed);
        }
        let bytes =
            ssx_json::canonical_json(&reference.to_value(), ssx_json::JsonProfile::Utf8Compact)
                .map_err(|_| Failure::Malformed)?;
        if bytes.len() > super::super::REFERENCE_BYTES {
            return Err(Failure::Malformed);
        }
        self.references.push(bytes.into_boxed_slice());
        if *self.available {
            self.raw.admit(reference, index)?;
        }
        Ok(())
    }
    fn node(&mut self, node: Node, cancel: &Cancellation) -> Result<(), Failure> {
        if node.artifact.kind() == ArtifactKind::Result
            && node.status == Status::Live
            && !*self.overflow
            && !self.ids.iter().any(|id| id.as_ref() == node.artifact.id())
        {
            self.ids.push(node.artifact.id().into());
            if self.ids.len() > 25 {
                *self.overflow = true;
                self.discard();
            }
        }
        if !*self.overflow {
            if self.node_count > 0 {
                self.nodes.raw(b",", cancel);
            }
            self.nodes.node(&node, cancel);
            if self.nodes.fault() == Some(Error::Cancelled) {
                return Err(Failure::Cancelled);
            }
            if self.nodes.fault().is_some() {
                self.discard();
            }
        }
        self.node_count += 1;
        if *self.available {
            self.raw.node(node, cancel)?;
        }
        Ok(())
    }
    fn edge(&mut self, edge: crate::lineage::Edge, index: IndexedEdge) {
        self.edges.push(index);
        if *self.available {
            self.raw.edge(edge, index);
        }
    }
    fn finish(self, truncated: bool) -> Observed {
        Observed {
            raw: self.raw.finish(truncated),
            nodes: self.nodes,
            node_count: self.node_count,
            references: self.references,
            edges: self.edges,
        }
    }
}
fn append_graph(prefix: &mut Prefix, graph: &Observed, cancel: &Cancellation) {
    prefix.raw(
        b"{\"contract\":\"slopsearx.artifact_lineage\",\"counts\":{\"edges\":",
        cancel,
    );
    prefix.raw(graph.edges.len().to_string().as_bytes(), cancel);
    prefix.raw(b",\"nodes\":", cancel);
    prefix.raw(graph.node_count.to_string().as_bytes(), cancel);
    prefix.raw(b"},\"direction\":", cancel);
    prefix.text(graph.raw.direction.as_str(), cancel);
    prefix.raw(b",\"edges\":[", cancel);
    for (index, edge) in graph.edges.iter().enumerate() {
        if index > 0 {
            prefix.raw(b",", cancel);
        }
        prefix.raw(b"{\"from\":", cancel);
        prefix.raw(&graph.references[usize::from(edge.from)], cancel);
        prefix.raw(b",\"relation\":", cancel);
        prefix.text(
            relation_name(edge.relation).expect("same closed edge enum"),
            cancel,
        );
        prefix.raw(b",\"to\":", cancel);
        prefix.raw(&graph.references[usize::from(edge.to)], cancel);
        prefix.raw(b"}", cancel);
    }
    prefix.raw(b"],\"max_depth\":", cancel);
    prefix.raw(graph.raw.max_depth.to_string().as_bytes(), cancel);
    prefix.raw(b",\"max_nodes\":", cancel);
    prefix.raw(graph.raw.max_nodes.to_string().as_bytes(), cancel);
    prefix.raw(b",\"nodes\":[", cancel);
    prefix.absorb(&graph.nodes, cancel);
    prefix.raw(b"],\"root\":", cancel);
    prefix.raw(&graph.references[0], cancel);
    prefix.raw(b",\"truncated\":", cancel);
    prefix.raw(
        if graph.raw.truncated {
            b"true"
        } else {
            b"false"
        },
        cancel,
    );
    prefix.raw(b",\"version\":1}", cancel);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        PortFuture,
        lineage::{Direction, Edge, Resolved},
    };
    use ssx_domain::LineageRelation;
    use ssx_json::{Value, json};
    use std::{
        future::Future,
        sync::Mutex,
        task::{Context, Poll, Waker},
    };
    fn ready<F: Future>(future: F) -> F::Output {
        let mut future = std::pin::pin!(future);
        match future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("pure test resolver must complete immediately"),
        }
    }
    fn reference(n: usize) -> ArtifactRef {
        ArtifactRef::new(ArtifactKind::Result, format!("snapshot:{n}")).unwrap()
    }
    fn request(root: usize, nodes: usize) -> Request {
        Request {
            root: reference(root),
            direction: Direction::Both,
            max_depth: 5,
            max_nodes: nodes,
        }
    }
    struct Model {
        details: Vec<Value>,
        count: usize,
        fail: Option<usize>,
        calls: Mutex<Vec<usize>>,
    }
    impl Resolver for Model {
        fn resolve<'a>(
            &'a self,
            _: &'a TenantId,
            reference: &'a ArtifactRef,
            _: &'a Cancellation,
        ) -> PortFuture<'a, Result<Resolved, Failure>> {
            Box::pin(async move {
                let index = reference
                    .id()
                    .split(':')
                    .next_back()
                    .unwrap()
                    .parse::<usize>()
                    .unwrap();
                self.calls.lock().unwrap().push(index);
                if self.fail == Some(index) {
                    return Err(Failure::Unavailable);
                }
                Ok(Resolved {
                    node: Node {
                        artifact: reference.clone(),
                        status: Status::Live,
                        created_at: None,
                        expires_at: None,
                        details: Some(
                            self.details[index % self.details.len()]
                                .as_object()
                                .unwrap()
                                .clone(),
                        ),
                    },
                    edges: (index + 1..self.count)
                        .map(|next| Edge {
                            from: reference.clone(),
                            relation: LineageRelation::Contains,
                            to: super::tests::reference(next),
                        })
                        .collect(),
                })
            })
        }
    }
    fn model(details: Vec<Value>, count: usize, fail: Option<usize>) -> Model {
        Model {
            details,
            count,
            fail,
            calls: Mutex::default(),
        }
    }
    #[test]
    fn combined_canonical_cuts_match_original_graphs_and_keep_raw_spelling() {
        let details=ssx_json::from_str(r#"{"z":null,"a":[18446744073709551617,1.000e0,-0.0,"雪😀\n"],"$serde_json::private::Number":"literal"}"#).unwrap();
        let resolver = model(vec![details], 4, None);
        let tenant = TenantId::new("tenant").unwrap();
        let cancel = Cancellation::default();
        let mut capture = ManifestCapture::new(2, vec!["explicit:0".into()], &cancel).unwrap();
        let mut expected = vec![];
        let mut raw = vec![];
        for root in [0, 2] {
            resolver.calls.lock().unwrap().clear();
            let graph = ready(lineage::traverse(
                &resolver,
                &tenant,
                request(root, 100),
                &cancel,
            ))
            .unwrap()
            .to_value();
            let expected_reads = resolver.calls.lock().unwrap().clone();
            resolver.calls.lock().unwrap().clear();
            raw.push(ssx_json::to_string(&graph).unwrap());
            expected.push(graph);
            ready(capture.append(&resolver, &tenant, request(root, 100), &cancel)).unwrap();
            assert_eq!(*resolver.calls.lock().unwrap(), expected_reads);
        }
        let finished = capture.finish(&cancel).unwrap();
        assert_eq!(
            finished.canonical_cuts().unwrap(),
            ssx_json::canonical_json(&Value::Array(expected), ssx_json::JsonProfile::Utf8Compact)
                .unwrap()
        );
        assert_eq!(
            finished.result_ids().collect::<Vec<_>>(),
            vec![
                "explicit:0",
                "snapshot:0",
                "snapshot:1",
                "snapshot:2",
                "snapshot:3"
            ]
        );
        let raw_owner = finished.raw().unwrap();
        assert_eq!(raw_owner.unique_details(), 1);
        for (index, expected) in raw.iter().enumerate() {
            let mut bytes = vec![];
            raw_owner
                .visit_graph(index, &mut |part| {
                    bytes.extend_from_slice(part);
                    Ok(())
                })
                .unwrap();
            assert_eq!(bytes, expected.as_bytes());
        }
        assert_eq!(
            finished.canonical_capacity(),
            crate::receipts::MAX_MANIFEST_BYTES
        );
    }
    #[test]
    fn canonical_overflow_drops_raw_but_later_walk_failure_still_wins() {
        let resolver = model(
            vec![
                json!({"small":true}),
                json!({"body":"x".repeat(crate::receipts::MAX_MANIFEST_BYTES)}),
            ],
            4,
            Some(3),
        );
        let tenant = TenantId::new("tenant").unwrap();
        let cancel = Cancellation::default();
        let mut capture = ManifestCapture::new(1, vec![], &cancel).unwrap();
        assert_eq!(
            ready(capture.append(&resolver, &tenant, request(0, 100), &cancel)),
            Err(Failure::Unavailable)
        );
        assert_eq!(*resolver.calls.lock().unwrap(), vec![0, 1, 2, 3]);
        assert!(!capture.raw_available);
        assert!(capture.raw.pool.0.is_empty());
        assert!(capture.raw.graphs.is_empty());
        assert!(capture.raw.pool.0.capacity() > 0);
        let retained_pool_slots =
            capture.raw.pool.0.capacity() * std::mem::size_of::<super::super::Detail>();
        assert_eq!(
            capture.raw.pool.dynamic_bytes().unwrap(),
            retained_pool_slots
        );
        println!(
            "W07_CANONICAL_CLEAR_WITNESS pool_slots_capacity={} pool_retained_requested_bytes={} unique_detail_boxes={}",
            capture.raw.pool.0.capacity(),
            retained_pool_slots,
            capture.raw.pool.0.len()
        );

        let resolver = model(
            vec![json!({"body":"x".repeat(crate::receipts::MAX_MANIFEST_BYTES)})],
            4,
            None,
        );
        let mut capture = ManifestCapture::new(1, vec![], &cancel).unwrap();
        ready(capture.append(&resolver, &tenant, request(0, 100), &cancel)).unwrap();
        assert_eq!(*resolver.calls.lock().unwrap(), vec![0, 1, 2, 3]);
        let finished = capture.finish(&cancel).unwrap();
        assert_eq!(finished.canonical_cuts(), Err(CanonicalFault::TooLarge));
        assert!(finished.raw().is_none());
    }
    #[test]
    fn twenty_sixth_selection_discards_backing_without_skipping_later_nodes() {
        let resolver = model(vec![json!({"small":true})], 40, None);
        let tenant = TenantId::new("tenant").unwrap();
        let cancel = Cancellation::default();
        ready(lineage::traverse(
            &resolver,
            &tenant,
            request(0, 100),
            &cancel,
        ))
        .unwrap();
        let expected_reads = resolver.calls.lock().unwrap().clone();
        assert_eq!(expected_reads.len(), 40);
        assert_eq!(expected_reads.last(), Some(&9));
        resolver.calls.lock().unwrap().clear();
        let mut capture = ManifestCapture::new(1, vec![], &cancel).unwrap();
        ready(capture.append(&resolver, &tenant, request(0, 100), &cancel)).unwrap();
        assert_eq!(*resolver.calls.lock().unwrap(), expected_reads);
        let finished = capture.finish(&cancel).unwrap();
        assert!(finished.selection_overflow());
        assert_eq!(finished.result_ids().count(), 26);
        assert!(finished.raw().is_none());
        assert_eq!(
            finished.canonical_cuts(),
            Err(CanonicalFault::SelectionOverflow)
        );
        let resolver = model(vec![json!({})], 40, Some(9));
        assert_eq!(
            ready(lineage::traverse(
                &resolver,
                &tenant,
                request(0, 100),
                &cancel
            )),
            Err(Failure::Unavailable)
        );
        let expected_reads = resolver.calls.lock().unwrap().clone();
        assert_eq!(expected_reads.len(), 40);
        assert_eq!(expected_reads.last(), Some(&9));
        resolver.calls.lock().unwrap().clear();
        let mut capture = ManifestCapture::new(1, vec![], &cancel).unwrap();
        assert_eq!(
            ready(capture.append(&resolver, &tenant, request(0, 100), &cancel)),
            Err(Failure::Unavailable)
        );
        assert_eq!(*resolver.calls.lock().unwrap(), expected_reads);
    }
    #[test]
    fn small_canonical_success_never_claims_to_bound_distinct_raw_numeric_backing() {
        let zeros = "0".repeat(1024 * 1024);
        let first = ssx_json::from_str(&format!("{{\"n\":0.{zeros}1}}")).unwrap();
        let second = ssx_json::from_str(&format!("{{\"n\":0.{zeros}2}}")).unwrap();
        let resolver = model(vec![first, second], 2, None);
        let tenant = TenantId::new("tenant").unwrap();
        let cancel = Cancellation::default();
        let mut capture = ManifestCapture::new(1, vec![], &cancel).unwrap();
        ready(capture.append(&resolver, &tenant, request(0, 100), &cancel)).unwrap();
        let finished = capture.finish(&cancel).unwrap();
        assert!(finished.canonical_cuts().unwrap().len() < 4096);
        let raw = finished.raw().unwrap();
        assert_eq!(raw.unique_details(), 2);
        assert!(raw.unique_detail_bytes().unwrap() > 2 * 1024 * 1024);
        assert_eq!(*resolver.calls.lock().unwrap(), vec![0, 1]);
        println!(
            "W07_CANONICAL_RAW_WITNESS canonical_bytes={} raw_unique_bytes={} raw_requested_backing={} unique_details={}",
            finished.canonical_cuts().unwrap().len(),
            raw.unique_detail_bytes().unwrap(),
            raw.owned_bytes().unwrap(),
            raw.unique_details()
        );
        let cancelled = Cancellation::default();
        cancelled.cancel();
        let mut capture = ManifestCapture::new(1, vec![], &cancelled).unwrap();
        resolver.calls.lock().unwrap().clear();
        assert_eq!(
            ready(capture.append(&resolver, &tenant, request(0, 100), &cancelled)),
            Err(Failure::Cancelled)
        );
        assert!(resolver.calls.lock().unwrap().is_empty());
    }
}
