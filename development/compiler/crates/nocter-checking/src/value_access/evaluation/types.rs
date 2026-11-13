//! One evaluator shape authority for domain checks, publication and plan projection.
use super::super::flow;
use nocter_constant_evaluation::state::Type;
use nocter_declarations::value_access::{CodeInstanceKey, PayloadSchema, Program};
use nocter_model::TypeId;
use std::collections::{BTreeMap, BTreeSet};
const INVALID: flow::IntegrityError = flow::IntegrityError::InconsistentTypedBody;
type Result<T> = std::result::Result<T, flow::IntegrityError>;
struct Shape {
    ty: Type,
    /// At least one value is representable. Unavailable variant payloads remain uninhabitable.
    supported: bool,
    /// The represented domain is nonempty and contains only final-publication data.
    serializable: bool,
}
#[derive(Default)]
pub(in crate::value_access) struct Types {
    shapes: Vec<Shape>,
    ids: BTreeMap<TypeId, usize>,
}
impl Types {
    /// Select only an already frozen direct callable leaf. Compound payload qualification remains
    /// independent: this does not invent a substituted nominal carrier or another type store.
    pub(in crate::value_access) fn get_in(
        &mut self,
        program: &Program,
        instance: &CodeInstanceKey,
        data: TypeId,
    ) -> Result<usize> {
        let data = match program.data_schema(data).ok_or(INVALID)? {
            PayloadSchema::Function(witness) => program
                .function_data(
                    instance
                        .resolve(program, witness)
                        .map_err(|_| INVALID)?
                        .witness(),
                )
                .ok_or(INVALID)?,
            _ => data,
        };
        self.get(program, data)
    }
    pub(in crate::value_access) fn supports_in(
        &mut self,
        program: &Program,
        instance: &CodeInstanceKey,
        data: TypeId,
    ) -> Result<bool> {
        let id = self.get_in(program, instance, data)?;
        Ok(self.shapes[id].supported)
    }
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
    pub(in crate::value_access) fn supports_final(
        &mut self,
        program: &Program,
        data: TypeId,
    ) -> Result<bool> {
        let id = self.get(program, data)?;
        Ok(self.shapes[id].serializable)
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
                PayloadSchema::Fallible(payload) => vec![
                    payload,
                    program
                        .data_type(nocter_model::BuiltinType::Error)
                        .ok_or(INVALID)?,
                ],
                PayloadSchema::Function(witness) => match witness.origin {
                    nocter_model::ValueFunctionOrigin::Closure(code) => {
                        vec![program.closure(code).ok_or(INVALID)?.environment()]
                    }
                    origin => {
                        let ty = match origin {
                            nocter_model::ValueFunctionOrigin::Declaration(code) => {
                                Type::NamedFunction(code)
                            }
                            _ => Type::Unavailable,
                        };
                        let supported = !matches!(ty, Type::Unavailable);
                        let id = self.insert(data, ty, supported);
                        self.shapes[id].serializable = false;
                        continue;
                    }
                },
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
                PayloadSchema::Optional(payload) => vec![payload],
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
            self.compose(data, schema, children)?;
        }
        Ok(())
    }
    fn compose(
        &mut self,
        data: TypeId,
        schema: PayloadSchema<'_>,
        children: Vec<usize>,
    ) -> Result<()> {
        let variant = matches!(
            schema,
            PayloadSchema::Optional(_) | PayloadSchema::Fallible(_)
        );
        let supported = match schema {
            PayloadSchema::Optional(_) => true,
            PayloadSchema::Fallible(_) => children.iter().any(|&id| self.shapes[id].supported),
            _ => children.iter().all(|&id| self.shapes[id].supported),
        };
        let serializable = supported
            && !matches!(schema, PayloadSchema::Function(_))
            && children
                .iter()
                .filter(|&&id| !variant || self.shapes[id].supported)
                .all(|&id| self.shapes[id].serializable);
        let ty = match schema {
            PayloadSchema::Aggregate(_) => Type::Record(children.into()),
            PayloadSchema::Optional(_) => Type::Variant {
                primary: *children.first().ok_or(INVALID)?,
                alternate: None,
            },
            PayloadSchema::Fallible(_) => Type::Variant {
                primary: *children.first().ok_or(INVALID)?,
                alternate: Some(*children.get(1).ok_or(INVALID)?),
            },
            PayloadSchema::Array(array) => Type::Array {
                element: *children.first().ok_or(INVALID)?,
                length: array.length(),
            },
            PayloadSchema::Function(witness) => {
                let nocter_model::ValueFunctionOrigin::Closure(code) = witness.origin else {
                    return Err(INVALID);
                };
                Type::CapturedFunction {
                    code,
                    environment: *children.first().ok_or(INVALID)?,
                }
            }
            PayloadSchema::Builtin(_) => return Err(INVALID),
        };
        let id = self.insert(data, ty, supported);
        self.shapes[id].serializable = serializable;
        Ok(())
    }
    fn insert(&mut self, data: TypeId, ty: Type, supported: bool) -> usize {
        let id = self.shapes.len();
        self.shapes.push(Shape {
            ty,
            supported,
            serializable: supported,
        });
        self.ids.insert(data, id);
        id
    }
}
