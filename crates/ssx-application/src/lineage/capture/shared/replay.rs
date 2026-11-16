//! Immutable owner-bound graph replay; no transport, quota or output activation.
use super::{Captured, CapturedSet, relation_name};
use crate::lineage::Failure;
use std::sync::Arc;
#[derive(Clone, Copy)]
enum Phase {
    Header,
    Root,
    DirectionKey,
    Direction,
    DepthKey,
    Depth,
    BoundKey,
    Bound,
    NodesKey,
    NodeComma,
    NodeHeader,
    DetailKey,
    Detail,
    DetailEnd,
    EdgesKey,
    EdgeComma,
    EdgeFromKey,
    EdgeFrom,
    RelationKey,
    Relation,
    EdgeToKey,
    EdgeTo,
    EdgeEnd,
    TruncatedKey,
    Truncated,
    CountNodesKey,
    CountNodes,
    CountEdgesKey,
    CountEdges,
    End,
    Done,
}
/// Position belongs to exactly one graph; exposed crate-only for an embedded
/// manifest's same-owner graph replay. Public replay always owns its Arc.
pub(crate) struct Position {
    graph: usize,
    phase: Phase,
    offset: usize,
    node: usize,
    edge: usize,
    decimal: [u8; 20],
}
impl Position {
    pub(crate) fn new(graph: usize) -> Self {
        Self {
            graph,
            phase: Phase::Header,
            offset: 0,
            node: 0,
            edge: 0,
            decimal: [0; 20],
        }
    }
    pub(crate) fn done(&self) -> bool {
        matches!(self.phase, Phase::Done)
    }
    fn graph<'a>(&self, owner: &'a CapturedSet) -> Result<&'a Captured, Failure> {
        owner.graphs.get(self.graph).ok_or(Failure::Malformed)
    }
    fn part<'a>(&'a mut self, owner: &'a CapturedSet) -> Result<&'a [u8], Failure> {
        let graph = self.graph(owner)?;
        let number = match self.phase {
            Phase::Depth => Some(graph.max_depth),
            Phase::Bound => Some(graph.max_nodes),
            Phase::CountNodes => Some(graph.nodes.len()),
            Phase::CountEdges => Some(graph.edges.len()),
            _ => None,
        };
        if let Some(mut value) = number {
            let mut at = self.decimal.len();
            loop {
                at -= 1;
                self.decimal[at] = b'0' + (value % 10) as u8;
                value /= 10;
                if value == 0 {
                    break;
                }
            }
            return Ok(&self.decimal[at..]);
        }
        Ok(match self.phase {
            Phase::Header => {
                b"{\"contract\":\"slopsearx.artifact_lineage\",\"version\":1,\"root\":"
            }
            Phase::Root => graph
                .references
                .first()
                .ok_or(Failure::Malformed)?
                .as_bytes(),
            Phase::DirectionKey => b",\"direction\":\"",
            Phase::Direction => graph.direction.as_str().as_bytes(),
            Phase::DepthKey => b"\",\"max_depth\":",
            Phase::BoundKey => b",\"max_nodes\":",
            Phase::NodesKey => b",\"nodes\":[",
            Phase::NodeComma | Phase::EdgeComma => b",",
            Phase::NodeHeader => {
                let node = graph.nodes.get(self.node).ok_or(Failure::Malformed)?;
                if node.detail.is_some() {
                    node.header
                        .as_bytes()
                        .strip_suffix(b"}")
                        .ok_or(Failure::Malformed)?
                } else {
                    node.header.as_bytes()
                }
            }
            Phase::DetailKey => b",\"details\":",
            Phase::Detail => {
                let index = graph
                    .nodes
                    .get(self.node)
                    .and_then(|node| node.detail)
                    .ok_or(Failure::Malformed)?;
                owner
                    .pool
                    .0
                    .get(usize::from(index))
                    .ok_or(Failure::Malformed)?
                    .bytes
                    .as_bytes()
            }
            Phase::DetailEnd | Phase::EdgeEnd => b"}",
            Phase::EdgesKey => b"],\"edges\":[",
            Phase::EdgeFromKey => b"{\"from\":",
            Phase::EdgeFrom => {
                let edge = graph.edges.get(self.edge).ok_or(Failure::Malformed)?;
                graph
                    .references
                    .get(usize::from(edge.from))
                    .ok_or(Failure::Malformed)?
                    .as_bytes()
            }
            Phase::RelationKey => b",\"relation\":\"",
            Phase::Relation => relation_name(
                graph
                    .edges
                    .get(self.edge)
                    .ok_or(Failure::Malformed)?
                    .relation,
            )
            .ok_or(Failure::Malformed)?
            .as_bytes(),
            Phase::EdgeToKey => b"\",\"to\":",
            Phase::EdgeTo => {
                let edge = graph.edges.get(self.edge).ok_or(Failure::Malformed)?;
                graph
                    .references
                    .get(usize::from(edge.to))
                    .ok_or(Failure::Malformed)?
                    .as_bytes()
            }
            Phase::TruncatedKey => b"],\"truncated\":",
            Phase::Truncated => {
                if graph.truncated {
                    b"true"
                } else {
                    b"false"
                }
            }
            Phase::CountNodesKey => b",\"counts\":{\"nodes\":",
            Phase::CountEdgesKey => b",\"edges\":",
            Phase::End => b"}}",
            Phase::Depth | Phase::Bound | Phase::CountNodes | Phase::CountEdges | Phase::Done => {
                return Err(Failure::Malformed);
            }
        })
    }
    fn advance(&mut self, owner: &CapturedSet) -> Result<(), Failure> {
        let graph = self.graph(owner)?;
        self.offset = 0;
        self.phase = match self.phase {
            Phase::Header => Phase::Root,
            Phase::Root => Phase::DirectionKey,
            Phase::DirectionKey => Phase::Direction,
            Phase::Direction => Phase::DepthKey,
            Phase::DepthKey => Phase::Depth,
            Phase::Depth => Phase::BoundKey,
            Phase::BoundKey => Phase::Bound,
            Phase::Bound => Phase::NodesKey,
            Phase::NodesKey => {
                if graph.nodes.is_empty() {
                    Phase::EdgesKey
                } else {
                    Phase::NodeHeader
                }
            }
            Phase::NodeComma => Phase::NodeHeader,
            Phase::NodeHeader => {
                if graph
                    .nodes
                    .get(self.node)
                    .ok_or(Failure::Malformed)?
                    .detail
                    .is_some()
                {
                    Phase::DetailKey
                } else {
                    self.node += 1;
                    if self.node < graph.nodes.len() {
                        Phase::NodeComma
                    } else {
                        Phase::EdgesKey
                    }
                }
            }
            Phase::DetailKey => Phase::Detail,
            Phase::Detail => Phase::DetailEnd,
            Phase::DetailEnd => {
                self.node += 1;
                if self.node < graph.nodes.len() {
                    Phase::NodeComma
                } else {
                    Phase::EdgesKey
                }
            }
            Phase::EdgesKey => {
                if graph.edges.is_empty() {
                    Phase::TruncatedKey
                } else {
                    Phase::EdgeFromKey
                }
            }
            Phase::EdgeComma => Phase::EdgeFromKey,
            Phase::EdgeFromKey => Phase::EdgeFrom,
            Phase::EdgeFrom => Phase::RelationKey,
            Phase::RelationKey => Phase::Relation,
            Phase::Relation => Phase::EdgeToKey,
            Phase::EdgeToKey => Phase::EdgeTo,
            Phase::EdgeTo => Phase::EdgeEnd,
            Phase::EdgeEnd => {
                self.edge += 1;
                if self.edge < graph.edges.len() {
                    Phase::EdgeComma
                } else {
                    Phase::TruncatedKey
                }
            }
            Phase::TruncatedKey => Phase::Truncated,
            Phase::Truncated => Phase::CountNodesKey,
            Phase::CountNodesKey => Phase::CountNodes,
            Phase::CountNodes => Phase::CountEdgesKey,
            Phase::CountEdgesKey => Phase::CountEdges,
            Phase::CountEdges => Phase::End,
            Phase::End => Phase::Done,
            Phase::Done => return Err(Failure::Malformed),
        };
        Ok(())
    }
    pub(crate) fn visit_next(
        &mut self,
        owner: &CapturedSet,
        output: &mut dyn FnMut(&[u8]) -> Result<(), Failure>,
    ) -> Result<bool, Failure> {
        if self.done() {
            return Ok(false);
        }
        let offset = self.offset;
        {
            let part = self.part(owner)?;
            output(part.get(offset..).ok_or(Failure::Malformed)?)?;
        }
        self.advance(owner)?;
        Ok(true)
    }
    pub(crate) fn copy(
        &mut self,
        owner: &CapturedSet,
        target: &mut [u8],
    ) -> Result<usize, Failure> {
        let mut written = 0;
        while written < target.len() && !self.done() {
            let offset = self.offset;
            let (length, count) = {
                let part = self.part(owner)?;
                let rest = part.get(offset..).ok_or(Failure::Malformed)?;
                let count = rest.len().min(target.len() - written);
                target[written..written + count].copy_from_slice(&rest[..count]);
                (part.len(), count)
            };
            if count == 0 {
                return Err(Failure::Malformed);
            }
            written += count;
            self.offset += count;
            if self.offset == length {
                self.advance(owner)?;
            }
        }
        Ok(written)
    }
}
/// Non-Clone position bound to the same immutable owner for its entire life.
/// The caller supplies bounded output storage; this type allocates no chunks.
pub struct GraphReplay {
    owner: Arc<CapturedSet>,
    position: Position,
}
impl GraphReplay {
    pub fn copy(&mut self, target: &mut [u8]) -> Result<usize, Failure> {
        self.position.copy(&self.owner, target)
    }
    pub fn done(&self) -> bool {
        self.position.done()
    }
}
impl CapturedSet {
    pub fn graph_replay(self: Arc<Self>, index: usize) -> Result<GraphReplay, Failure> {
        if self.graphs.get(index).is_none() {
            return Err(Failure::Malformed);
        }
        Ok(GraphReplay {
            owner: self,
            position: Position::new(index),
        })
    }
}
