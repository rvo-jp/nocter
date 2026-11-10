//! One admitted evaluator shape authority for domain checks, publication and plan projection.
use super::super::flow;
use nocter_constant_evaluation::state::Type;
use nocter_declarations::value_access::{PayloadSchema, Program};
use nocter_model::TypeId;
use std::collections::{BTreeMap, BTreeSet};
const INVALID: flow::IntegrityError = flow::IntegrityError::InconsistentTypedBody;

#[derive(Default)]
pub(in crate::value_access) struct Types {
    pub(super) values: Vec<Type>,
    ids: BTreeMap<TypeId, Option<usize>>,
}
impl Types {
    pub(super) fn get(
        &mut self,
        program: &Program,
        data: TypeId,
    ) -> Result<usize, flow::IntegrityError> {
        self.project(program, data)?.ok_or(INVALID)
    }
    pub(in crate::value_access) fn supports(
        &mut self,
        program: &Program,
        data: TypeId,
    ) -> Result<bool, flow::IntegrityError> {
        Ok(self.project(program, data)?.is_some())
    }
    /// None is an unadmitted domain, not an invalid source declaration. In particular zero-length
    /// containment cycles need a richer evaluator schema projection than this postorder domain.
    fn project(
        &mut self,
        program: &Program,
        root: TypeId,
    ) -> Result<Option<usize>, flow::IntegrityError> {
        let mut pending = vec![(root, false)];
        let mut active = BTreeSet::new();
        while let Some((data, expanded)) = pending.pop() {
            if self.ids.contains_key(&data) {
                continue;
            }
            let schema = program.data_schema(data).ok_or(INVALID)?;
            let children: Vec<_> = match schema {
                PayloadSchema::Builtin(builtin) => {
                    let id = Type::builtin(builtin).map(|ty| self.insert(ty));
                    self.ids.insert(data, id);
                    continue;
                }
                PayloadSchema::Function(_) => {
                    self.ids.insert(data, None);
                    continue;
                }
                PayloadSchema::Aggregate(aggregate) => {
                    aggregate.components().map(|c| c.data).collect()
                }
                PayloadSchema::Array(array) => vec![array.element()],
            };
            if !expanded {
                if !active.insert(data) {
                    self.ids.insert(data, None);
                    continue;
                }
                pending.push((data, true));
                pending.extend(children.into_iter().rev().map(|child| (child, false)));
                continue;
            }
            active.remove(&data);
            let children = children
                .into_iter()
                .map(|child| self.ids.get(&child).copied().ok_or(INVALID))
                .collect::<Result<Option<Vec<_>>, _>>()?;
            let id = if let Some(children) = children {
                let ty = match schema {
                    PayloadSchema::Aggregate(_) => Type::Record(children.into()),
                    PayloadSchema::Array(array) => Type::Array {
                        element: *children.first().ok_or(INVALID)?,
                        length: array.length(),
                    },
                    PayloadSchema::Builtin(_) | PayloadSchema::Function(_) => return Err(INVALID),
                };
                Some(self.insert(ty))
            } else {
                None
            };
            self.ids.insert(data, id);
        }
        self.ids.get(&root).copied().ok_or(INVALID)
    }
    fn insert(&mut self, ty: Type) -> usize {
        let id = self.values.len();
        self.values.push(ty);
        id
    }
}
