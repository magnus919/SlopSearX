//! Same-owner detail interning only; no quota/profile or transport activation.
pub mod manifest;
pub mod replay;
use super::{Build, IndexedEdge, NODE_BYTES, REFERENCE_BYTES, fragment, relation_name, visit};
use crate::lineage::{Direction, Edge, Failure, Node, Request, Resolver, Status};
use crate::receipts::Cancellation;
use sha2::{Digest, Sha256};
use ssx_domain::{ArtifactKind, ArtifactRef, TenantId};
use ssx_json::{Value, json};

struct Detail {
    digest: [u8; 32],
    bytes: Box<str>,
}
#[derive(Default)]
struct Pool(Vec<Detail>);
impl Pool {
    fn intern(
        &mut self,
        value: &Value,
        digest: [u8; 32],
        len: usize,
        cancel: &Cancellation,
    ) -> Result<u16, Failure> {
        for (index, candidate) in self.0.iter().enumerate() {
            if candidate.digest != digest || candidate.bytes.len() != len {
                continue;
            }
            let mut offset = 0usize;
            let mut equal = true;
            visit(value, 1, cancel, &mut |part| {
                let end = offset.checked_add(part.len()).ok_or(Failure::Malformed)?;
                equal &= candidate.bytes.as_bytes().get(offset..end) == Some(part);
                offset = end;
                Ok(())
            })?;
            if equal && offset == len {
                return u16::try_from(index).map_err(|_| Failure::Malformed);
            }
        }
        let index = u16::try_from(self.0.len()).map_err(|_| Failure::Malformed)?;
        let mut bytes = Vec::with_capacity(len);
        visit(value, 1, cancel, &mut |part| {
            let next = bytes
                .len()
                .checked_add(part.len())
                .ok_or(Failure::Malformed)?;
            if next > len {
                return Err(Failure::Malformed);
            }
            bytes.extend_from_slice(part);
            Ok(())
        })?;
        if bytes.len() != len {
            return Err(Failure::Malformed);
        }
        let bytes = String::from_utf8(bytes)
            .map_err(|_| Failure::Malformed)?
            .into_boxed_str();
        self.0.push(Detail { digest, bytes });
        Ok(index)
    }
    fn dynamic_bytes(&self) -> Option<usize> {
        let mut total = self
            .0
            .capacity()
            .checked_mul(std::mem::size_of::<Detail>())?;
        for entry in &self.0 {
            total = total.checked_add(entry.bytes.len())?;
        }
        Some(total)
    }
}
struct CapturedNode {
    header: Box<str>,
    detail: Option<u16>,
}
struct Captured {
    direction: Direction,
    max_depth: usize,
    max_nodes: usize,
    references: Vec<Box<str>>,
    nodes: Vec<CapturedNode>,
    edges: Vec<IndexedEdge>,
    selected_results: Vec<Box<str>>,
    truncated: bool,
}
impl Captured {
    fn dynamic_bytes(&self) -> Option<usize> {
        let mut total = 0usize;
        for (capacity, size) in [
            (self.references.capacity(), std::mem::size_of::<Box<str>>()),
            (self.nodes.capacity(), std::mem::size_of::<CapturedNode>()),
            (self.edges.capacity(), std::mem::size_of::<IndexedEdge>()),
            (
                self.selected_results.capacity(),
                std::mem::size_of::<Box<str>>(),
            ),
        ] {
            total = total.checked_add(capacity.checked_mul(size)?)?;
        }
        for bytes in self.references.iter().chain(&self.selected_results) {
            total = total.checked_add(bytes.len())?;
        }
        for node in &self.nodes {
            total = total.checked_add(node.header.len())?;
        }
        Some(total)
    }
}
/// Only this owner can pair graph descriptors with their detail pool. No Clone,
/// independent graph/token return, hash-only equality or cross-owner lookup.
pub struct CapturedSet {
    pool: Pool,
    graphs: Vec<Captured>,
    max_roots: usize,
    attempts: usize,
}
impl CapturedSet {
    /// Trusted caller already performed the existing semantic root-count check.
    /// Zero roots is legal for an explicit-result manifest; direct lineage uses1.
    pub fn new(max_roots: usize) -> Result<Self, Failure> {
        if max_roots > 25 {
            return Err(Failure::Malformed);
        }
        Ok(Self {
            pool: Pool::default(),
            graphs: Vec::with_capacity(max_roots),
            max_roots,
            attempts: 0,
        })
    }
    /// Same resolver walk. Failed attempts consume a slot too, so a caller
    /// cannot retain fragments from arbitrarily many unsuccessful walks.
    pub async fn append(
        &mut self,
        resolver: &dyn Resolver,
        tenant: &TenantId,
        request: Request,
        cancel: &Cancellation,
    ) -> Result<usize, Failure> {
        if self.attempts >= self.max_roots {
            return Err(Failure::Malformed);
        }
        self.attempts += 1;
        let graph = crate::lineage::walk(resolver, tenant, request, cancel, |request| {
            SharedBuild::new(request, &mut self.pool)
        })
        .await?;
        let index = self.graphs.len();
        self.graphs.push(graph);
        Ok(index)
    }
    pub fn len(&self) -> usize {
        self.graphs.len()
    }
    pub fn is_empty(&self) -> bool {
        self.graphs.is_empty()
    }
    pub fn unique_details(&self) -> usize {
        self.pool.0.len()
    }
    pub fn unique_detail_bytes(&self) -> Option<usize> {
        self.pool
            .0
            .iter()
            .try_fold(0usize, |n, detail| n.checked_add(detail.bytes.len()))
    }
    /// Exact requested retained backing: owner/slot capacities plus all Box<str>
    /// lengths. Decoder/encoding/sort/boxing growth scratch and allocator/RSS are
    /// explicitly outside this number, and need separate later quota credits.
    pub fn owned_bytes(&self) -> Option<usize> {
        let mut total = std::mem::size_of::<Self>()
            .checked_add(self.pool.dynamic_bytes()?)?
            .checked_add(
                self.graphs
                    .capacity()
                    .checked_mul(std::mem::size_of::<Captured>())?,
            )?;
        for graph in &self.graphs {
            total = total.checked_add(graph.dynamic_bytes()?)?;
        }
        Some(total)
    }
    pub fn selected_results(&self, index: usize) -> Option<impl Iterator<Item = &str>> {
        self.graphs
            .get(index)
            .map(|graph| graph.selected_results.iter().map(AsRef::as_ref))
    }
    /// Synchronous byte visitation for equivalence/hash/count qualification.
    /// Uses the same position formatter as owner-bound replay. This synchronous
    /// function is NOT a streaming transport or a whole-payload allocation.
    pub fn visit_graph(
        &self,
        index: usize,
        output: &mut dyn FnMut(&[u8]) -> Result<(), Failure>,
    ) -> Result<(), Failure> {
        if self.graphs.get(index).is_none() {
            return Err(Failure::Malformed);
        }
        let mut position = replay::Position::new(index);
        while position.visit_next(self, output)? {}
        Ok(())
    }
}
struct SharedBuild<'a> {
    graph: Captured,
    pool: &'a mut Pool,
}
impl<'a> SharedBuild<'a> {
    fn new(request: &Request, pool: &'a mut Pool) -> Self {
        Self {
            pool,
            graph: Captured {
                direction: request.direction,
                max_depth: request.max_depth,
                max_nodes: request.max_nodes,
                references: Vec::with_capacity(request.max_nodes),
                nodes: Vec::with_capacity(request.max_nodes),
                edges: Vec::new(),
                selected_results: Vec::new(),
                truncated: false,
            },
        }
    }
}
impl Build for SharedBuild<'_> {
    type Output = Captured;
    fn admit(&mut self, reference: &ArtifactRef, index: u8) -> Result<(), Failure> {
        if usize::from(index) != self.graph.references.len() {
            return Err(Failure::Malformed);
        }
        let encoded = ssx_json::to_string(&reference.to_value()).map_err(|_| Failure::Malformed)?;
        if encoded.len() > REFERENCE_BYTES {
            return Err(Failure::Malformed);
        }
        self.graph.references.push(encoded.into_boxed_str());
        Ok(())
    }
    fn node(&mut self, node: Node, cancel: &Cancellation) -> Result<(), Failure> {
        let Node {
            artifact,
            status,
            created_at,
            expires_at,
            details,
        } = node;
        if artifact.kind() == ArtifactKind::Result && status == Status::Live {
            self.graph.selected_results.push(artifact.id().into());
        }
        let header = fragment(
            &json!({"artifact":artifact.to_value(),"status":status.as_str(),"created_at":created_at.map(|t|t.seconds()),"expires_at":expires_at.map(|t|t.seconds())}),
            cancel,
            NODE_BYTES,
        )?;
        let detail = match details.filter(|details| !details.is_empty()) {
            None => None,
            Some(details) => {
                let value = Value::Object(details);
                let mut len = 0usize;
                let mut hash = Sha256::new();
                visit(&value, 1, cancel, &mut |part| {
                    len = len.checked_add(part.len()).ok_or(Failure::Malformed)?;
                    if header
                        .len()
                        .checked_add(11)
                        .and_then(|n| n.checked_add(len))
                        .is_none_or(|n| n > NODE_BYTES)
                    {
                        return Err(Failure::Malformed);
                    }
                    hash.update(part);
                    Ok(())
                })?;
                let mut digest = [0; 32];
                digest.copy_from_slice(&hash.finalize());
                Some(self.pool.intern(&value, digest, len, cancel)?)
            }
        };
        self.graph.nodes.push(CapturedNode { header, detail });
        Ok(())
    }
    fn edge(&mut self, _: Edge, edge: IndexedEdge) {
        self.graph.edges.push(edge);
    }
    fn finish(mut self, truncated: bool) -> Captured {
        self.graph.truncated = truncated;
        self.graph
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        PortFuture,
        lineage::{self, Resolved},
    };
    use ssx_domain::{LineageRelation, WallClockTimestamp};
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
            Poll::Pending => panic!("pure synthetic resolver must be immediately ready"),
        }
    }
    fn reference(n: usize) -> ArtifactRef {
        ArtifactRef::new(ArtifactKind::Result, format!("snapshot:{n}")).unwrap()
    }
    struct Model {
        details: Value,
        calls: Mutex<Vec<String>>,
        fail: Option<String>,
    }
    impl Resolver for Model {
        fn resolve<'a>(
            &'a self,
            _: &'a TenantId,
            reference: &'a ArtifactRef,
            _: &'a Cancellation,
        ) -> PortFuture<'a, Result<Resolved, Failure>> {
            Box::pin(async move {
                self.calls.lock().unwrap().push(reference.id().into());
                if self.fail.as_deref() == Some(reference.id()) {
                    return Err(Failure::Unavailable);
                }
                Ok(Resolved {
                    node: Node {
                        artifact: reference.clone(),
                        status: Status::Live,
                        created_at: Some(WallClockTimestamp::new(1.).unwrap()),
                        expires_at: None,
                        details: Some(self.details.as_object().unwrap().clone()),
                    },
                    edges: (0..4)
                        .map(|n| Edge {
                            from: reference.clone(),
                            relation: LineageRelation::DerivedFrom,
                            to: super::tests::reference(n),
                        })
                        .collect(),
                })
            })
        }
    }
    fn request(root: usize) -> Request {
        Request {
            root: reference(root),
            direction: Direction::Both,
            max_depth: 2,
            max_nodes: 100,
        }
    }
    #[test]
    fn same_owner_interns_repeated_details_across_distinct_roots_without_skipping_reads() {
        let details=ssx_json::from_str::<Value>(&format!(r#"{{"text":"{}","opaque":{{"n":18446744073709551617,"float":1.000e0,"$serde_json::private::Number":"literal"}}}}"#,"x".repeat(32768))).unwrap();
        let model = Model {
            details,
            calls: Mutex::default(),
            fail: None,
        };
        let tenant = TenantId::new("tenant").unwrap();
        let cancel = Cancellation::default();
        let mut set = CapturedSet::new(2).unwrap();
        let mut old_backing = 0;
        for root in 0..2 {
            model.calls.lock().unwrap().clear();
            let expected = ready(lineage::traverse(&model, &tenant, request(root), &cancel))
                .unwrap()
                .to_value();
            let expected_reads = model.calls.lock().unwrap().clone();
            model.calls.lock().unwrap().clear();
            let index = ready(set.append(&model, &tenant, request(root), &cancel)).unwrap();
            assert_eq!(*model.calls.lock().unwrap(), expected_reads);
            let mut bytes = Vec::new();
            set.visit_graph(index, &mut |part| {
                bytes.extend_from_slice(part);
                Ok(())
            })
            .unwrap();
            assert_eq!(bytes, ssx_json::to_vec(&expected).unwrap());
            assert_eq!(set.selected_results(index).unwrap().count(), 4);
            old_backing += ready(lineage::capture(&model, &tenant, request(root), &cancel))
                .unwrap()
                .owned_bytes()
                .unwrap();
        }
        assert_eq!(set.len(), 2);
        assert_eq!(set.unique_details(), 1);
        assert!(set.owned_bytes().unwrap() < old_backing / 2);
        println!(
            "W07_DETAIL_INTERNING graphs=2 resolved_nodes=8 unique_details=1 unique_detail_bytes={} requested_set_backing_bytes={} old_separate_graph_backing_bytes={old_backing} allocator_or_rss_claim=false",
            set.unique_detail_bytes().unwrap(),
            set.owned_bytes().unwrap()
        );
    }
    #[test]
    fn full_streamed_bytes_prevent_hash_collision_and_raw_spelling_or_order_merges() {
        let cancel = Cancellation::default();
        let mut pool = Pool::default();
        let value: Value = ssx_json::from_str(r#"{"x":2}"#).unwrap();
        let encoded = ssx_json::to_vec(&value).unwrap();
        let mut digest = [0; 32];
        digest.copy_from_slice(&Sha256::digest(&encoded));
        // Deliberately forge equal candidate hash/length to test full byte equality.
        pool.0.push(Detail {
            digest,
            bytes: r#"{"x":1}"#.into(),
        });
        assert_eq!(
            pool.intern(&value, digest, encoded.len(), &cancel).unwrap(),
            1
        );
        assert_eq!(
            pool.intern(&value, digest, encoded.len(), &cancel).unwrap(),
            1
        );
        assert_eq!(pool.0.len(), 2);
        for raw in [
            r#"{"x":1e0,"z":null}"#,
            r#"{"x":1.0,"z":null}"#,
            r#"{"z":null,"x":1.0}"#,
            r#"{"$serde_json::private::Number":"literal"}"#,
        ] {
            let value: Value = ssx_json::from_str(raw).unwrap();
            let bytes = ssx_json::to_vec(&value).unwrap();
            let mut digest = [0; 32];
            digest.copy_from_slice(&Sha256::digest(&bytes));
            let index = pool.intern(&value, digest, bytes.len(), &cancel).unwrap();
            assert_eq!(pool.0[usize::from(index)].bytes.as_bytes(), bytes);
        }
        assert_eq!(pool.0.len(), 6);
    }
    #[test]
    fn attempts_bound_failed_retention_and_precancel_without_changing_later_error_order() {
        let model = Model {
            details: json!({"same":"retained-before-later-error"}),
            calls: Mutex::default(),
            fail: Some(reference(2).id().into()),
        };
        let tenant = TenantId::new("tenant").unwrap();
        let mut set = CapturedSet::new(2).unwrap();
        for _ in 0..2 {
            assert!(matches!(
                ready(set.append(&model, &tenant, request(0), &Cancellation::default())),
                Err(Failure::Unavailable)
            ));
        }
        let reads = model.calls.lock().unwrap().len();
        assert_eq!(reads, 6);
        assert_eq!(set.len(), 0);
        assert_eq!(set.unique_details(), 1);
        assert!(matches!(
            ready(set.append(&model, &tenant, request(0), &Cancellation::default())),
            Err(Failure::Malformed)
        ));
        assert_eq!(model.calls.lock().unwrap().len(), reads);
        let mut set = CapturedSet::new(1).unwrap();
        let cancel = Cancellation::default();
        cancel.cancel();
        assert!(matches!(
            ready(set.append(&model, &tenant, request(0), &cancel)),
            Err(Failure::Cancelled)
        ));
        assert_eq!(model.calls.lock().unwrap().len(), reads);
        assert_eq!(set.unique_details(), 0);
        assert!(CapturedSet::new(26).is_err());
        assert!(CapturedSet::new(0).unwrap().is_empty());
    }
}
