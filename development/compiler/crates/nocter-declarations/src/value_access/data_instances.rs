//! Contextual shape selection preserves semantic type identity and complete callable witnesses.
use super::{
    AggregateSchema, ArraySchema, CodeBindingError, CodeInstanceKey, ConcreteWitness,
    FunctionWitness, PayloadSchema, Program,
};
use nocter_model::{BuiltinType, TypeId, ValueCode, ValueParameter};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DataInstanceKey {
    pub(super) data: TypeId,
    pub(super) witnesses: BTreeMap<ValueParameter, ConcreteWitness>,
}
/// One canonical schema with a resolved callable value; carrier children retain semantic `TypeId`s.
#[derive(Clone, Debug)]
pub enum InstanceSchema<'a> {
    Builtin(BuiltinType),
    Aggregate(AggregateSchema<'a>),
    Array(ArraySchema),
    Optional(TypeId),
    Fallible(TypeId),
    Function(ConcreteWitness),
}
impl DataInstanceKey {
    #[must_use]
    pub const fn data(&self) -> TypeId {
        self.data
    }
    /// Select the frozen shape without interning a new semantic type or granting representation.
    /// # Errors
    /// Rejects a missing schema or unresolved callable dependency.
    pub fn schema<'a>(&self, program: &'a Program) -> Result<InstanceSchema<'a>, CodeBindingError> {
        Ok(
            match program
                .data_schema(self.data)
                .ok_or(CodeBindingError::InvalidWitness)?
            {
                PayloadSchema::Builtin(data) => InstanceSchema::Builtin(data),
                PayloadSchema::Aggregate(data) => InstanceSchema::Aggregate(data),
                PayloadSchema::Array(data) => InstanceSchema::Array(data),
                PayloadSchema::Optional(data) => InstanceSchema::Optional(data),
                PayloadSchema::Fallible(data) => InstanceSchema::Fallible(data),
                PayloadSchema::Function(witness) => {
                    InstanceSchema::Function(self.resolve(program, witness)?)
                }
            },
        )
    }
    fn resolve(
        &self,
        program: &Program,
        witness: FunctionWitness,
    ) -> Result<ConcreteWitness, CodeBindingError> {
        program.resolve_bindings(witness, |parameter| self.witnesses.get(&parameter))
    }
    /// Retain only the selected child's dependencies. This is metadata, not a place-access proof.
    /// Empty array element identity does not require a positively instantiated element payload.
    /// # Errors
    /// Rejects missing bindings or pending producer terms in a requested positive child.
    pub fn project(&self, program: &Program, data: TypeId) -> Result<Self, CodeBindingError> {
        program.bind_payload_with(data, |witness| self.resolve(program, witness))
    }
    /// Project an actual selected closure's own captured context, independent of invocation inputs.
    /// # Errors
    /// Rejects a missing schema, unresolved producer or incompatible captured dependency.
    pub fn environment(&self, program: &Program) -> Result<Option<Self>, CodeBindingError> {
        let InstanceSchema::Function(selected) = self.schema(program)? else {
            return Ok(None);
        };
        let ValueCode::Closure(code) = selected.code() else {
            return Ok(None);
        };
        let environment = program
            .closure(code)
            .ok_or(CodeBindingError::UnknownCode)?
            .environment();
        program
            .bind_payload_with(environment, |witness| {
                program.resolve_bindings(witness, |parameter| selected.captures().get(&parameter))
            })
            .map(Some)
    }
}
impl Program {
    /// Bind a canonical payload descriptor through the already selected concrete code frame.
    /// This supplies no source receipt, payload, eligibility, ownership or target-layout proof.
    /// # Errors
    /// Rejects pending producers, missing/foreign bindings and incompatible callable views.
    pub fn instantiate_data(
        &self,
        data: TypeId,
        frame: &CodeInstanceKey,
    ) -> Result<DataInstanceKey, CodeBindingError> {
        self.bind_payload_with(data, |witness| frame.resolve(self, witness))
    }
}
