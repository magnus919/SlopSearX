//! Resumable byte replay bound to one immutable captured manifest.
//! This component does not select a handler/output limit or acquire quota.
use super::{CapturedManifest, Failure, map_lineage};
use crate::lineage::capture::shared::replay::Position as GraphPosition;
use std::sync::Arc;

#[derive(Clone, Copy)]
enum Phase {
    Header,
    ItemsKey,
    ItemComma,
    Item,
    CutsKey,
    GraphComma,
    Graph,
    NoteKey,
    Note,
    SourceKey,
    Source,
    ArtifactKey,
    Artifact,
    LineageKey,
    Lineage,
    End,
    Done,
}
/// Private position; its public wrapper permanently binds the immutable owner.
pub(super) struct Position {
    phase: Phase,
    offset: usize,
    item: usize,
    graph_index: usize,
    graph: GraphPosition,
}
impl Position {
    pub(super) fn new() -> Self {
        Self {
            phase: Phase::Header,
            offset: 0,
            item: 0,
            graph_index: 0,
            graph: GraphPosition::new(0),
        }
    }
    fn done(&self) -> bool {
        matches!(self.phase, Phase::Done)
    }
    fn part<'a>(&self, owner: &'a CapturedManifest) -> Result<&'a [u8], Failure> {
        Ok(match self.phase {
            Phase::Header => owner
                .header
                .as_bytes()
                .strip_suffix(b"}")
                .ok_or(Failure::Malformed)?,
            Phase::ItemsKey => b",\"items\":[",
            Phase::ItemComma | Phase::GraphComma => b",",
            Phase::Item => owner
                .items
                .get(self.item)
                .ok_or(Failure::Malformed)?
                .as_bytes(),
            Phase::CutsKey => b"],\"lineage_cuts\":[",
            Phase::NoteKey => b"],\"observations_verified\":false,\"verification_note\":",
            Phase::Note => owner.note.as_bytes(),
            Phase::SourceKey => b",\"source\":",
            Phase::Source => owner.source.as_ref().ok_or(Failure::Malformed)?.as_bytes(),
            Phase::ArtifactKey => b",\"artifact\":",
            Phase::Artifact => owner.artifact.as_bytes(),
            Phase::LineageKey => b",\"lineage\":",
            Phase::Lineage => owner.lineage.as_bytes(),
            Phase::End => b"}",
            Phase::Graph | Phase::Done => return Err(Failure::Malformed),
        })
    }
    fn advance(&mut self, owner: &CapturedManifest) -> Result<(), Failure> {
        self.offset = 0;
        self.phase = match self.phase {
            Phase::Header => Phase::ItemsKey,
            Phase::ItemsKey => {
                if owner.items.is_empty() {
                    Phase::CutsKey
                } else {
                    Phase::Item
                }
            }
            Phase::ItemComma => Phase::Item,
            Phase::Item => {
                self.item += 1;
                if self.item < owner.items.len() {
                    Phase::ItemComma
                } else {
                    Phase::CutsKey
                }
            }
            Phase::CutsKey => {
                if owner.graphs.is_empty() {
                    Phase::NoteKey
                } else {
                    Phase::Graph
                }
            }
            Phase::GraphComma => Phase::Graph,
            Phase::Graph => {
                self.graph_index += 1;
                if self.graph_index < owner.graphs.len() {
                    self.graph = GraphPosition::new(self.graph_index);
                    Phase::GraphComma
                } else {
                    Phase::NoteKey
                }
            }
            Phase::NoteKey => Phase::Note,
            Phase::Note => {
                if owner.source.is_some() {
                    Phase::SourceKey
                } else {
                    Phase::ArtifactKey
                }
            }
            Phase::SourceKey => Phase::Source,
            Phase::Source => Phase::ArtifactKey,
            Phase::ArtifactKey => Phase::Artifact,
            Phase::Artifact => Phase::LineageKey,
            Phase::LineageKey => Phase::Lineage,
            Phase::Lineage => Phase::End,
            Phase::End => Phase::Done,
            Phase::Done => return Err(Failure::Malformed),
        };
        Ok(())
    }
    pub(super) fn visit_next(
        &mut self,
        owner: &CapturedManifest,
        output: &mut dyn FnMut(&[u8]) -> Result<(), Failure>,
    ) -> Result<bool, Failure> {
        if self.done() {
            return Ok(false);
        }
        if matches!(self.phase, Phase::Graph) {
            let mut output_error = None;
            let result = self.graph.visit_next(&owner.graphs, &mut |bytes| {
                output(bytes).map_err(|error| {
                    output_error = Some(error);
                    crate::lineage::Failure::Unavailable
                })
            });
            if let Some(error) = output_error {
                return Err(error);
            }
            result.map_err(map_lineage)?;
            if self.graph.done() {
                self.advance(owner)?;
            }
        } else {
            output(
                self.part(owner)?
                    .get(self.offset..)
                    .ok_or(Failure::Malformed)?,
            )?;
            self.advance(owner)?;
        }
        Ok(true)
    }
    fn copy(&mut self, owner: &CapturedManifest, target: &mut [u8]) -> Result<usize, Failure> {
        let mut written = 0;
        while written < target.len() && !self.done() {
            if matches!(self.phase, Phase::Graph) {
                let count = self
                    .graph
                    .copy(&owner.graphs, &mut target[written..])
                    .map_err(map_lineage)?;
                written += count;
                if self.graph.done() {
                    self.advance(owner)?;
                } else if count == 0 {
                    return Err(Failure::Malformed);
                }
            } else {
                let part = self.part(owner)?;
                let rest = part.get(self.offset..).ok_or(Failure::Malformed)?;
                let count = rest.len().min(target.len() - written);
                if count == 0 {
                    return Err(Failure::Malformed);
                }
                target[written..written + count].copy_from_slice(&rest[..count]);
                written += count;
                self.offset += count;
                if self.offset == part.len() {
                    self.advance(owner)?;
                }
            }
        }
        Ok(written)
    }
}
/// Non-Clone cursor owns an Arc to the exact captured result. It neither
/// reexecutes reads nor allocates chunks; callers provide bounded byte storage.
pub struct ManifestReplay {
    owner: Arc<CapturedManifest>,
    position: Position,
}
impl ManifestReplay {
    pub fn copy(&mut self, target: &mut [u8]) -> Result<usize, Failure> {
        self.position.copy(&self.owner, target)
    }
    pub fn done(&self) -> bool {
        self.position.done()
    }
}
impl CapturedManifest {
    pub fn replay(self: Arc<Self>) -> ManifestReplay {
        ManifestReplay {
            owner: self,
            position: Position::new(),
        }
    }
}
