//! Exact source evidence shared by declaration and checking consumers, without evaluation.
use super::Context;
use nocter_declarations::value_access::{DeclarationId, HeaderPosition, Program};
use nocter_model::{AccessCodeId, NominalTypeId};
use nocter_syntax::{NodeId, SyntaxOrigin, SyntaxToken};
use std::{collections::BTreeMap, sync::Arc};

mod dependencies;
mod recipes;
mod registration;
pub use dependencies::{BodyDependencies, BodySourceGraph, BodySourceNode};
pub(super) use recipes::Recipes;

#[derive(Clone, Debug)]
pub(super) struct Sources {
    pub codes: Arc<BTreeMap<AccessCodeId, CodeSource>>,
    pub records: Arc<BTreeMap<NominalTypeId, Arc<RecordSource>>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceMismatch {
    UnknownSource,
    ChangedAuthority,
    ChangedSource,
    InconsistentSyntax,
    InconsistentDependencies,
}

#[derive(Clone, Debug)]
pub(super) struct FieldSource {
    pub name: SyntaxToken,
    pub ty: NodeId,
}
#[derive(Clone, Debug)]
pub(super) struct RecordSource {
    pub name: SyntaxToken,
    pub fields: Box<[FieldSource]>,
}

#[derive(Clone, Debug)]
pub struct CodeSource {
    header: Option<CodeHeaderSource>,
    body: NodeId,
}
#[derive(Clone, Debug)]
struct CodeHeaderSource {
    name: SyntaxToken,
    parameters: Box<[SyntaxToken]>,
    parameter_types: Box<[NodeId]>,
    result: NodeId,
}
impl CodeSource {
    pub(super) fn name(&self) -> Option<SyntaxToken> {
        self.header.as_ref().map(|header| header.name)
    }
    /// The authored declaration name or the exact anonymous expression root.
    #[must_use]
    pub fn origin(&self) -> SyntaxOrigin {
        match &self.header {
            Some(header) => SyntaxOrigin::Token(header.name),
            None => SyntaxOrigin::Node(self.body),
        }
    }
    #[must_use]
    pub fn parameters(&self) -> &[SyntaxToken] {
        self.header
            .as_ref()
            .map_or(&[], |header| &header.parameters)
    }
    /// The result annotation, or the expression requiring a usize result for anonymous code.
    #[must_use]
    pub fn result_origin(&self) -> NodeId {
        match &self.header {
            Some(header) => header.result,
            None => self.body,
        }
    }
    #[must_use]
    pub const fn body(&self) -> NodeId {
        self.body
    }
}

/// Exact bytes and header lineage, never a source lookup or semantic authority of their own.
#[derive(Clone, Debug)]
struct Evidence {
    header: Arc<Program>,
    bytes: Arc<[u8]>,
}
impl Evidence {
    fn capture(context: &Context<'_>, bytes: &[u8]) -> Self {
        Self {
            header: context.program_handle(),
            bytes: bytes.into(),
        }
    }
    fn bind<'source, T>(
        &self,
        context: &Context<'_>,
        locate: impl FnOnce() -> Result<(T, &'source [u8]), SourceMismatch>,
    ) -> Result<T, SourceMismatch> {
        if !context.program().preserves_headers(&self.header) {
            return Err(SourceMismatch::ChangedAuthority);
        }
        let (source, bytes) = locate()?;
        if bytes != self.bytes.as_ref() {
            return Err(SourceMismatch::ChangedSource);
        }
        Ok(source)
    }
    fn retain(&mut self, source: &Self) -> Result<(), SourceMismatch> {
        if self.bytes != source.bytes {
            return Err(SourceMismatch::ChangedSource);
        }
        if source.header.preserves_headers(&self.header) {
            self.header = source.header.clone();
        } else if !self.header.preserves_headers(&source.header) {
            return Err(SourceMismatch::ChangedAuthority);
        }
        Ok(())
    }
}

/// A source-neutral body-owner/bytes/authority pairing captured only from a current context.
/// This evidence grants neither a ready contract nor body acceptance.
#[derive(Clone, Debug)]
pub struct BodySource {
    owner: AccessCodeId,
    evidence: Evidence,
}
impl BodySource {
    #[must_use]
    pub const fn owner(&self) -> AccessCodeId {
        self.owner
    }
    /// Locate the same body in an exact descendant of its original declaration authority.
    /// # Errors
    /// Rejects a foreign authority, a changed body or inconsistent source metadata.
    pub fn bind<'context>(
        &self,
        context: &'context Context<'_>,
    ) -> Result<&'context CodeSource, SourceMismatch> {
        self.evidence.bind(context, || body(context, self.owner))
    }
}

/// One declaration-owned type annotation, including an unready record's field annotation.
/// The source root is not executable code and grants no normalized type or value.
#[derive(Clone, Debug)]
pub struct HeaderSource {
    declaration: DeclarationId,
    position: HeaderPosition,
    evidence: Evidence,
}
impl HeaderSource {
    #[must_use]
    pub const fn declaration(&self) -> DeclarationId {
        self.declaration
    }
    #[must_use]
    pub const fn position(&self) -> HeaderPosition {
        self.position
    }
    /// Resolve this exact annotation to its current syntax node without type lookup or parsing.
    /// # Errors
    /// Rejects changed authority/source or a missing/inconsistent current annotation.
    pub fn bind(&self, context: &Context<'_>) -> Result<NodeId, SourceMismatch> {
        self.evidence
            .bind(context, || header(context, self.declaration, self.position))
    }
}

