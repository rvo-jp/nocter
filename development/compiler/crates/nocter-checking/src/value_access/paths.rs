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
impl Referent {
    pub(super) fn projection_path<'a, I: Copy>(
        mut self,
        place: impl Fn(AccessPlaceId) -> Option<&'a super::Place>,
        expression: impl Fn(AccessExpressionId) -> Option<&'a Expression>,
        constant: impl Fn(u64) -> I,
        index: impl Fn(AccessExpressionId, AccessExpressionId) -> Option<I>,
    ) -> Option<(AccessLocalId, ValuePath<I>)> {
        let mut suffixes: Vec<ValuePath<I>> = Vec::new();
        loop {
            match self {
                Referent::Place(id) => {
                    let place = place(id)?;
                    let mut fields = known_by(&place.fields, &constant).fields().to_vec();
                    for suffix in suffixes.into_iter().rev() {
                        fields.extend_from_slice(suffix.fields());
                    }
                    return Some((place.local, ValuePath::new(fields)));
                }
                Referent::Evaluated(id) => {
                    let source = match &expression(id)?.kind {
                        ExpressionKind::Index {
                            base,
                            index: operand,
                            action: IndexAction::Access,
                            ..
                        } => {
                            suffixes.push(ValuePath::new([ValueProjection::Array(index(
                                id, *operand,
                            )?)]));
                            *base
                        }
                        ExpressionKind::Select {
                            source,
                            fields,
                            action: SelectionAction::Access,
                        } => {
                            suffixes.push(known_by(fields, &constant));
                            *source
                        }
                        _ => return None,
                    };
                    self = expression(source)?.referent(source)?;
                }
            }
        }
    }
}
