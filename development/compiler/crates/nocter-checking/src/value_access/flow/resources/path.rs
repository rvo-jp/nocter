//! Resource indices name concrete numbers or backing for a live checked snapshot, never unknowns.
use super::{Program, TypedQuery};
use crate::value_access::{ExpressionKind, IndexAction};
use nocter_model::{
    AccessExpressionId, FieldPath, IndexRelation, PathRelation, TypeId, ValueField, ValuePath,
    ValueProjection,
};

/// Names reusable backing within one body. Equality is meaningful only while the associated
/// snapshot is owned; another execution of the recipe cannot overwrite a live owner.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RetainedIndex {
    owner: nocter_model::ValueCode,
    selection: AccessExpressionId,
    operand: AccessExpressionId,
    bound: u64,
}
impl RetainedIndex {
    /// Describe backing for a checked selection's normal continuation. This alone constructs
    /// no snapshot or ownership.
    #[must_use]
    pub fn for_selection(typed: &TypedQuery, selection: AccessExpressionId) -> Option<Self> {
        let expression = typed.body().expressions().get(selection)?;
        let ExpressionKind::Index {
            base,
            index,
            bounds,
            action: IndexAction::Access,
            ..
        } = expression.kind
        else {
            return None;
        };
        if !expression.ty.may_complete() || bounds == nocter_model::IndexBoundsCheck::ProvenTrap {
            return None;
        }
        let index_data = typed.body().expressions().get(index)?.ty.normal()?.data();
        if typed.program().data_kind(index_data) != Some(nocter_model::BuiltinType::Usize) {
            return None;
        }
        let base = typed.body().expressions().get(base)?.ty.access()?.data();
        let nocter_declarations::value_access::PayloadSchema::Array(array) =
            typed.program().data_schema(base)?
        else {
            return None;
        };
        (!array.is_empty()).then_some(Self {
            owner: typed.code(),
            selection,
            operand: index,
            bound: array.length(),
        })
    }
    #[must_use]
    pub const fn owner(self) -> nocter_model::ValueCode {
        self.owner
    }
    #[must_use]
    pub const fn selection(self) -> AccessExpressionId {
        self.selection
    }
    #[must_use]
    pub const fn operand(self) -> AccessExpressionId {
        self.operand
    }
    #[must_use]
    pub const fn bound(self) -> u64 {
        self.bound
    }
    /// A normal whole-element selector beneath a concrete existing owning array. This is
    /// structural identity only; it proves neither availability nor permission to transfer.
    pub(in crate::value_access::flow) fn array(self, typed: &TypedQuery) -> Option<super::Place> {
        if self.owner != typed.code() || Self::for_selection(typed, self.selection) != Some(self) {
            return None;
        }
        let ExpressionKind::Index { base, .. } =
            typed.body().expressions().get(self.selection)?.kind
        else {
            return None;
        };
        let base = typed.body().expressions().get(base)?.referent(base)?;
        let (local, fields) = Path::for_referent(typed, base)?;
        if fields.concrete().is_none()
            || typed.body().locals().get(local)?.access().mode()
                != nocter_model::ValueAccessMode::Own
        {
            return None;
        }
        let root = typed.body().locals().get(local)?.data;
        let parent = fields.data(typed.program(), root)?;
        let nocter_declarations::value_access::PayloadSchema::Array(array) =
            typed.program().data_schema(parent)?
        else {
            return None;
        };
        if array.length() != self.bound
            || array.element()
                != typed
                    .body()
                    .expressions()
                    .get(self.selection)?
                    .ty
                    .normal()?
                    .data()
        {
            return None;
        }
        Some(super::Holder::Local(local).place().with_fields(fields))
    }
}
/// Body-local backing key for one concrete array's changing committed prefix endpoint.
/// The canonical origin names storage, never the number to capture on a later transfer.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ArrayFrontier(RetainedIndex);
impl ArrayFrontier {
    pub(in crate::value_access::flow) const fn new(origin: RetainedIndex) -> Self {
        Self(origin)
    }
    #[must_use]
    pub const fn owner(self) -> nocter_model::ValueCode {
        self.0.owner()
    }
    #[must_use]
    pub const fn bound(self) -> u64 {
        self.0.bound()
    }
    #[must_use]
    pub fn array(self, typed: &TypedQuery) -> Option<super::Place> {
        self.0.array(typed)
    }
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Index {
    Constant(u64),
    Retained(RetainedIndex),
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum IndexBacking {
    Selection(RetainedIndex),
    Frontier(ArrayFrontier),
}
impl IndexBacking {
    #[must_use]
    pub const fn holder(self) -> super::Holder {
        match self {
            Self::Selection(index) => super::Holder::IndexSnapshot(index),
            Self::Frontier(frontier) => super::Holder::ArrayFrontier(frontier),
        }
    }
    #[must_use]
    pub const fn bound(self) -> u64 {
        match self {
            Self::Selection(index) => index.bound(),
            Self::Frontier(frontier) => frontier.bound(),
        }
    }
}
#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Path(pub(super) ValuePath<Index>);
impl Path {
    #[must_use]
    pub fn from_fields(fields: &FieldPath) -> Self {
        Self(ValuePath::new(
            fields.fields().iter().map(|field| project(*field)),
        ))
    }
    pub(in crate::value_access::flow) fn for_referent(
        typed: &TypedQuery,
        referent: crate::value_access::Referent,
    ) -> Option<(nocter_model::AccessLocalId, Self)> {
        Self::for_referent_with(typed, referent, |_, operand| {
            Some(typed.body().expressions().get(operand)?.index_fact())
        })
    }
    pub(in crate::value_access::flow) fn for_referent_with(
        typed: &TypedQuery,
        referent: crate::value_access::Referent,
        fact: impl Fn(AccessExpressionId, AccessExpressionId) -> Option<crate::value_access::IndexFact>,
    ) -> Option<(nocter_model::AccessLocalId, Self)> {
        let (local, path) = referent.projection_path(
            |id| typed.body().places().get(id),
            |id| typed.body().expressions().get(id),
            Index::Constant,
            |selection, operand| {
                Some(match fact(selection, operand)? {
                    crate::value_access::IndexFact::Known(value) => Index::Constant(value),
                    crate::value_access::IndexFact::Unknown => {
                        Index::Retained(RetainedIndex::for_selection(typed, selection)?)
                    }
                })
            },
        )?;
        Some((local, Self(path)))
    }
    #[must_use]
    pub fn fields(&self) -> &[ValueProjection<Index>] {
        self.0.fields()
    }
    #[must_use]
    pub fn is_root(&self) -> bool {
        self.0.is_root()
    }
    #[must_use]
    pub fn project(&self, field: ValueField) -> Self {
        Self(self.0.project(project(field)))
    }
    #[must_use]
    pub fn project_retained(&self, index: RetainedIndex) -> Self {
        Self(
            self.0
                .project(ValueProjection::Array(Index::Retained(index))),
        )
    }
    #[must_use]
    pub fn concat_fields(&self, fields: &FieldPath) -> Self {
        Self(self.0.concat(&Self::from_fields(fields).0))
    }
    pub fn retained(&self) -> impl Iterator<Item = RetainedIndex> + '_ {
        self.fields().iter().filter_map(|field| match field {
            ValueProjection::Array(Index::Retained(index)) => Some(*index),
            _ => None,
        })
    }
    #[must_use]
    pub fn concrete(&self) -> Option<FieldPath> {
        self.fields()
            .iter()
            .map(|field| {
                Some(match *field {
                    ValueProjection::Record(id) => ValueField::Record(id),
                    ValueProjection::Tuple(position) => ValueField::Tuple(position),
                    ValueProjection::Variant(primary) => ValueField::Variant(primary),
                    ValueProjection::Array(Index::Constant(index)) => ValueField::Array(index),
                    ValueProjection::Array(Index::Retained(_)) => return None,
                })
            })
            .collect::<Option<Vec<_>>>()
            .map(FieldPath::new)
    }
    #[must_use]
    pub fn data(&self, program: &Program, root: TypeId) -> Option<TypeId> {
        program.project_data_by(root, &self.0, |index, length| match *index {
            Index::Constant(index) => index < length,
            Index::Retained(snapshot) => snapshot.bound <= length,
        })
    }
    fn relation(&self, other: &Self) -> PathRelation {
        self.0
            .relation(&other.0, |left, right| match (left, right) {
                (Index::Constant(a), Index::Constant(b)) if a != b => IndexRelation::Distinct,
                (a, b) if a == b => IndexRelation::Equal,
                _ => IndexRelation::Unknown,
            })
    }
    #[must_use]
    pub fn overlaps(&self, other: &Self) -> bool {
        self.relation(other) != PathRelation::Disjoint
    }
    #[must_use]
    pub fn is_prefix_of(&self, other: &Self) -> bool {
        matches!(
            self.relation(other),
            PathRelation::Equal | PathRelation::Ancestor
        )
    }
}
fn project(field: ValueField) -> ValueProjection<Index> {
    match field {
        ValueField::Record(id) => ValueProjection::Record(id),
        ValueField::Tuple(position) => ValueProjection::Tuple(position),
        ValueField::Variant(primary) => ValueProjection::Variant(primary),
        ValueField::Array(index) => ValueProjection::Array(Index::Constant(index)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_access::{
        accepted::{SourceUnit, Unit},
        tests::{bound, input},
    };
    fn prepare(text: &str) -> Unit {
        let (sources, tree) = input(text);
        SourceUnit::prepare(bound(&sources, &tree))
            .unwrap()
            .into_unit()
    }
    fn body(unit: &Unit) -> &TypedQuery {
        let code = unit
            .program()
            .resolve(unit.program().symbols().get("work").unwrap())
            .unwrap();
        unit.bodies().find(|(id, _)| *id == code).unwrap().1.typed()
    }
    #[test]
    fn separate_snapshot_backings_never_prove_disjoint_numbers() {
        let unit = prepare(
            "func work(values: [i32; 2], index: usize): i32 { values[index] + values[index] }",
        );
        let typed = body(&unit);
        let indices: Vec<_> = typed
            .body()
            .expressions()
            .iter()
            .filter_map(|(id, _)| RetainedIndex::for_selection(typed, id))
            .collect();
        assert_eq!(indices.len(), 2);
        let a = Path::default().project_retained(indices[0]);
        let b = Path::default().project_retained(indices[1]);
        assert!(a.overlaps(&b));
        assert!(!a.is_prefix_of(&b));
        assert!(a.concrete().is_none());
        let array = typed.body().locals().iter().next().unwrap().1.data;
        assert_eq!(
            a.data(typed.program(), array),
            typed.program().data_type(nocter_model::BuiltinType::I32)
        );
        for index in 0..2 {
            assert!(a.overlaps(&Path::default().project(ValueField::Array(index))));
        }
    }
    #[test]
    fn a_snapshot_key_retains_the_actual_body_domain() {
        let unit = prepare(
            "func work(values: [i32; 2], index: usize): i32 { values[index] }\nfunc other(values: [i32; 2], index: usize): i32 { values[index] }",
        );
        let indices: Vec<_> = unit
            .bodies()
            .filter_map(|(_, query)| {
                query
                    .typed()
                    .body()
                    .expressions()
                    .iter()
                    .find_map(|(id, _)| RetainedIndex::for_selection(query.typed(), id))
            })
            .collect();
        assert_eq!(indices.len(), 2);
        assert_eq!(indices[0].selection(), indices[1].selection());
        assert_ne!(indices[0].owner(), indices[1].owner());
        assert_ne!(indices[0], indices[1]);
    }
    #[test]
    fn interrupted_or_computed_selection_provides_no_retained_source_recipe() {
        for text in [
            "func work(values: [void; 0]): void { let _ = values[0] }",
            "func work(): i32 { [20, 42][1] }",
        ] {
            let unit = prepare(text);
            let typed = body(&unit);
            assert!(
                typed
                    .body()
                    .expressions()
                    .iter()
                    .all(|(id, _)| RetainedIndex::for_selection(typed, id).is_none())
            );
        }
    }
}
