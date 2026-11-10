//! Body-local required roots, paired through one full-body source guard rather than header slots.
use super::{BodyRoot, BodySource, Context, SourceMismatch};
use crate::access_declarations::value_type_syntax;
use nocter_model::{AccessCodeId, ValueExpression};
use nocter_syntax::{
    BodySyntaxLocator, BodySyntaxProjection, BoundSyntax, NodeId, NodeKind, SyntaxElement,
    SyntaxOrigin,
};
use std::collections::BTreeMap;

/// Exact-current catalog of required expressions in one complete source body.
/// The reverse index is derived from the same ordered roots; it is not a name namespace.
#[derive(Debug)]
pub struct BodyExpressions<'source> {
    syntax: BoundSyntax<'source>,
    source: BodySource,
    roots: Box<[(ValueExpression, BodySyntaxLocator)]>,
    nodes: BTreeMap<NodeId, ValueExpression>,
}
impl BodyExpressions<'_> {
    #[must_use]
    pub const fn syntax(&self) -> BoundSyntax<'_> {
        self.syntax
    }
    #[must_use]
    pub fn ids(&self) -> impl ExactSizeIterator<Item = ValueExpression> + '_ {
        self.roots.iter().map(|&(id, _)| id)
    }
    #[must_use]
    pub fn expression(&self, root: NodeId) -> Option<ValueExpression> {
        self.nodes.get(&root).copied()
    }
    /// Select an established code/source pairing; a caller cannot substitute another root.
    #[must_use]
    pub fn source(&self, id: ValueExpression) -> Option<BodySource> {
        let &(selected, locator) = self.roots.get(usize::try_from(id.slot).ok()?)?;
        (id == selected).then(|| BodySource {
            owner: id.owner,
            root: BodyRoot::Expression {
                slot: id.slot,
                locator,
            },
            evidence: self.source.evidence.clone(),
        })
    }
}

impl<'source> Context<'source> {
    /// Register array annotation/repetition count roots in this body without reserving declarations.
    /// # Errors
    /// Rejects an unknown/inconsistent body, unsupported type syntax or an exhausted local ID domain.
    pub fn body_expressions(
        &self,
        owner: AccessCodeId,
    ) -> Result<BodyExpressions<'source>, SourceMismatch> {
        let source = self.body_source(owner)?;
        let root = source.bind(self)?.body();
        let syntax = self.syntax();
        let projection = BodySyntaxProjection::for_root(syntax.tree(), root)
            .ok_or(SourceMismatch::InconsistentSyntax)?;
        let mut roots = Vec::new();
        let mut nodes = BTreeMap::new();
        let mut pending = vec![(root, false)];
        while let Some((node, required)) = pending.pop() {
            if required {
                let code = ValueExpression {
                    owner,
                    slot: u32::try_from(roots.len())
                        .map_err(|_| SourceMismatch::InconsistentSyntax)?,
                };
                let locator = projection
                    .locator(SyntaxOrigin::Node(node))
                    .ok_or(SourceMismatch::InconsistentSyntax)?;
                if nodes.insert(node, code).is_some() {
                    return Err(SourceMismatch::InconsistentSyntax);
                }
                roots.push((code, locator));
            }
            if syntax
                .tree()
                .node(node)
                .ok_or(SourceMismatch::InconsistentSyntax)?
                .kind()
                == NodeKind::Type
            {
                let ty = value_type_syntax(syntax, node)
                    .map_err(|_| SourceMismatch::InconsistentSyntax)?;
                let mut lengths = Vec::new();
                ty.length_expressions(&mut lengths);
                // The shared type projection already visited nested data/callable types. Only
                // the count bodies can contain additional local annotations of their own.
                pending.extend(lengths.into_iter().rev().map(|length| (length, true)));
            } else if let Some((seed, count)) =
                nocter_syntax::array_repetition_parts(syntax.tree(), node)
            {
                pending.push((count, true));
                pending.push((seed, false));
            } else {
                pending.extend(
                    syntax
                        .tree()
                        .children(node)
                        .iter()
                        .rev()
                        .filter_map(|child| match child {
                            SyntaxElement::Node(node) => Some((*node, false)),
                            _ => None,
                        }),
                );
            }
        }
        Ok(BodyExpressions {
            syntax,
            source,
            roots: roots.into(),
            nodes,
        })
    }
}
