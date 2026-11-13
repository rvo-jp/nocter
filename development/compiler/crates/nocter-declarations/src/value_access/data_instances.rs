//! Contextual shape selection preserves semantic type identity and complete callable witnesses.
use super::{
    AggregateSchema, ArraySchema, CodeBindingError, CodeInstanceKey, ConcreteWitness,
    PayloadSchema, Program,
};
use nocter_model::{BuiltinType, TypeId, ValueCode, ValueField};
use std::{collections::BTreeMap, sync::Arc};
mod graph;

#[derive(Clone)]
pub struct DataInstanceKey {
    pub(super) data: TypeId,
    pub(super) context: Arc<Context>,
}
#[derive(Debug)]
pub(super) enum Context {
    Closed,
    Function(ConcreteWitness),
    Components(BTreeMap<ValueField, DataInstanceKey>),
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
    /// Whether representation selection needs any captured/formal substitution.
    #[must_use]
    pub fn is_context_free(&self) -> bool {
        matches!(&*self.context, Context::Closed)
    }
    /// Select the actual function leaf's representation key without relabelling its carrier.
    /// The original instance retains its semantic identity; only the selected callable changes.
    /// # Errors
    /// Rejects an unresolved selected value or missing captured context.
    pub fn function_representation(
        &self,
        program: &Program,
    ) -> Result<Option<Self>, CodeBindingError> {
        let InstanceSchema::Function(selected) = self.schema(program)? else {
            return Ok(None);
        };
        let data = program
            .function_data(selected.witness())
            .ok_or(CodeBindingError::InvalidWitness)?;
        program.instantiate_function(data, &selected).map(Some)
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
                    InstanceSchema::Function(match &*self.context {
                        Context::Closed => program.concrete_witness(witness)?,
                        Context::Function(selected) => selected.clone(),
                        Context::Components(_) => return Err(CodeBindingError::InvalidWitness),
                    })
                }
            },
        )
    }
    /// Select one declaration-validated component and retain only its dependencies.
    /// This is metadata, not a place-access or numeric-bounds proof.
    /// Empty array element identity does not require a positively instantiated element payload.
    /// # Errors
    /// Rejects missing bindings or pending producer terms in a requested positive child.
    pub fn component(
        &self,
        program: &Program,
        field: ValueField,
    ) -> Result<Self, CodeBindingError> {
        let data = program
            .component(self.data, field)
            .ok_or(CodeBindingError::InvalidWitness)?
            .data;
        let slot = if matches!(field, ValueField::Array(_)) {
            ValueField::Array(0)
        } else {
            field
        };
        if let Context::Components(children) = &*self.context
            && let Some(child) = children.get(&slot)
        {
            return Ok(child.clone());
        }
        program.instantiate_closed_data(data)
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
        selected
            .environment()
            .map_or_else(
                || program.instantiate_closed_data(environment),
                |environment| Ok(environment.clone()),
            )
            .map(Some)
    }
}
impl Program {
    /// Instantiate a context-free descriptor, independently of a containing value or code frame.
    /// This supplies no payload, source admission, ownership or target-layout proof.
    /// # Errors
    /// Rejects unresolved producer terms, formal dependencies and invalid descriptors.
    pub fn instantiate_closed_data(
        &self,
        data: TypeId,
    ) -> Result<DataInstanceKey, CodeBindingError> {
        self.bind_payload_with(data, |witness| self.concrete_witness(witness))
    }
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
