//! Budgeted metadata-domain joins retain the source descriptor and complete positive identities.
use super::{Context, DataInstanceKey, InstanceSchema, Program};
use crate::value_access::{CodeBindingError, ConcreteWitness, PayloadSchema};
use nocter_model::TypeId;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataDomainJoinFailure<E> {
    Binding(CodeBindingError),
    Work(E),
}
impl<E> From<CodeBindingError> for DataDomainJoinFailure<E> {
    fn from(error: CodeBindingError) -> Self {
        Self::Binding(error)
    }
}
type Node = (TypeId, *const Context, *const Context);
fn node(left: &DataInstanceKey, right: &DataInstanceKey) -> Node {
    (
        left.data(),
        if left.is_context_free() {
            std::ptr::null()
        } else {
            Arc::as_ptr(&left.context)
        },
        if right.is_context_free() {
            std::ptr::null()
        } else {
            Arc::as_ptr(&right.context)
        },
    )
}

impl Program {
    /// Join already validated representation domains of the same source descriptor.
    /// The result is metadata only, with no source receipt, selector, transport or invocation.
    /// Shared schema pairs are visited once; arrays retain one element descriptor.
    /// The caller meters graph/alternative work through `step`. Failure publishes no partial key.
    /// # Errors
    /// Rejects incompatible descriptors, missing schemas, cycles and exhausted caller work.
    pub fn join_data_domains<E>(
        &self,
        left: &DataInstanceKey,
        right: &DataInstanceKey,
        mut step: impl FnMut() -> Result<(), E>,
    ) -> Result<DataInstanceKey, DataDomainJoinFailure<E>> {
        let invalid = || DataDomainJoinFailure::Binding(CodeBindingError::InvalidWitness);
        let root = node(left, right);
        let mut pending = vec![(left.clone(), right.clone(), false)];
        let mut active = BTreeSet::new();
        let mut complete = BTreeMap::new();
        while let Some((left, right, expanded)) = pending.pop() {
            step().map_err(DataDomainJoinFailure::Work)?;
            if left.data() != right.data() {
                return Err(invalid());
            }
            let key = node(&left, &right);
            if complete.contains_key(&key) {
                continue;
            }
            let a = left.schema(self)?;
            let b = right.schema(self)?;
            let ready = if left == right || right.is_uninhabited() {
                Some(left.clone())
            } else if left.is_uninhabited() {
                Some(right.clone())
            } else if matches!(
                self.data_schema(left.data()),
                Some(PayloadSchema::Function(_))
            ) {
                let mut choices = BTreeSet::new();
                for schema in [a, b] {
                    let alternatives: &[ConcreteWitness] = match &schema {
                        InstanceSchema::Function(witness) => std::slice::from_ref(witness),
                        InstanceSchema::FunctionAlternatives(choices) => choices,
                        _ => return Err(invalid()),
                    };
                    for actual in alternatives {
                        step().map_err(DataDomainJoinFailure::Work)?;
                        let actual = self.instantiate_function_context(left.data(), actual)?;
                        let InstanceSchema::Function(actual) = actual.schema(self)? else {
                            return Err(invalid());
                        };
                        choices.insert(actual);
                    }
                }
                Some(self.function_alternative_context(left.data(), choices)?)
            } else {
                None
            };
            if let Some(ready) = ready {
                complete.insert(key, ready);
                continue;
            }
            let fields = self.instance_fields(left.data())?;
            if fields.is_empty() {
                return Err(invalid());
            }
            if !expanded {
                if !active.insert(key) {
                    return Err(invalid());
                }
                pending.push((left.clone(), right.clone(), true));
                for (field, _) in fields.into_iter().rev() {
                    step().map_err(DataDomainJoinFailure::Work)?;
                    pending.push((
                        left.component(self, field)?,
                        right.component(self, field)?,
                        false,
                    ));
                }
                continue;
            }
            active.remove(&key);
            let mut children = Vec::new();
            for (field, _) in fields {
                step().map_err(DataDomainJoinFailure::Work)?;
                let a = left.component(self, field)?;
                let b = right.component(self, field)?;
                children.push((
                    field,
                    complete.get(&node(&a, &b)).ok_or_else(invalid)?.clone(),
                ));
            }
            complete.insert(key, self.instantiate_components(left.data(), children)?);
        }
        complete.remove(&root).ok_or_else(invalid)
    }
}
