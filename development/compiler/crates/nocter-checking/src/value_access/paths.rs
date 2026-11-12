//! Checking-owned numeric selection facts shared by availability and access analysis.
use super::{Expression, ExpressionKind, IndexAction, Referent, SelectionAction};
use nocter_model::{
    AccessExpressionId, AccessLocalId, AccessPlaceId, FieldPath, IndexRelation, PathRelation,
    ValuePath, ValueProjection,
};

/// Evaluated numeric evidence, never an expression-site or loop-generation identity.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum IndexFact {
    Known(u64),
    Unknown,
}
pub type AccessPath = ValuePath<IndexFact>;

pub(super) fn known(fields: &FieldPath) -> AccessPath {
    known_by(fields, IndexFact::Known)
}
fn known_by<I>(fields: &FieldPath, index: impl Fn(u64) -> I) -> ValuePath<I> {
    ValuePath::new(fields.fields().iter().map(|field| match *field {
        ValueProjection::Record(id) => ValueProjection::Record(id),
        ValueProjection::Tuple(position) => ValueProjection::Tuple(position),
        ValueProjection::Variant(primary) => ValueProjection::Variant(primary),
        ValueProjection::Array(value) => ValueProjection::Array(index(value)),
    }))
}
pub(super) fn concrete(fields: &AccessPath) -> Option<FieldPath> {
    fields
        .fields()
        .iter()
        .map(|field| {
            Some(match *field {
                ValueProjection::Record(id) => ValueProjection::Record(id),
                ValueProjection::Tuple(index) => ValueProjection::Tuple(index),
                ValueProjection::Variant(primary) => ValueProjection::Variant(primary),
                ValueProjection::Array(IndexFact::Known(index)) => ValueProjection::Array(index),
                ValueProjection::Array(IndexFact::Unknown) => return None,
            })
        })
        .collect::<Option<Vec<_>>>()
        .map(FieldPath::new)
}
pub(super) fn relation(left: &AccessPath, right: &AccessPath) -> PathRelation {
    left.relation(right, |left, right| match (left, right) {
        (IndexFact::Known(left), IndexFact::Known(right)) if left == right => IndexRelation::Equal,
        (IndexFact::Known(_), IndexFact::Known(_)) => IndexRelation::Distinct,
        _ => IndexRelation::Unknown,
    })
}
impl Expression {
    pub(super) fn index_fact(&self) -> IndexFact {
        self.normal_integer()
            .and_then(|value| u64::try_from(value).ok())
            .map_or(IndexFact::Unknown, IndexFact::Known)
    }
}
/// Forwarded control already validated its selected place before leaving the producing scope.
/// Provenance queries expand its alternatives; availability queries retain that receipt.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum ProjectionRoots {
    All,
    Unforwarded,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum ProjectionRoot {
    Place(AccessPlaceId),
    Temporary(AccessExpressionId),
}

