//! Canonical component contexts; source types and access proofs remain separate.
use super::{Context, DataInstanceKey};
use crate::value_access::{
    CodeBindingError, ConcreteWitness, FunctionWitness, PayloadSchema, Program,
};
use nocter_model::{BuiltinType, TypeId, ValueField, ValueFunctionOrigin};
use std::collections::BTreeSet;
use std::{collections::BTreeMap, sync::Arc};

impl Program {
    /// Retain an already resolved value occurrence in its original callable source descriptor.
    /// This validates identity, signature and invocation view, not the occurrence's source receipt.
    /// # Errors
    /// Rejects non-callables, unresolved producer descriptors, incompatible contracts and code
    /// substitution at a concrete source leaf. Captured context is never rebuilt from a frame.
    pub fn instantiate_function(
        &self,
        data: TypeId,
        actual: &ConcreteWitness,
    ) -> Result<DataInstanceKey, CodeBindingError> {
        let Some(PayloadSchema::Function(wanted)) = self.data_schema(data) else {
            return Err(CodeBindingError::InvalidWitness);
        };
        if matches!(
            wanted.origin,
            ValueFunctionOrigin::Result(_) | ValueFunctionOrigin::Application(_)
        ) {
            return Err(CodeBindingError::InvalidWitness);
        }
        self.instantiate_function_context(data, actual)
    }
    /// The caller validates the declaration-owned result family before binding an opaque leaf.
    pub(super) fn instantiate_function_context(
        &self,
        data: TypeId,
        actual: &ConcreteWitness,
    ) -> Result<DataInstanceKey, CodeBindingError> {
        let Some(PayloadSchema::Function(wanted)) = self.data_schema(data) else {
            return Err(CodeBindingError::InvalidWitness);
        };
        let actual = self.adapt_witness(actual, wanted.invocation)?;
        self.constrain_instance_function(data, actual.witness())?;
        let context = if self.concrete_witness(wanted).as_ref() == Ok(&actual) {
            Context::Closed
        } else {
            Context::Function(actual)
        };
        Ok(DataInstanceKey {
            data,
            context: Arc::new(context),
        })
    }
    pub(in super::super) fn constrain_instance_function(
        &self,
        data: TypeId,
        actual: FunctionWitness,
    ) -> Result<(), CodeBindingError> {
        let Some(PayloadSchema::Function(wanted)) = self.data_schema(data) else {
            return Err(CodeBindingError::InvalidWitness);
        };
        if actual.invocation != wanted.invocation {
            return Err(CodeBindingError::InvalidWitness);
        }
        match wanted.origin {
            ValueFunctionOrigin::Contract(_) => {
                let supplied = self
                    .function_data(actual)
                    .ok_or(CodeBindingError::InvalidWitness)?;
                self.constrain_callable_contract(supplied, wanted)
                    .map_err(|failure| CodeBindingError::Contract(failure.mismatch))?;
            }
            ValueFunctionOrigin::Parameter(_)
            | ValueFunctionOrigin::Result(_)
            | ValueFunctionOrigin::Application(_) => {
                let contract = self
                    .function_contract(wanted)
                    .ok_or(CodeBindingError::InvalidWitness)?;
                let supplied = self
                    .function_data(actual)
                    .ok_or(CodeBindingError::InvalidWitness)?;
                self.constrain_declared_function(supplied, &contract)
                    .map_err(|failure| CodeBindingError::Contract(failure.mismatch))?;
            }
            ValueFunctionOrigin::Closure(_) | ValueFunctionOrigin::Declaration(_)
                if actual == wanted => {}
            _ => return Err(CodeBindingError::InvalidWitness),
        }
        Ok(())
    }
    pub(in super::super) fn instance_fields(
        &self,
        data: TypeId,
    ) -> Result<Vec<(ValueField, TypeId)>, CodeBindingError> {
        Ok(
            match self
                .data_schema(data)
                .ok_or(CodeBindingError::InvalidWitness)?
            {
                PayloadSchema::Aggregate(schema) => {
                    schema.components().map(|c| (c.field, c.data)).collect()
                }
                PayloadSchema::Enumeration(schema) => {
                    let mut fields = Vec::new();
                    for &variant in schema.variants() {
                        fields.extend(
                            self.variant_components(data, variant)
                                .ok_or(CodeBindingError::InvalidWitness)?
                                .map(|component| (component.field, component.data)),
                        );
                    }
                    fields
                }
                PayloadSchema::Array(array) if !array.is_empty() => {
                    vec![(ValueField::Array(0), array.element())]
                }
                PayloadSchema::Optional(data) => vec![(ValueField::Variant(true), data)],
                PayloadSchema::Fallible(data) => vec![
                    (ValueField::Variant(true), data),
                    (
                        ValueField::Variant(false),
                        self.data_type(BuiltinType::Error)
                            .ok_or(CodeBindingError::InvalidWitness)?,
                    ),
                ],
                _ => vec![],
            },
        )
    }
    /// Assemble a contextual carrier from its complete declaration-selected component instances.
    /// Arrays supply one homogeneous element schema at `Array(0)`, independently of cardinality.
    /// This validates metadata only; it grants no value, access, layout or source admission proof.
    /// # Errors
    /// Rejects missing, duplicate, extra or mistyped components and non-carrier roots.
    pub fn instantiate_components(
        &self,
        data: TypeId,
        children: impl IntoIterator<Item = (ValueField, DataInstanceKey)>,
    ) -> Result<DataInstanceKey, CodeBindingError> {
        let schema = self
            .data_schema(data)
            .ok_or(CodeBindingError::InvalidWitness)?;
        match schema {
            PayloadSchema::Aggregate(_)
            | PayloadSchema::Array(_)
            | PayloadSchema::Optional(_)
            | PayloadSchema::Fallible(_) => {}
            PayloadSchema::Enumeration(case) if case.has_payload() => {}
            _ => return Err(CodeBindingError::InvalidWitness),
        }
        let expected: BTreeMap<_, _> = self.instance_fields(data)?.into_iter().collect();
        let mut supplied = BTreeMap::new();
        for (field, child) in children {
            if expected.get(&field) != Some(&child.data())
                || supplied.insert(field, child).is_some()
            {
                return Err(CodeBindingError::InvalidWitness);
            }
        }
        if supplied.len() != expected.len() {
            return Err(CodeBindingError::InvalidWitness);
        }
        let uninhabited = match schema {
            PayloadSchema::Optional(_) => false,
            PayloadSchema::Fallible(_) => supplied.values().all(DataInstanceKey::is_uninhabited),
            // This instance describes the common storage of all cases. Specialization that
            // changes or excludes a field needs an independent per-case representation proof.
            PayloadSchema::Enumeration(_) => {
                if !supplied.values().all(DataInstanceKey::is_context_free) {
                    return Err(CodeBindingError::InvalidWitness);
                }
                false
            }
            _ => supplied.values().any(DataInstanceKey::is_uninhabited),
        };
        if uninhabited {
            return self.instantiate_uninhabited_data(data);
        }
        supplied.retain(|_, child| !child.is_context_free());
        Ok(DataInstanceKey {
            data,
            context: Arc::new(if supplied.is_empty() {
                Context::Closed
            } else {
                Context::Components(supplied)
            }),
        })
    }
    pub(in super::super) fn build_data_instance(
        &self,
        root: TypeId,
        resolve: impl Fn(TypeId, FunctionWitness) -> Result<Option<ConcreteWitness>, CodeBindingError>,
    ) -> Result<DataInstanceKey, CodeBindingError> {
        let mut pending = vec![(root, false)];
        let mut active = BTreeSet::new();
        let mut completed = BTreeMap::<TypeId, DataInstanceKey>::new();
        while let Some((data, expanded)) = pending.pop() {
            if completed.contains_key(&data) {
                continue;
            }
            let schema = self
                .data_schema(data)
                .ok_or(CodeBindingError::InvalidWitness)?;
            if let PayloadSchema::Function(witness) = schema {
                let key = match resolve(data, witness)? {
                    Some(actual) => self.instantiate_function(data, &actual)?,
                    None => self.instantiate_uninhabited_data(data)?,
                };
                completed.insert(data, key);
                continue;
            }
            let fields = self.instance_fields(data)?;
            if !expanded {
                if !active.insert(data) {
                    return Err(CodeBindingError::InvalidWitness);
                }
                pending.push((data, true));
                pending.extend(fields.iter().rev().map(|(_, child)| (*child, false)));
                continue;
            }
            active.remove(&data);
            let key = if matches!(schema, PayloadSchema::Builtin(_))
                || matches!(schema, PayloadSchema::Enumeration(schema) if !schema.has_payload())
            {
                DataInstanceKey {
                    data,
                    context: Arc::new(Context::Closed),
                }
            } else {
                self.instantiate_components(
                    data,
                    fields
                        .into_iter()
                        .map(|(field, child)| (field, completed[&child].clone())),
                )?
            };
            completed.insert(data, key);
        }
        completed
            .remove(&root)
            .ok_or(CodeBindingError::InvalidWitness)
    }
}
