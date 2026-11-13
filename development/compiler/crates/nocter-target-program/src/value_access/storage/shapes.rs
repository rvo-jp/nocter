//! Native materialization of the declaration-owned contextual data graph.
use super::{Failure, Prepared};
use nocter_declarations::value_access::{DataInstanceKey, InstanceSchema, Program};
use nocter_model::BuiltinType;
use nocter_runtime_contract::{
    RuntimeAbiIdentity, RuntimeDataId, RuntimeEnvironment, RuntimePrimitive, RuntimeType,
    RuntimeTypeRepresentation, RuntimeTypeRepresentationTable, RuntimeTypeTableBuilder,
    RuntimeValueFieldRepresentation,
};
use std::collections::BTreeMap;
#[cfg(test)]
mod tests;

pub(super) struct Builder<'a> {
    program: &'a Program,
    ids: BTreeMap<DataInstanceKey, RuntimeDataId>,
    pending: Vec<(RuntimeDataId, DataInstanceKey)>,
    next: u32,
    types: RuntimeTypeTableBuilder,
    representations: BTreeMap<RuntimeDataId, RuntimeTypeRepresentation>,
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
        }
    }
    pub(super) fn intern(&mut self, key: DataInstanceKey) -> Result<RuntimeDataId, Failure> {
        let source = key.data();
        let invalid = || Failure::InvalidPayload(source);
        let key = key
            .function_representation(self.program)
            .map_err(|_| invalid())?
            .unwrap_or(key);
        if let Some(&id) = self.ids.get(&key) {
            return Ok(id);
        }
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
    fn child(
        &mut self,
        key: &DataInstanceKey,
        data: nocter_model::TypeId,
    ) -> Result<RuntimeDataId, Failure> {
        let child = key
            .project(self.program, data)
            .map_err(|_| Failure::InvalidPayload(data))?;
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
        self.child(key, data)
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
                let element = self.child(key, array.element())?;
                for builtin in [BuiltinType::Usize, BuiltinType::Bool] {
                    self.builtin(key, builtin)?;
                }
                RuntimeType::FixedArray {
                    element,
                    length: array.length(),
                }
            }
            InstanceSchema::Optional(payload) => {
                let payload = self.child(key, payload)?;
                for builtin in [BuiltinType::Bool, BuiltinType::U8] {
                    self.builtin(key, builtin)?;
                }
                RuntimeType::Optional(payload)
            }
            InstanceSchema::Fallible(payload) => {
                let payload = self.child(key, payload)?;
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
                            self.child(key, component.data)?,
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