impl Context<'_> {
    /// Capture a body dependency without permitting a caller-supplied owner/bytes join.
    /// # Errors
    /// Rejects an unknown body or an inconsistent current source pairing.
    pub fn body_source(&self, owner: AccessCodeId) -> Result<BodySource, SourceMismatch> {
        let (_, bytes) = body(self, owner)?;
        Ok(BodySource {
            owner,
            evidence: Evidence::capture(self, bytes),
        })
    }
    /// Capture one raw type-annotation root independently of its contract's readiness.
    /// # Errors
    /// Rejects a non-type position, an unknown slot or an inconsistent current source pairing.
    pub fn header_source(
        &self,
        declaration: DeclarationId,
        position: HeaderPosition,
    ) -> Result<HeaderSource, SourceMismatch> {
        let (_, bytes) = header(self, declaration, position)?;
        Ok(HeaderSource {
            declaration,
            position,
            evidence: Evidence::capture(self, bytes),
        })
    }
}

/// A union of exact body dependencies. Conflicting evidence never replaces an existing guard.
#[derive(Clone, Debug, Default)]
pub struct BodySources {
    bodies: BTreeMap<AccessCodeId, BodySource>,
}
impl BodySources {
    /// Retain the strongest compatible header prefix for this source owner.
    /// # Errors
    /// Rejects conflicting bytes or incomparable declaration authorities without mutation.
    pub fn insert(&mut self, source: &BodySource) -> Result<(), SourceMismatch> {
        use std::collections::btree_map::Entry;
        match self.bodies.entry(source.owner) {
            Entry::Vacant(entry) => {
                entry.insert(source.clone());
            }
            Entry::Occupied(mut entry) => {
                entry.get_mut().evidence.retain(&source.evidence)?;
            }
        }
        Ok(())
    }
    /// Verify source dependencies without lookup, checking or evaluation of their contents.
    /// # Errors
    /// Reports the first changed authority/body or inconsistent current source pairing.
    pub fn verify(&self, context: &Context<'_>) -> Result<(), SourceMismatch> {
        for source in self.bodies.values() {
            source.bind(context)?;
        }
        Ok(())
    }
}

fn body<'context, 'source>(
    context: &'context Context<'source>,
    owner: AccessCodeId,
) -> Result<(&'context CodeSource, &'source [u8]), SourceMismatch> {
    let source = context
        .code_source(owner)
        .ok_or(SourceMismatch::UnknownSource)?;
    Ok((source, bytes(context, source.body())?))
}
fn header<'source>(
    context: &Context<'source>,
    declaration: DeclarationId,
    position: HeaderPosition,
) -> Result<(NodeId, &'source [u8]), SourceMismatch> {
    let Some(SyntaxOrigin::Node(node)) = header_origin(
        &context.sources.codes,
        &context.sources.records,
        declaration,
        position,
        true,
    ) else {
        return Err(SourceMismatch::UnknownSource);
    };
    Ok((node, bytes(context, node)?))
}
pub(super) fn header_origin(
    sources: &BTreeMap<AccessCodeId, CodeSource>,
    records: &BTreeMap<NominalTypeId, Arc<RecordSource>>,
    declaration: DeclarationId,
    position: HeaderPosition,
    typed: bool,
) -> Option<SyntaxOrigin> {
    Some(match declaration {
        DeclarationId::Code(id) => {
            let s = sources.get(&id)?.header.as_ref()?;
            match position {
                HeaderPosition::Name => SyntaxOrigin::Token(s.name),
                HeaderPosition::Parameter(i) if typed => {
                    SyntaxOrigin::Node(*s.parameter_types.get(i)?)
                }
                HeaderPosition::Parameter(i) => SyntaxOrigin::Token(*s.parameters.get(i)?),
                HeaderPosition::Result => SyntaxOrigin::Node(s.result),
                HeaderPosition::Field(_) => return None,
            }
        }
        DeclarationId::Record(id) => {
            let s = records.get(&id)?;
            match position {
                HeaderPosition::Name => SyntaxOrigin::Token(s.name),
                HeaderPosition::Field(i) if typed => SyntaxOrigin::Node(s.fields.get(i)?.ty),
                HeaderPosition::Field(i) => SyntaxOrigin::Token(s.fields.get(i)?.name),
                _ => return None,
            }
        }
    })
}

fn bytes<'source>(
    context: &Context<'source>,
    root: NodeId,
) -> Result<&'source [u8], SourceMismatch> {
    let syntax = context.syntax();
    let range = syntax
        .tree()
        .node(root)
        .ok_or(SourceMismatch::InconsistentSyntax)?
        .range();
    Ok(syntax
        .text_at(range)
        .ok_or(SourceMismatch::InconsistentSyntax)?
        .as_bytes())
}

#[cfg(test)]
mod tests;
