//! Sorted lazy result relationships. This changes representation, not the set or
//! order of source relationships. All generated identities preflight first.
use super::{Edge, Failure, Node, Resolved};
use ssx_domain::{ArtifactKind, ArtifactRef, LineageRelation};
use std::{iter::Peekable, vec::IntoIter};

pub struct CompactResolved {
    pub node: Node,
    pub edges: Adjacency,
}
impl From<Resolved> for CompactResolved {
    fn from(resolved: Resolved) -> Self {
        Self {
            node: resolved.node,
            edges: Adjacency::new(resolved.edges, None),
        }
    }
}
impl CompactResolved {
    pub fn materialize(self) -> Resolved {
        Resolved {
            node: self.node,
            edges: self.edges.collect(),
        }
    }
}
pub struct Adjacency {
    fixed: Peekable<IntoIter<Edge>>,
    results: Option<Peekable<Family>>,
    generated_backing_upper_bytes: usize,
}
impl Adjacency {
    pub fn new(mut fixed: Vec<Edge>, results: Option<Results>) -> Self {
        fixed.sort_by_key(Edge::key);
        let generated_backing_upper_bytes = results.as_ref().map_or(0, |r| {
            std::mem::size_of::<Option<Peekable<Family>>>() + r.parent.id().len() + 12288
        });
        Self {
            fixed: fixed.into_iter().peekable(),
            results: results.map(|r| Family::Results(r).peekable()),
            generated_backing_upper_bytes,
        }
    }
    pub fn with_members(mut fixed: Vec<Edge>, members: Members) -> Self {
        fixed.sort_by_key(Edge::key);
        let generated_backing_upper_bytes = std::mem::size_of::<Option<Peekable<Family>>>()
            + members.parent.id().len()
            + members.snapshot.capacity()
            + std::mem::size_of_val(members.indices.as_ref())
            + 12288;
        Self {
            fixed: fixed.into_iter().peekable(),
            results: Some(Family::Members(members).peekable()),
            generated_backing_upper_bytes,
        }
    }
    /// Requested family/peek storage + one peeked edge's bounded id capacity, excluding
    /// allocator overhead and explicit fixed edges (reported separately).
    pub fn generated_backing_upper_bytes(&self) -> usize {
        self.generated_backing_upper_bytes
    }
    pub fn is_empty(&mut self) -> bool {
        self.fixed.peek().is_none() && self.results.as_mut().is_none_or(|r| r.peek().is_none())
    }
}
impl Iterator for Adjacency {
    type Item = Edge;
    fn next(&mut self) -> Option<Self::Item> {
        let result = self.results.as_mut().and_then(Peekable::peek);
        let take_fixed = match (self.fixed.peek(), result) {
            (Some(fixed), Some(result)) => fixed.key() <= result.key(),
            (Some(_), None) => true,
            _ => false,
        };
        if take_fixed {
            self.fixed.next()
        } else {
            self.results.as_mut().and_then(Iterator::next)
        }
    }
}
/// Decimal suffix order is lexicographic:0,1,10,100,...,11,...,2,... . Do not
/// replace this with numeric order or allocate/sort all generated result IDs.
pub struct Results {
    parent: ArtifactRef,
    count: usize,
    next: Option<usize>,
}
impl Results {
    pub fn new(parent: ArtifactRef, count: usize) -> Result<Self, Failure> {
        // All suffixes are ASCII decimal and the longest has count-1's width.
        // Preserve malformed-reference precedence before entity projections.
        if count != 0 {
            ArtifactRef::new(
                ArtifactKind::Result,
                format!("{}:{}", parent.id(), count - 1),
            )
            .map_err(|_| Failure::Malformed)?;
        }
        Ok(Self {
            // Normalize a trusted caller's potentially over-capacity String.
            // ArtifactRef's derived String clone requests exactly its length.
            parent: parent.clone(),
            count,
            next: (count != 0).then_some(0),
        })
    }
    fn advance(&self, n: usize) -> Option<usize> {
        if n == 0 {
            return (self.count > 1).then_some(1);
        }
        if let Some(child) = n.checked_mul(10).filter(|v| *v < self.count) {
            return Some(child);
        }
        let mut n = n;
        while n % 10 == 9 || n.checked_add(1).is_none_or(|next| next >= self.count) {
            n /= 10;
            if n == 0 {
                return None;
            }
        }
        Some(n + 1)
    }
}
impl Iterator for Results {
    type Item = Edge;
    fn next(&mut self) -> Option<Self::Item> {
        let n = self.next?;
        self.next = self.advance(n);
        Some(Edge {
            from: ArtifactRef::new(ArtifactKind::Result, format!("{}:{n}", self.parent.id()))
                .expect("all generated identities passed longest-suffix preflight"),
            relation: LineageRelation::DerivedFrom,
            to: self.parent.clone(),
        })
    }
}

enum Family {
    Results(Results),
    Members(Members),
}
impl Iterator for Family {
    type Item = Edge;
    fn next(&mut self) -> Option<Edge> {
        match self {
            Self::Results(r) => r.next(),
            Self::Members(m) => m.next(),
        }
    }
}
/// Identified group membership is indexed, never an eager vector of long IDs.
pub struct Members {
    parent: ArtifactRef,
    snapshot: String,
    indices: Box<[usize]>,
    position: usize,
}
impl Members {
    pub fn new(
        parent: ArtifactRef,
        snapshot: &str,
        mut indices: Vec<usize>,
    ) -> Result<Self, Failure> {
        // Preserve validation in original member order before sorting/returning a
        // node, even for max_depth0 or a full node queue.
        for n in &indices {
            ArtifactRef::new(ArtifactKind::Result, format!("{snapshot}:{n}"))
                .map_err(|_| Failure::Malformed)?;
        }
        // In-place sort has bounded stack; comparator allocates only two <=20-byte
        // decimal strings, independent of the potentially long snapshot prefix.
        indices.sort_unstable_by_key(|a| a.to_string());
        Ok(Self {
            parent: parent.clone(),
            snapshot: snapshot.to_owned(),
            indices: indices.into_boxed_slice(),
            position: 0,
        })
    }
}
impl Iterator for Members {
    type Item = Edge;
    fn next(&mut self) -> Option<Edge> {
        let n = *self.indices.get(self.position)?;
        self.position += 1;
        Some(Edge {
            from: self.parent.clone(),
            relation: LineageRelation::Contains,
            to: ArtifactRef::new(ArtifactKind::Result, format!("{}:{n}", self.snapshot))
                .expect("all member refs preflighted before output"),
        })
    }
}
