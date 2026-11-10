//! Source evidence shared by declaration and checking consumers, without semantic evaluation.
use super::{CallableSource, Context};
use nocter_declarations::value_access::Program;
use nocter_model::AccessCallableId;
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BodySourceFailure {
    UnknownBody,
    ChangedAuthority,
    ChangedBody,
    InconsistentSyntax,
}

/// One source-neutral owner/bytes/authority pairing, captured only from a current context.
/// This evidence grants neither a ready contract nor body acceptance.
#[derive(Clone, Debug)]
pub struct BodySource {
    owner: AccessCallableId,
    header: Arc<Program>,
    bytes: Arc<[u8]>,
}
impl BodySource {
    #[must_use]
    pub const fn owner(&self) -> AccessCallableId {
        self.owner
    }
    /// Locate the same body in an exact descendant of its original declaration authority.
    /// # Errors
    /// Rejects a foreign authority, a changed body or inconsistent source metadata.
    pub fn bind<'context>(
        &self,
        context: &'context Context<'_>,
    ) -> Result<&'context CallableSource, BodySourceFailure> {
        if !context.program().preserves_headers(&self.header) {
            return Err(BodySourceFailure::ChangedAuthority);
        }
        let (source, bytes) = body(context, self.owner)?;
        if bytes != self.bytes.as_ref() {
            return Err(BodySourceFailure::ChangedBody);
        }
        Ok(source)
    }
}

impl Context<'_> {
    /// Capture an exact source dependency without permitting a caller-supplied owner/bytes join.
    /// # Errors
    /// Rejects an unknown body or an inconsistent current source pairing.
    pub fn body_source(&self, owner: AccessCallableId) -> Result<BodySource, BodySourceFailure> {
        let (_, bytes) = body(self, owner)?;
        Ok(BodySource {
            owner,
            header: self.program_handle(),
            bytes: bytes.into(),
        })
    }
}

/// A union of exact body dependencies. Conflicting evidence never replaces an existing guard.
#[derive(Clone, Debug, Default)]
pub struct BodySources {
    bodies: BTreeMap<AccessCallableId, BodySource>,
}
impl BodySources {
    /// Retain the strongest compatible header prefix for this source owner.
    /// # Errors
    /// Rejects conflicting bytes or incomparable declaration authorities without mutation.
    pub fn insert(&mut self, source: &BodySource) -> Result<(), BodySourceFailure> {
        use std::collections::btree_map::Entry;
        match self.bodies.entry(source.owner) {
            Entry::Vacant(entry) => {
                entry.insert(source.clone());
            }
            Entry::Occupied(mut entry) => {
                let previous = entry.get();
                if previous.bytes != source.bytes {
                    return Err(BodySourceFailure::ChangedBody);
                }
                if source.header.preserves_headers(&previous.header) {
                    entry.insert(source.clone());
                } else if !previous.header.preserves_headers(&source.header) {
                    return Err(BodySourceFailure::ChangedAuthority);
                }
            }
        }
        Ok(())
    }
    /// Verify source dependencies without lookup, checking or evaluation of their contents.
    /// # Errors
    /// Reports the first changed authority/body or inconsistent current source pairing.
    pub fn verify(&self, context: &Context<'_>) -> Result<(), BodySourceFailure> {
        for source in self.bodies.values() {
            source.bind(context)?;
        }
        Ok(())
    }
}

fn body<'context, 'source>(
    context: &'context Context<'source>,
    owner: AccessCallableId,
) -> Result<(&'context CallableSource, &'source [u8]), BodySourceFailure> {
    let source = context
        .callable_source(owner)
        .ok_or(BodySourceFailure::UnknownBody)?;
    let syntax = context.syntax();
    let range = syntax
        .tree()
        .node(source.body())
        .ok_or(BodySourceFailure::InconsistentSyntax)?
        .range();
    let bytes = syntax
        .text_at(range)
        .ok_or(BodySourceFailure::InconsistentSyntax)?
        .as_bytes();
    Ok((source, bytes))
}

#[cfg(test)]
mod tests;
