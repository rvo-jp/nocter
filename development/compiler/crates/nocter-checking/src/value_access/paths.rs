//! Checking-owned numeric selection facts shared by availability and access analysis.
use super::{Expression, ExpressionKind, IndexAction, Referent, SelectionAction, TypedBody};
use nocter_model::{
    AccessLocalId, FieldPath, IndexRelation, PathRelation, ValuePath, ValueProjection,
};

/// Evaluated numeric evidence, never an expression-site or loop-generation identity.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum IndexFact {
    Known(u64),
    Unknown,
}
pub type AccessPath = ValuePath<IndexFact>;

pub(super) fn known(fields: &FieldPath) -> AccessPath {
    AccessPath::new(fields.fields().iter().map(|field| match *field {
        ValueProjection::Record(id) => ValueProjection::Record(id),
        ValueProjection::Tuple(index) => ValueProjection::Tuple(index),
        ValueProjection::Array(index) => ValueProjection::Array(IndexFact::Known(index)),
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
impl TypedBody {
    /// Logical availability path relative to its local slot. Outside referent identity remains
    /// access analysis's responsibility; unknown indices grant no concrete ownership path.
    pub(super) fn referent_path(
        &self,
        mut referent: Referent,
    ) -> Option<(AccessLocalId, AccessPath)> {
        let mut suffixes: Vec<AccessPath> = Vec::new();
        loop {
            match referent {
                Referent::Place(id) => {
                    let place = self.places().get(id)?;
                    let mut fields = known(&place.fields).fields().to_vec();
                    for suffix in suffixes.into_iter().rev() {
                        fields.extend_from_slice(suffix.fields());
                    }
                    return Some((place.local, AccessPath::new(fields)));
                }
                Referent::Evaluated(id) => {
                    let source = match &self.expressions().get(id)?.kind {
                        ExpressionKind::Index {
                            base,
                            index,
                            action: IndexAction::Access,
                            ..
                        } => {
                            suffixes.push(AccessPath::new([ValueProjection::Array(
                                self.expressions().get(*index)?.index_fact(),
                            )]));
                            *base
                        }
                        ExpressionKind::Select {
                            source,
                            fields,
                            action: SelectionAction::Access,
                        } => {
                            suffixes.push(known(fields));
                            *source
                        }
                        _ => return None,
                    };
                    referent = self.expressions().get(source)?.referent(source)?;
                }
            }
        }
    }
}
