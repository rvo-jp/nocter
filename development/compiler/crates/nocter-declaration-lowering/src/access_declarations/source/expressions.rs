//! Body-local required roots, paired through one full-body source guard rather than header slots.
use super::{BodyRoot, BodySource, Context, SourceMismatch};
use nocter_model::{AccessCodeId, ValueExpression};
use nocter_syntax::{
    BodySyntaxLocator, BodySyntaxProjection, BoundSyntax, NodeId, NodeKind, SyntaxElement,
    SyntaxOrigin, direct_node,
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
    /// Register required array counts and expression-shaped generic argument roots in this body.
    /// # Errors
    /// Rejects an unknown or inconsistent body, or an exhausted local ID domain.
    pub fn body_expressions(
        &self,
        owner: AccessCodeId,
    ) -> Result<BodyExpressions<'source>, SourceMismatch> {
        let source = self.body_source(owner)?;
        let root = source
            .bind(self)?
            .body()
            .ok_or(SourceMismatch::UnknownSource)?;
        let syntax = self.for_code(owner)?.syntax();
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
            let kind = syntax
                .tree()
                .node(node)
                .ok_or(SourceMismatch::InconsistentSyntax)?
                .kind();
            if kind == NodeKind::FixedArrayType {
                let count = direct_node(syntax.tree(), node, NodeKind::Expression)
                    .ok_or(SourceMismatch::InconsistentSyntax)?;
                let element = direct_node(syntax.tree(), node, NodeKind::Type)
                    .ok_or(SourceMismatch::InconsistentSyntax)?;
                pending.push((count, true));
                pending.push((element, false));
            } else if kind == NodeKind::GenericArgument
                && let Some(expression) = direct_node(syntax.tree(), node, NodeKind::Expression)
            {
                pending.push((expression, true));
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
