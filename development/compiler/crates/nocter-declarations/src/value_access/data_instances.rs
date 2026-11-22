//! Contextual shape selection preserves semantic type identity and complete callable witnesses.
use super::{
    AggregateSchema, ArraySchema, CodeBindingError, CodeInstanceKey, ConcreteWitness,
    PayloadSchema, Program,
};
use nocter_model::{BuiltinType, FieldPath, TypeId, ValueCode, ValueField};
use std::{collections::BTreeMap, sync::Arc};
mod alternatives;
mod graph;
mod join;
pub use join::DataDomainJoinFailure;
mod representation;
mod reprojection;

#[derive(Clone)]
pub struct DataInstanceKey {
    pub(super) data: TypeId,
    pub(super) context: Arc<Context>,
}
#[derive(Debug)]
pub(super) enum Context {
    Closed,
    Uninhabited,
    Function(ConcreteWitness),
    FunctionAlternatives(Arc<[ConcreteWitness]>),
    Components(BTreeMap<ValueField, DataInstanceKey>),
}
/// One canonical representation domain; inhabited callable leaves retain complete witnesses.
#[derive(Clone, Debug)]
pub enum InstanceSchema<'a> {
    /// This semantic descriptor has no represented values in the selected specialization.
    Uninhabited,
    Builtin(BuiltinType),
    Aggregate(AggregateSchema<'a>),
    Array(ArraySchema),
    Optional(TypeId),
    Fallible(TypeId),
    Function(ConcreteWitness),
    /// A finite positive domain; no active selector, invocation or storage is supplied.
    FunctionAlternatives(Arc<[ConcreteWitness]>),
}
impl DataInstanceKey {
    #[must_use]
    pub const fn data(&self) -> TypeId {
        self.data
    }
    /// Request-local graph identity. Recreated closed handles share their schema node.
    fn context_identity(&self) -> usize {
        if self.is_context_free() {
            0
        } else {
            Arc::as_ptr(&self.context) as usize
        }
    }
    /// Whether the declared descriptor alone selects this representation domain.
    #[must_use]
    pub fn is_context_free(&self) -> bool {
        matches!(&*self.context, Context::Closed)
    }
    /// An empty represented domain, not missing metadata or a currently uninitialized owner.
    #[must_use]
    pub fn is_uninhabited(&self) -> bool {
        matches!(&*self.context, Context::Uninhabited)
    }
    fn selected_function(
        &self,
        program: &Program,
    ) -> Result<Option<ConcreteWitness>, CodeBindingError> {
        match self.schema(program)? {
            InstanceSchema::Function(selected) => Ok(Some(selected)),
            InstanceSchema::FunctionAlternatives(_) => Err(CodeBindingError::InvalidWitness),
            _ => Ok(None),
        }
    }
    /// Select the actual function leaf's representation key without relabelling its carrier.
    /// The original instance retains its semantic identity; only the selected callable changes.
    /// # Errors
    /// Rejects an unresolved selected value or missing captured context.
    pub fn function_representation(
        &self,
        program: &Program,
    ) -> Result<Option<Self>, CodeBindingError> {
        let Some(selected) = self.selected_function(program)? else {
            return Ok(None);
        };
        Self::concrete_function_representation(program, &selected).map(Some)
    }
    /// Select the frozen shape without interning a new semantic type or granting representation.
    /// # Errors
    /// Rejects a missing schema or unresolved callable dependency.
    pub fn schema<'a>(&self, program: &'a Program) -> Result<InstanceSchema<'a>, CodeBindingError> {
        if self.is_uninhabited() {
            program
                .data_schema(self.data)
                .ok_or(CodeBindingError::InvalidWitness)?;
            return Ok(InstanceSchema::Uninhabited);
        }
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
                PayloadSchema::Future(_) => return Err(CodeBindingError::InvalidWitness),
                PayloadSchema::Function(witness) => match &*self.context {
                    Context::Closed => InstanceSchema::Function(program.concrete_witness(witness)?),
                    Context::Function(selected) => InstanceSchema::Function(selected.clone()),
                    Context::FunctionAlternatives(alternatives) => {
                        InstanceSchema::FunctionAlternatives(alternatives.clone())
                    }
                    Context::Components(_) | Context::Uninhabited => {
                        return Err(CodeBindingError::InvalidWitness);
                    }
                },
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
        if self.is_uninhabited() {
            return Err(CodeBindingError::InvalidWitness);
        }
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
    /// Follow validated component metadata without replacing context by a source type lookup.
    /// This supplies no source access, numeric-index relation or initialization proof.
    /// # Errors
    /// Rejects unknown schemas, invalid fields and nonexistent empty-array elements.
    pub fn project_path(
        &self,
        program: &Program,
        path: &FieldPath,
    ) -> Result<Self, CodeBindingError> {
        self.schema(program)?;
        path.fields()
            .iter()
            .try_fold(self.clone(), |key, &field| key.component(program, field))
    }
    /// Project an actual selected closure's own captured context, independent of invocation inputs.
    /// # Errors
    /// Rejects a missing schema, unresolved producer or incompatible captured dependency.
    pub fn environment(&self, program: &Program) -> Result<Option<Self>, CodeBindingError> {
        let Some(selected) = self.selected_function(program)? else {
            return Ok(None);
        };
        program.bound_environment(selected.code(), selected.environment())
    }
}
impl Program {
    /// Instantiate one structural domain from callable-leaf witnesses or explicit empty domains.
    /// `None` selects an empty domain; unresolved metadata must return an error. The fold retains
    /// each requested semantic identity and validates every supplied witness's code and contract.
    /// This grants no source receipt, ownership, layout or executable admission.
    /// # Errors
    /// Rejects unknown/cyclic schemas, missing dependencies and invalid witnesses.
    pub fn instantiate_callable_domains(
        &self,
        data: TypeId,
        resolve: impl Fn(
            TypeId,
            super::FunctionWitness,
        ) -> Result<Option<ConcreteWitness>, CodeBindingError>,
    ) -> Result<DataInstanceKey, CodeBindingError> {
        self.build_data_instance(data, resolve)
    }
    /// Select the empty represented domain of an existing semantic descriptor.
    /// This is metadata only: it creates no value, concrete callable, owner or target layout.
    /// # Errors
    /// Rejects an unknown or unavailable semantic descriptor.
    pub fn instantiate_uninhabited_data(
        &self,
        data: TypeId,
    ) -> Result<DataInstanceKey, CodeBindingError> {
        self.data_schema(data)
            .ok_or(CodeBindingError::InvalidWitness)?;
        Ok(DataInstanceKey {
            data,
            context: Arc::new(Context::Uninhabited),
        })
    }
    /// Select the complete environment input of one existing code frame.
    /// Source/body guards and ownership remain separate from this metadata projection.
    /// # Errors
    /// Rejects unknown closures or an inconsistent contextual environment schema.
    pub fn instantiate_environment(
        &self,
        frame: &CodeInstanceKey,
    ) -> Result<Option<DataInstanceKey>, CodeBindingError> {
        self.bound_environment(frame.code(), frame.environment())
    }
    fn bound_environment(
        &self,
        code: ValueCode,
        environment: Option<&DataInstanceKey>,
    ) -> Result<Option<DataInstanceKey>, CodeBindingError> {
        let ValueCode::Closure(code) = code else {
            return if environment.is_none() {
                Ok(None)
            } else {
                Err(CodeBindingError::InvalidWitness)
            };
        };
        let data = self
            .closure(code)
            .ok_or(CodeBindingError::UnknownCode)?
            .environment();
        if let Some(environment) = environment {
            if environment.data() != data {
                return Err(CodeBindingError::InvalidWitness);
            }
            return Ok(Some(environment.clone()));
        }
        self.instantiate_closed_data(data).map(Some)
    }
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
