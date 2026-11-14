//! Native materialization of the declaration-owned contextual data graph.
use super::{Failure, Prepared};
use nocter_declarations::value_access::{DataInstanceKey, InstanceSchema, Program};
use nocter_model::{BuiltinType, ValueField};
use nocter_runtime_contract::{
    RuntimeAbiIdentity, RuntimeDataId, RuntimeEnvironment, RuntimePrimitive, RuntimeType,
    RuntimeTypeRepresentation, RuntimeTypeRepresentationTable, RuntimeTypeTableBuilder,
    RuntimeValueFieldRepresentation,
};
use std::collections::BTreeMap;
#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum StructuralShape {
    Tuple(Box<[RuntimeDataId]>),
    Array { element: RuntimeDataId, length: u64 },
    Optional(RuntimeDataId),
    Fallible(RuntimeDataId),
}

pub(super) struct Builder<'a> {
    program: &'a Program,
    ids: BTreeMap<DataInstanceKey, RuntimeDataId>,
    pending: Vec<(RuntimeDataId, DataInstanceKey)>,
    next: u32,
    types: RuntimeTypeTableBuilder,
    representations: BTreeMap<RuntimeDataId, RuntimeTypeRepresentation>,
    structures: BTreeMap<StructuralShape, RuntimeDataId>,
}
impl<'a> Builder<'a> {
    pub(super) fn new(program: &'a Program) -> Self {
        Self {
            program,
            ids: BTreeMap::new(),
            pending: Vec::new(),
            next: 0,
            types: RuntimeTypeTableBuilder::new(),
            representations: BTreeMap::new(),
            structures: BTreeMap::new(),
        }
    }
    pub(super) fn intern(&mut self, key: DataInstanceKey) -> Result<RuntimeDataId, Failure> {
        let root = self.normalized(key)?;
        let mut pending = vec![(root.clone(), false)];
        while let Some((key, expanded)) = pending.pop() {
            if self.ids.contains_key(&key) {
                continue;
            }
            let children = self.structural_children(&key)?;
            if let Some(children) = &children
                && !expanded
            {
                pending.push((key, true));
                pending.extend(children.iter().rev().cloned().map(|key| (key, false)));
                continue;
            }
            let structure = children
                .as_deref()
                .map(|children| self.structural_identity(&key, children))
                .transpose()?;
            if let Some(id) = structure
                .as_ref()
                .and_then(|shape| self.structures.get(shape))
                .copied()
            {
                self.ids.insert(key, id);
                continue;
            }
            let id = self.reserve(key)?;
            if let Some(structure) = structure {
                self.structures.insert(structure, id);
            }
        }
        self.ids
            .get(&root)
            .copied()
            .ok_or(Failure::InvalidPayload(root.data()))
    }
    fn normalized(&self, key: DataInstanceKey) -> Result<DataInstanceKey, Failure> {
        let source = key.data();
        let invalid = || Failure::InvalidPayload(source);
        Ok(key
            .function_representation(self.program)
            .map_err(|_| invalid())?
            .unwrap_or(key))
    }
    fn reserve(&mut self, key: DataInstanceKey) -> Result<RuntimeDataId, Failure> {
        let invalid = || Failure::InvalidPayload(key.data());
        let id = if key.is_context_free() {
            RuntimeDataId::Source(key.data())
        } else {
            let id = self.next;
            self.next = self.next.checked_add(1).ok_or_else(invalid)?;
            RuntimeDataId::Instance(id)
        };
        self.ids.insert(key.clone(), id);
        self.pending.push((id, key));
        Ok(id)
    }
    fn structural_children(
        &self,
        key: &DataInstanceKey,
    ) -> Result<Option<Vec<DataInstanceKey>>, Failure> {
        let invalid = || Failure::InvalidPayload(key.data());
        let fields = match key.schema(self.program).map_err(|_| invalid())? {
            InstanceSchema::Aggregate(schema)
                if matches!(
                    schema.form(),
                    nocter_declarations::value_access::AggregateForm::Tuple(_)
                ) =>
            {
                schema.components().map(|c| c.field).collect()
            }
            InstanceSchema::Array(array) if !array.is_empty() => vec![ValueField::Array(0)],
            InstanceSchema::Optional(_) | InstanceSchema::Fallible(_) => {
                vec![ValueField::Variant(true)]
            }
            _ => return Ok(None),
        };
        fields
            .into_iter()
            .map(|field| {
                self.normalized(key.component(self.program, field).map_err(|_| invalid())?)
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some)
    }
    fn structural_identity(
        &self,
        key: &DataInstanceKey,
        children: &[DataInstanceKey],
    ) -> Result<StructuralShape, Failure> {
        let invalid = || Failure::InvalidPayload(key.data());
        let children = children
            .iter()
            .map(|key| self.ids.get(key).copied().ok_or_else(invalid))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(match key.schema(self.program).map_err(|_| invalid())? {
            InstanceSchema::Aggregate(_) => StructuralShape::Tuple(children.into()),
            InstanceSchema::Array(array) => StructuralShape::Array {
                element: *children.first().ok_or_else(invalid)?,
                length: array.length(),
            },
            InstanceSchema::Optional(_) => {
                StructuralShape::Optional(*children.first().ok_or_else(invalid)?)
            }
            InstanceSchema::Fallible(_) => {
                StructuralShape::Fallible(*children.first().ok_or_else(invalid)?)
            }
            _ => return Err(invalid()),
        })
    }
    fn component(
        &mut self,
        key: &DataInstanceKey,
        field: ValueField,
    ) -> Result<RuntimeDataId, Failure> {
        let child = key
            .component(self.program, field)
            .map_err(|_| Failure::InvalidPayload(key.data()))?;
        self.intern(child)
    }
    fn builtin(
        &mut self,
        key: &DataInstanceKey,
        builtin: BuiltinType,
    ) -> Result<RuntimeDataId, Failure> {
        let data = self
            .program
            .data_type(builtin)
            .ok_or(Failure::InvalidPayload(key.data()))?;
        let closed = self
            .program
            .instantiate_closed_data(data)
            .map_err(|_| Failure::InvalidPayload(data))?;
        self.intern(closed)
    }
    pub(super) fn finish(mut self, abi: RuntimeAbiIdentity) -> Result<Prepared, Failure> {
        while let Some((id, key)) = self.pending.pop() {
            let kind = self.shape(id, &key)?;
            self.types.insert(id, kind).map_err(Failure::Types)?;
        }
        let runtime = RuntimeEnvironment::new(
            self.types.finish().map_err(Failure::Types)?,
            RuntimeTypeRepresentationTable::new(self.representations),
            abi,
        )
        .map_err(Failure::Environment)?;
        Ok(Prepared {
            runtime,
            ids: self.ids,
        })
    }
    fn shape(&mut self, id: RuntimeDataId, key: &DataInstanceKey) -> Result<RuntimeType, Failure> {
        let invalid = || Failure::InvalidPayload(key.data());
        Ok(match key.schema(self.program).map_err(|_| invalid())? {
            InstanceSchema::Array(array) if array.is_empty() => RuntimeType::EmptyArray {
                // Metadata only: no element binding, payload or geometry is manufactured.
                element: RuntimeDataId::Source(array.element()),
            },
            InstanceSchema::Array(array) => {
                let element = self.component(key, ValueField::Array(0))?;
                for builtin in [BuiltinType::Usize, BuiltinType::Bool] {
                    self.builtin(key, builtin)?;
                }
                RuntimeType::FixedArray {
                    element,
                    length: array.length(),
                }
            }
            InstanceSchema::Optional(_) => {
                let payload = self.component(key, ValueField::Variant(true))?;
                for builtin in [BuiltinType::Bool, BuiltinType::U8] {
                    self.builtin(key, builtin)?;
                }
                RuntimeType::Optional(payload)
            }
            InstanceSchema::Fallible(_) => {
                let payload = self.component(key, ValueField::Variant(true))?;
                for builtin in [BuiltinType::Error, BuiltinType::Bool, BuiltinType::U8] {
                    self.builtin(key, builtin)?;
                }
                RuntimeType::Fallible(payload)
            }
            InstanceSchema::Builtin(builtin) => self.primitive(id, key.data(), builtin)?,
            InstanceSchema::Aggregate(aggregate) => {
                let fields = aggregate
                    .components()
                    .map(|component| {
                        Ok(RuntimeValueFieldRepresentation::new(
                            component.field,
                            self.component(key, component.field)?,
                        ))
                    })
                    .collect::<Result<_, Failure>>()?;
                self.representations
                    .insert(id, RuntimeTypeRepresentation::ValueAggregate { fields });
                RuntimeType::Aggregate
            }
            InstanceSchema::Function(_) => {
                let environment = key
                    .environment(self.program)
                    .map_err(|_| invalid())?
                    .map(|environment| self.intern(environment))
                    .transpose()?;
                RuntimeType::StaticCallable { environment }
            }
        })
    }
    fn primitive(
        &mut self,
        id: RuntimeDataId,
        source: nocter_model::TypeId,
        builtin: BuiltinType,
    ) -> Result<RuntimeType, Failure> {
        Ok(match builtin {
            BuiltinType::Bool | BuiltinType::Error => {
                RuntimeType::Primitive(RuntimePrimitive::for_builtin(builtin))
            }
            _ if nocter_model::BuiltinNumericType::for_builtin(builtin)
                .and_then(nocter_model::BuiltinNumericType::integer_bounds)
                .is_some() =>
            {
                RuntimeType::Primitive(RuntimePrimitive::for_builtin(builtin))
            }
            BuiltinType::Str => {
                self.representations
                    .insert(id, RuntimeTypeRepresentation::ValueText);
                RuntimeType::Primitive(RuntimePrimitive::Text)
            }
            BuiltinType::Void => RuntimeType::Unit,
            _ => return Err(Failure::InvalidPayload(source)),
        })
    }
}
