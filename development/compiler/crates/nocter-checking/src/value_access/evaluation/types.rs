//! One evaluator shape authority for domain checks, publication and plan projection.
use super::super::flow;
use nocter_constant_evaluation::state::Type;
use nocter_declarations::value_access::{CodeInstanceKey, DataInstanceKey, PayloadSchema, Program};
use nocter_model::{TypeId, ValueField};
use std::collections::{BTreeMap, BTreeSet};
#[cfg(test)]
mod tests;
#[cfg(test)]
pub(in crate::value_access) use tests::received_test_frames;
const INVALID: flow::IntegrityError = flow::IntegrityError::InconsistentTypedBody;
type Result<T> = std::result::Result<T, flow::IntegrityError>;
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum ShapeKey {
    Raw(TypeId),
    Instance(DataInstanceKey),
}
impl ShapeKey {
    fn instantiated(program: &Program, instance: DataInstanceKey) -> Result<Self> {
        let instance = instance
            .function_representation(program)
            .map_err(|_| INVALID)?
            .unwrap_or(instance);
        Ok(if instance.is_context_free() {
            Self::Raw(instance.data())
        } else {
            Self::Instance(instance)
        })
    }
    fn data(&self) -> TypeId {
        match self {
            Self::Raw(data) => *data,
            Self::Instance(instance) => instance.data(),
        }
    }
    fn component(&self, program: &Program, field: ValueField) -> Result<Self> {
        match self {
            Self::Raw(data) => Ok(Self::Raw(
                program.component(*data, field).ok_or(INVALID)?.data,
            )),
            Self::Instance(instance) => Self::instantiated(
                program,
                instance.component(program, field).map_err(|_| INVALID)?,
            ),
        }
    }
    fn environment(&self, program: &Program, code: nocter_model::ValueClosure) -> Result<Self> {
        match self {
            Self::Raw(_) => Ok(Self::Raw(
                program.closure(code).ok_or(INVALID)?.environment(),
            )),
            Self::Instance(instance) => Self::instantiated(
                program,
                instance
                    .environment(program)
                    .map_err(|_| INVALID)?
                    .ok_or(INVALID)?,
            ),
        }
    }
}
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
    ids: BTreeMap<ShapeKey, usize>,
}
impl Types {
    /// Project the shared contextual data instance, preserving carrier and captured identities.
    pub(in crate::value_access) fn get_in(
        &mut self,
        program: &Program,
        instance: &CodeInstanceKey,
        data: TypeId,
    ) -> Result<usize> {
        let data = program
            .instantiate_data(data, instance)
            .map_err(|_| INVALID)?;
        self.get_instance(program, data)
    }
    pub(super) fn get_environment(
        &mut self,
        program: &Program,
        instance: &CodeInstanceKey,
        source: TypeId,
    ) -> Result<usize> {
        let data = program
            .instantiate_environment(instance)
            .map_err(|_| INVALID)?
            .ok_or(INVALID)?;
        if data.data() != source {
            return Err(INVALID);
        }
        self.get_instance(program, data)
    }
    pub(super) fn get_instance(
        &mut self,
        program: &Program,
        data: DataInstanceKey,
    ) -> Result<usize> {
        self.get_key(program, &ShapeKey::instantiated(program, data)?)
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
    /// Classify a received input from its exact entry context. An unrelated environment cannot
    /// supply missing compound-input metadata; direct callable bindings keep their own authority.
    pub(in crate::value_access) fn supports_input(
        &mut self,
        program: &Program,
        instance: &CodeInstanceKey,
        index: usize,
        data: TypeId,
    ) -> Result<bool> {
        if let Some(actual) = instance.input_data(index) {
            if actual.data() != data {
                return Err(INVALID);
            }
            self.supports_instance(program, actual.clone())
        } else if matches!(program.data_schema(data), Some(PayloadSchema::Function(_))) {
            self.supports_in(program, instance, data)
        } else if program.instantiate_closed_data(data).is_ok() {
            self.supports(program, data)
        } else {
            Ok(false)
        }
    }
    pub(super) fn appended(&self, from: usize) -> Vec<Type> {
        self.shapes[from..]
            .iter()
            .map(|shape| shape.ty.clone())
            .collect()
    }
    pub(in crate::value_access) fn supports_instance(
        &mut self,
        program: &Program,
        data: DataInstanceKey,
    ) -> Result<bool> {
        let id = self.get_instance(program, data)?;
        Ok(self.shapes[id].supported)
    }
    pub(super) fn len(&self) -> usize {
        self.shapes.len()
    }
    /// Follow represented components without rebuilding a formal type in the caller's frame.
    pub(super) fn select_path(
        &self,
        mut data: usize,
        fields: &nocter_constant_evaluation::state::ProjectionPath,
    ) -> Result<usize> {
        use nocter_constant_evaluation::state::Projection;
        self.shapes.get(data).ok_or(INVALID)?;
        for field in fields.iter() {
            data = match *field {
                Projection::Field(index) => self
                    .shapes
                    .get(data)
                    .and_then(|shape| shape.ty.component(index))
                    .ok_or(INVALID)?,
                Projection::Index { bound, .. } => self.array_element(data, bound)?,
            };
        }
        Ok(data)
    }
    /// Only the array schema is checked here; the VM still checks the actual saved number.
    pub(super) fn array_element(&self, data: usize, bound: u64) -> Result<usize> {
        let shape = &self.shapes.get(data).ok_or(INVALID)?.ty;
        let Type::Array { length, .. } = shape else {
            return Err(INVALID);
        };
        if bound == 0 || bound > *length {
            return Err(INVALID);
        }
        shape.component(0).ok_or(INVALID)
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
        self.get_key(program, &ShapeKey::Raw(root))
    }
    fn get_key(&mut self, program: &Program, root: &ShapeKey) -> Result<usize> {
        let mut roots = vec![(root.clone(), false)];
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
        self.ids.get(root).copied().ok_or(INVALID)
    }
    fn project(
        &mut self,
        program: &Program,
        root: ShapeKey,
        metadata_only: bool,
        roots: &mut Vec<(ShapeKey, bool)>,
        empty: &mut Vec<(usize, ShapeKey)>,
    ) -> Result<()> {
        let mut pending = vec![(root, false)];
        let mut active = BTreeSet::new();
        while let Some((data, expanded)) = pending.pop() {
            if self.ids.contains_key(&data) {
                continue;
            }
            let Some(schema) = program.data_schema(data.data()) else {
                if !metadata_only || program.record_identity(data.data()).is_none() {
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
                PayloadSchema::Fallible(_) => vec![
                    data.component(program, ValueField::Variant(true))?,
                    data.component(program, ValueField::Variant(false))?,
                ],
                PayloadSchema::Function(witness) => match witness.origin {
                    nocter_model::ValueFunctionOrigin::Closure(code) => {
                        vec![data.environment(program, code)?]
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
                    empty.push((id, ShapeKey::Raw(array.element())));
                    roots.push((ShapeKey::Raw(array.element()), true));
                    continue;
                }
                PayloadSchema::Aggregate(aggregate) => aggregate
                    .components()
                    .map(|component| data.component(program, component.field))
                    .collect::<Result<Vec<_>>>()?,
                PayloadSchema::Array(_) => vec![data.component(program, ValueField::Array(0))?],
                PayloadSchema::Optional(_) => {
                    vec![data.component(program, ValueField::Variant(true))?]
                }
            };
            if !expanded {
                if !active.insert(data.clone()) {
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
        data: ShapeKey,
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
    fn insert(&mut self, data: ShapeKey, ty: Type, supported: bool) -> usize {
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