impl Referent {
    /// Enumerate normal place alternatives through one typed projection traversal. These are
    /// conservative provenance possibilities, not evidence that all branches execute or alias.
    pub(super) fn projections<'a, I: Copy>(
        self,
        expression: impl Fn(AccessExpressionId) -> Option<&'a Expression>,
        block: impl Fn(nocter_model::AccessBlockId) -> Option<&'a super::ir::Block>,
        roots: ProjectionRoots,
        constant: impl Fn(u64) -> I,
        index: impl Fn(AccessExpressionId, AccessExpressionId) -> Option<I>,
    ) -> Option<Vec<(ProjectionRoot, ValuePath<I>)>> {
        let mut pending = vec![(self, Vec::<ValuePath<I>>::new())];
        let mut result = Vec::new();
        while let Some((source, mut suffixes)) = pending.pop() {
            let Referent::Evaluated(id) = source else {
                let Referent::Place(base) = source else {
                    unreachable!()
                };
                let fields = suffixes
                    .into_iter()
                    .rev()
                    .flat_map(|path| path.fields().to_vec());
                result.push((ProjectionRoot::Place(base), ValuePath::new(fields)));
                continue;
            };
            let value = expression(id)?;
            if value.referent(id) != Some(Referent::Evaluated(id)) {
                return None;
            }
            if let ExpressionKind::Boundary {
                source,
                action: super::BoundaryAction::TemporaryPlace,
            } = value.kind
            {
                if roots == ProjectionRoots::All {
                    let fields = suffixes
                        .into_iter()
                        .rev()
                        .flat_map(|path| path.fields().to_vec());
                    result.push((ProjectionRoot::Temporary(source), ValuePath::new(fields)));
                }
                continue;
            }
            if roots == ProjectionRoots::Unforwarded
                && matches!(
                    value.kind,
                    ExpressionKind::Scoped { .. }
                        | ExpressionKind::If { .. }
                        | ExpressionKind::Block { .. }
                )
            {
                continue;
            }
            let mut blocks = Vec::new();
            let source = match &value.kind {
                ExpressionKind::GuardView { source } => {
                    pending.push((*source, suffixes));
                    continue;
                }
                ExpressionKind::Scoped { projected, .. } => Some(*projected),
                ExpressionKind::Block { block } => {
                    blocks.push(*block);
                    None
                }
                ExpressionKind::If {
                    then_block,
                    else_block,
                    ..
                } => {
                    blocks.extend([*then_block, *else_block]);
                    None
                }
                ExpressionKind::Index {
                    base,
                    index: operand,
                    action: IndexAction::Access,
                    ..
                } => {
                    suffixes.push(ValuePath::new([ValueProjection::Array(index(
                        id, *operand,
                    )?)]));
                    Some(*base)
                }
                ExpressionKind::Select {
                    source,
                    fields,
                    action: SelectionAction::Access,
                } => {
                    suffixes.push(known_by(fields, &constant));
                    Some(*source)
                }
                _ => return None,
            };
            if let Some(source) = source {
                pending.push((expression(source)?.referent(source)?, suffixes));
            } else {
                for selected in blocks {
                    let branch = block(selected)?;
                    if branch.ty.may_complete() {
                        let value = branch.result?;
                        pending.push((expression(value)?.referent(value)?, suffixes.clone()));
                    }
                }
            }
        }
        Some(result)
    }
    pub(super) fn projection_paths<'a, I: Copy>(
        self,
        place: impl Fn(AccessPlaceId) -> Option<&'a super::Place>,
        expression: impl Fn(AccessExpressionId) -> Option<&'a Expression>,
        block: impl Fn(nocter_model::AccessBlockId) -> Option<&'a super::ir::Block>,
        roots: ProjectionRoots,
        constant: impl Fn(u64) -> I,
        index: impl Fn(AccessExpressionId, AccessExpressionId) -> Option<I>,
    ) -> Option<Vec<(AccessLocalId, ValuePath<I>)>> {
        self.projections(expression, block, roots, &constant, index)?
            .into_iter()
            .map(|(base, suffix)| {
                let ProjectionRoot::Place(base) = base else {
                    return None;
                };
                let base = place(base)?;
                Some((
                    base.local,
                    known_by(&base.fields, &constant).concat(&suffix),
                ))
            })
            .collect()
    }
    pub(super) fn projection_path<'a, I: Copy + Eq>(
        self,
        place: impl Fn(AccessPlaceId) -> Option<&'a super::Place>,
        expression: impl Fn(AccessExpressionId) -> Option<&'a Expression>,
        block: impl Fn(nocter_model::AccessBlockId) -> Option<&'a super::ir::Block>,
        constant: impl Fn(u64) -> I,
        index: impl Fn(AccessExpressionId, AccessExpressionId) -> Option<I>,
    ) -> Option<(AccessLocalId, ValuePath<I>)> {
        let mut paths = self
            .projection_paths(
                place,
                expression,
                block,
                ProjectionRoots::All,
                constant,
                index,
            )?
            .into_iter();
        let first = paths.next()?;
        paths.all(|path| path == first).then_some(first)
    }
}
