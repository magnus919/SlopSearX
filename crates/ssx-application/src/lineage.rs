//! Deterministic bounded lineage traversal over tenant-bound readonly ports.
pub mod adjacency;
pub mod capture;
pub mod input;
pub mod resolver;
pub mod workflows;
use crate::{PortFuture, receipts::Cancellation};
use ssx_domain::{ArtifactRef, DomainError, LineageRelation, TenantId, WallClockTimestamp};
use ssx_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Incoming,
    Outgoing,
    Both,
}
impl Direction {
    pub fn parse(value: &str) -> Result<Self, DomainError> {
        match value {
            "incoming" => Ok(Self::Incoming),
            "outgoing" => Ok(Self::Outgoing),
            "both" => Ok(Self::Both),
            _ => Err(DomainError("direction must be outgoing, incoming, or both")),
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Incoming => "incoming",
            Self::Outgoing => "outgoing",
            Self::Both => "both",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Live,
    Expired,
    Missing,
    Unavailable,
    PolicyDenied,
}
impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Expired => "expired",
            Self::Missing => "missing",
            Self::Unavailable => "unavailable",
            Self::PolicyDenied => "policy_denied",
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub artifact: ArtifactRef,
    pub status: Status,
    pub created_at: Option<WallClockTimestamp>,
    pub expires_at: Option<WallClockTimestamp>,
    pub details: Option<ssx_json::Map<String, Value>>,
}
impl Node {
    pub fn missing(artifact: ArtifactRef, status: Status) -> Self {
        Self {
            artifact,
            status,
            created_at: None,
            expires_at: None,
            details: None,
        }
    }
    pub fn policy_denied(artifact: ArtifactRef) -> Self {
        let mut n = Self::missing(artifact, Status::PolicyDenied);
        n.details = Some(
            json!({"reason":"current_policy_denied"})
                .as_object()
                .unwrap()
                .clone(),
        );
        n
    }
    pub fn to_value(&self) -> Value {
        let mut v = json!({"artifact":self.artifact.to_value(),"status":self.status.as_str(),"created_at":self.created_at.map(|t|t.seconds()),"expires_at":self.expires_at.map(|t|t.seconds())});
        if let Some(details) = &self.details
            && !details.is_empty()
        {
            v["details"] = Value::Object(details.clone())
        }
        v
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edge {
    pub from: ArtifactRef,
    pub relation: LineageRelation,
    pub to: ArtifactRef,
}
type NodeKey = (String, String);
type EdgeKey = (NodeKey, String, NodeKey);
fn key(reference: &ArtifactRef) -> NodeKey {
    (
        reference.to_value()["kind"]
            .as_str()
            .expect("closed artifact kind")
            .into(),
        reference.id().into(),
    )
}
impl Edge {
    fn key(&self) -> EdgeKey {
        (
            key(&self.from),
            ssx_json::to_value(self.relation)
                .expect("closed lineage relation")
                .as_str()
                .unwrap()
                .into(),
            key(&self.to),
        )
    }
    pub fn to_value(&self) -> Value {
        json!({"from":self.from.to_value(),"relation":self.relation,"to":self.to.to_value()})
    }
    /// Persisted edges may contain extra fields; the original reader normalizes
    /// them through lineage_edge and discards corrupt relationships individually.
    pub fn from_value(value: &Value) -> Option<Self> {
        Some(Self {
            from: input::artifact(value.get("from")?).ok()?,
            relation: ssx_json::from_value(value.get("relation")?.clone()).ok()?,
            to: input::artifact(value.get("to")?).ok()?,
        })
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Resolved {
    pub node: Node,
    pub edges: Vec<Edge>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    Closed,
    Cancelled,
    Unavailable,
    Malformed,
}
pub trait Resolver: Send + Sync {
    /// Resolve current policy/lifecycle for this exact tenant and identity.
    /// No dispatch, persistence, lease renewal or retention extension is allowed.
    fn resolve<'a>(
        &'a self,
        tenant: &'a TenantId,
        reference: &'a ArtifactRef,
        cancellation: &'a Cancellation,
    ) -> PortFuture<'a, Result<Resolved, Failure>>;
    /// Same authority/read operation, with a compact adjacency representation.
    /// Existing injected resolvers retain their exact default behavior.
    fn resolve_compact<'a>(
        &'a self,
        tenant: &'a TenantId,
        reference: &'a ArtifactRef,
        cancellation: &'a Cancellation,
    ) -> PortFuture<'a, Result<adjacency::CompactResolved, Failure>> {
        Box::pin(async move {
            self.resolve(tenant, reference, cancellation)
                .await
                .map(Into::into)
        })
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Graph {
    pub root: ArtifactRef,
    pub direction: Direction,
    pub max_depth: usize,
    pub max_nodes: usize,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub truncated: bool,
}
impl Graph {
    pub fn to_value(&self) -> Value {
        let mut value = json!({"contract":"slopsearx.artifact_lineage","version":1,"root":self.root.to_value(),"direction":self.direction.as_str(),"max_depth":self.max_depth,"max_nodes":self.max_nodes,"nodes":null,"edges":null,"truncated":self.truncated,"counts":{"nodes":self.nodes.len(),"edges":self.edges.len()}});
        // Owned Values avoid a whole-document reparse when valid nested retained
        // fields are wrapped in a graph response.
        value["nodes"] = Value::Array(self.nodes.iter().map(Node::to_value).collect());
        value["edges"] = Value::Array(self.edges.iter().map(Edge::to_value).collect());
        value
    }
}
pub struct Request {
    pub root: ArtifactRef,
    pub direction: Direction,
    pub max_depth: usize,
    pub max_nodes: usize,
}
impl Request {
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.max_depth > 5 {
            return Err(DomainError("max_depth must be an integer from 0 to 5"));
        }
        if !(1..=100).contains(&self.max_nodes) {
            return Err(DomainError("max_nodes must be an integer from 1 to 100"));
        }
        Ok(())
    }
}
struct GraphBuild(Graph);
impl GraphBuild {
    fn new(request: &Request) -> Self {
        Self(Graph {
            root: request.root.clone(),
            direction: request.direction,
            max_depth: request.max_depth,
            max_nodes: request.max_nodes,
            nodes: vec![],
            edges: vec![],
            truncated: false,
        })
    }
}
impl capture::Build for GraphBuild {
    type Output = Graph;

    fn admit(&mut self, _: &ArtifactRef, _: u8) -> Result<(), Failure> {
        Ok(())
    }
    fn node(&mut self, node: Node, _: &Cancellation) -> Result<(), Failure> {
        self.0.nodes.push(node);
        Ok(())
    }
    fn edge(&mut self, edge: Edge, _: capture::IndexedEdge) {
        self.0.edges.push(edge);
    }
    fn finish(mut self, truncated: bool) -> Graph {
        self.0.truncated = truncated;
        self.0
    }
}
/// Original typed graph result; compact transport capture uses the same walk.
pub async fn traverse(
    resolver: &dyn Resolver,
    tenant: &TenantId,
    request: Request,
    cancellation: &Cancellation,
) -> Result<Graph, Failure> {
    walk::<GraphBuild, _>(resolver, tenant, request, cancellation, GraphBuild::new).await
}
/// Immutable output capture. Each same resolved node becomes one encoded
/// fragment before the next read. IDs and relation triples are retained once.
pub async fn capture(
    resolver: &dyn Resolver,
    tenant: &TenantId,
    request: Request,
    cancellation: &Cancellation,
) -> Result<capture::CapturedGraph, Failure> {
    walk::<capture::CompactBuild, _>(
        resolver,
        tenant,
        request,
        cancellation,
        capture::CompactBuild::new,
    )
    .await
}
/// Every expansion is sequential and bounded by existing node/depth caps.
/// A resolver must drain its operation before returning, including cancellation.
async fn walk<B: capture::Build, F: FnOnce(&Request) -> B>(
    resolver: &dyn Resolver,
    tenant: &TenantId,
    request: Request,
    cancellation: &Cancellation,
    build: F,
) -> Result<B::Output, Failure> {
    request.validate().map_err(|_| Failure::Malformed)?;
    let mut queue = VecDeque::from([(request.root.clone(), 0)]);
    let mut queued = BTreeMap::from([(key(&request.root), 0u8)]);
    let mut edge_keys = BTreeSet::new();
    let mut graph = build(&request);
    graph.admit(&request.root, 0)?;
    let mut truncated = false;
    while let Some((reference, depth)) = queue.pop_front() {
        if cancellation.is_cancelled() {
            return Err(Failure::Cancelled);
        }
        let mut adjacent = resolver
            .resolve_compact(tenant, &reference, cancellation)
            .await?;
        if cancellation.is_cancelled() {
            return Err(Failure::Cancelled);
        }
        if adjacent.node.artifact != reference {
            return Err(Failure::Malformed);
        }
        if adjacent.node.status == Status::PolicyDenied {
            adjacent.node = if adjacent
                .node
                .details
                .as_ref()
                .is_some_and(|d| !d.is_empty())
            {
                Node::policy_denied(reference.clone())
            } else {
                Node::missing(reference.clone(), Status::PolicyDenied)
            };
            adjacent.edges = adjacency::Adjacency::new(vec![], None);
        }
        graph.node(adjacent.node, cancellation)?;
        // Source truncation observes all adjacency before direction filtering.
        if depth >= request.max_depth {
            truncated |= !adjacent.edges.is_empty();
            continue;
        }
        for edge in adjacent.edges {
            if cancellation.is_cancelled() {
                return Err(Failure::Cancelled);
            }
            let mut candidates = Vec::with_capacity(2);
            if request.direction != Direction::Incoming && edge.from == reference {
                candidates.push(&edge.to);
            }
            if request.direction != Direction::Outgoing && edge.to == reference {
                candidates.push(&edge.from);
            }
            let mut include = false;
            for candidate in candidates {
                let k = key(candidate);
                if queued.contains_key(&k) {
                    include = true;
                    continue;
                }
                if queued.len() >= request.max_nodes {
                    truncated = true;
                    continue;
                }
                let index = u8::try_from(queued.len()).map_err(|_| Failure::Malformed)?;
                graph.admit(candidate, index)?;
                queued.insert(k, index);
                queue.push_back((candidate.clone(), depth + 1));
                include = true;
            }
            if include {
                // One endpoint is current, the other admitted. No unrelated
                // edge can insert an external identity or consume a slot.
                let index = capture::IndexedEdge {
                    from: *queued.get(&key(&edge.from)).ok_or(Failure::Malformed)?,
                    relation: capture::relation_index(edge.relation),
                    to: *queued.get(&key(&edge.to)).ok_or(Failure::Malformed)?,
                };
                if edge_keys.insert(index) {
                    graph.edge(edge, index);
                }
            }
        }
    }
    Ok(graph.finish(truncated))
}

#[cfg(test)]
mod source_cycle_contract {
    use super::*;
    use ssx_domain::ArtifactKind;
    struct Cycle {
        root: ArtifactRef,
        child: ArtifactRef,
        expired: ArtifactRef,
    }
    impl Resolver for Cycle {
        fn resolve<'a>(
            &'a self,
            _: &'a TenantId,
            reference: &'a ArtifactRef,
            _: &'a Cancellation,
        ) -> PortFuture<'a, Result<Resolved, Failure>> {
            Box::pin(async move {
                let child_edge = Edge {
                    from: self.child.clone(),
                    relation: LineageRelation::DerivedFrom,
                    to: self.root.clone(),
                };
                let mut node = Node::missing(reference.clone(), Status::Live);
                let edges = if reference == &self.root {
                    vec![
                        child_edge.clone(),
                        child_edge,
                        Edge {
                            from: self.expired.clone(),
                            relation: LineageRelation::DerivedFrom,
                            to: self.root.clone(),
                        },
                    ]
                } else if reference == &self.child {
                    vec![Edge {
                        from: self.root.clone(),
                        relation: LineageRelation::Contains,
                        to: self.child.clone(),
                    }]
                } else {
                    node.status = Status::Expired;
                    node.expires_at = Some(WallClockTimestamp::new(1.).unwrap());
                    vec![]
                };
                Ok(Resolved { node, edges })
            })
        }
    }
    fn ready<F: std::future::Future>(future: F) -> F::Output {
        let mut future = std::pin::pin!(future);
        match future
            .as_mut()
            .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()))
        {
            std::task::Poll::Ready(v) => v,
            std::task::Poll::Pending => panic!("pure source cycle resolver suspended"),
        }
    }
    #[test]
    fn original_cycle_duplicate_edges_and_expired_nodes_are_exact_and_deterministic() {
        let model = Cycle {
            root: ArtifactRef::new(ArtifactKind::Snapshot, "root").unwrap(),
            child: ArtifactRef::new(ArtifactKind::Result, "root:0").unwrap(),
            expired: ArtifactRef::new(ArtifactKind::Snapshot, "expired").unwrap(),
        };
        let tenant = TenantId::new("cycle-source").unwrap();
        let cancel = Cancellation::default();
        let request = || Request {
            root: model.root.clone(),
            direction: Direction::Both,
            max_depth: 5,
            max_nodes: 100,
        };
        let first = ready(traverse(&model, &tenant, request(), &cancel)).unwrap();
        let second = ready(traverse(&model, &tenant, request(), &cancel)).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first
                .nodes
                .iter()
                .map(|n| (&n.artifact, n.status))
                .collect::<Vec<_>>(),
            vec![
                (&model.root, Status::Live),
                (&model.child, Status::Live),
                (&model.expired, Status::Expired)
            ]
        );
        assert_eq!(
            first.nodes[2].expires_at,
            Some(WallClockTimestamp::new(1.).unwrap())
        );
        assert_eq!(first.edges.len(), 3);
        let actual: BTreeSet<_> = first
            .edges
            .iter()
            .map(|e| ssx_json::to_string(&e.to_value()).unwrap())
            .collect();
        let expected: BTreeSet<_> = [
            json!({"from":model.child.to_value(),"relation":"derived_from","to":model.root.to_value()}),
            json!({"from":model.expired.to_value(),"relation":"derived_from","to":model.root.to_value()}),
            json!({"from":model.root.to_value(),"relation":"contains","to":model.child.to_value()}),
        ].iter().map(|e| ssx_json::to_string(e).unwrap()).collect();
        assert_eq!(actual, expected);
        assert!(!first.truncated);
    }
}
