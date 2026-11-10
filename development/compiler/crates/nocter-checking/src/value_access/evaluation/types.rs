//! One evaluator shape authority for domain checks, publication and plan projection.
use super::super::flow;
use nocter_constant_evaluation::state::Type;
use nocter_declarations::value_access::{PayloadSchema, Program};
use nocter_model::TypeId;
use std::collections::{BTreeMap, BTreeSet};
const INVALID: flow::IntegrityError = flow::IntegrityError::InconsistentTypedBody;
type Result<T> = std::result::Result<T, flow::IntegrityError>;
struct Shape {
    ty: Type,
    supported: bool,
}
#[derive(Default)]
pub(in crate::value_access) struct Types {
    shapes: Vec<Shape>,
    ids: BTreeMap<TypeId, usize>,
}
impl Types {
    pub(super) fn finish(self) -> Vec<Type> {
        self.shapes.into_iter().map(|shape| shape.ty).collect()
    }
    pub(in crate::value_access) fn supports(
        &mut self,
        program: &Program,
        data: TypeId,
    ) -> Result<bool> {
        let id = self.get(program, data)?;
        Ok(self.shapes[id].supported)
    }
    pub(super) fn get(&mut self, program: &Program, root: TypeId) -> Result<usize> {
        let mut roots = vec![(root, false)];
        let mut empty = Vec::new();
        while let Some((root, metadata_only)) = roots.pop() {
            self.project(program, root, metadata_only, &mut roots, &mut empty)?;
        }
        // Empty element identity survives, but its representation is not a payload prerequisite.
        // Link only after the current positive-containment walk has finished.
        for (id, element) in empty {
            let Type::Array {
                element: target, ..
            } = &mut self.shapes[id].ty
            else {
                return Err(INVALID);
            };
            *target = *self.ids.get(&element).ok_or(INVALID)?;
        }
        self.ids.get(&root).copied().ok_or(INVALID)
    }
    fn project(
        &mut self,
        program: &Program,
        root: TypeId,
        metadata_only: bool,
        roots: &mut Vec<(TypeId, bool)>,
        empty: &mut Vec<(usize, TypeId)>,
    ) -> Result<()> {
        let mut pending = vec![(root, false)];
        let mut active = BTreeSet::new();
        while let Some((data, expanded)) = pending.pop() {
            if self.ids.contains_key(&data) {
                continue;
            }
            let Some(schema) = program.data_schema(data) else {
                if !metadata_only || program.record_identity(data).is_none() {
                    return Err(INVALID);
                }
                // This identity came from a validated empty-array schema. Its declaration can
                // still be unready during required evaluation: no payload header is demanded by
                // that zero edge. Whole-unit closure independently validates every declaration.
                self.insert(data, Type::Unavailable, false);
                continue;
            };
            let children: Vec<_> = match schema {
                PayloadSchema::Builtin(builtin) => {
                    let ty = Type::builtin(builtin);
                    let supported = ty.is_some();
                    self.insert(data, ty.unwrap_or(Type::Unavailable), supported);
                    continue;
                }
                PayloadSchema::Function(_) => {
                    self.insert(data, Type::Unavailable, false);
                    continue;
                }
                PayloadSchema::Array(array) if array.is_empty() => {
                    // A private self-edge is a valid zero containment shape even before linking.
                    // Nothing observes this draft table until all element identities are linked.
                    let id = self.insert(
                        data,
                        Type::Array {
                            element: self.shapes.len(),
                            length: 0,
                        },
                        true,
                    );
                    empty.push((id, array.element()));
                    roots.push((array.element(), true));
                    continue;
                }
                PayloadSchema::Aggregate(aggregate) => {
                    aggregate.components().map(|c| c.data).collect()
                }
                PayloadSchema::Array(array) => vec![array.element()],
            };
            if !expanded {
                if !active.insert(data) {
                    return Err(INVALID);
                }
                pending.push((data, true));
                pending.extend(children.into_iter().rev().map(|child| (child, false)));
                continue;
            }
            active.remove(&data);
            let children = children
                .into_iter()
                .map(|child| self.ids.get(&child).copied().ok_or(INVALID))
                .collect::<Result<Vec<_>>>()?;
            let supported = children.iter().all(|&id| self.shapes[id].supported);
            let ty = match schema {
                PayloadSchema::Aggregate(_) => Type::Record(children.into()),
                PayloadSchema::Array(array) => Type::Array {
                    element: *children.first().ok_or(INVALID)?,
                    length: array.length(),
                },
                PayloadSchema::Builtin(_) | PayloadSchema::Function(_) => return Err(INVALID),
            };
            self.insert(data, ty, supported);
        }
        Ok(())
    }
    fn insert(&mut self, data: TypeId, ty: Type, supported: bool) -> usize {
        let id = self.shapes.len();
        self.shapes.push(Shape { ty, supported });
        self.ids.insert(data, id);
        id
    }
}
