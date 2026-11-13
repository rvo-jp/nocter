//! Canonical component contexts; source types and access proofs remain separate.
use super::{Context, DataInstanceKey};
use crate::value_access::{
    CodeBindingError, ConcreteWitness, FunctionWitness, PayloadSchema, Program,
};
use nocter_model::{BuiltinType, TypeId, ValueField, ValueFunctionOrigin};
use std::{cmp::Ordering, collections::BTreeSet};
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
        let actual = self.adapt_witness(actual, wanted.invocation)?;
        match wanted.origin {
            ValueFunctionOrigin::Parameter(parameter) => {
                let mut contract = self
                    .parameter_contract(parameter)
                    .ok_or(CodeBindingError::InvalidWitness)?
                    .clone();
                contract.invocation = wanted.invocation;
                let supplied = self
                    .function_data(actual.witness())
                    .ok_or(CodeBindingError::InvalidWitness)?;
                self.constrain_function(supplied, &contract)
                    .map_err(CodeBindingError::Contract)?;
            }
            ValueFunctionOrigin::Closure(_) | ValueFunctionOrigin::Declaration(_)
                if actual.witness() == wanted => {}
            _ => return Err(CodeBindingError::InvalidWitness),
        }
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
    fn instance_fields(&self, data: TypeId) -> Result<Vec<(ValueField, TypeId)>, CodeBindingError> {
        Ok(
            match self
                .data_schema(data)
                .ok_or(CodeBindingError::InvalidWitness)?
            {
                PayloadSchema::Aggregate(schema) => {
                    schema.components().map(|c| (c.field, c.data)).collect()
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
        if !matches!(
            self.data_schema(data),
            Some(
                PayloadSchema::Aggregate(_)
                    | PayloadSchema::Array(_)
                    | PayloadSchema::Optional(_)
                    | PayloadSchema::Fallible(_)
            )
        ) {
            return Err(CodeBindingError::InvalidWitness);
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
        resolve: impl Fn(FunctionWitness) -> Result<ConcreteWitness, CodeBindingError>,
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
                let actual = resolve(witness)?;
                completed.insert(data, self.instantiate_function(data, &actual)?);
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
            let key = if matches!(schema, PayloadSchema::Builtin(_)) {
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

enum Pending<'a> {
    Key(&'a DataInstanceKey, &'a DataInstanceKey),
    Children(
        std::collections::btree_map::Iter<'a, ValueField, DataInstanceKey>,
        std::collections::btree_map::Iter<'a, ValueField, DataInstanceKey>,
    ),
}
impl Ord for DataInstanceKey {
    fn cmp(&self, other: &Self) -> Ordering {
        let mut pending = vec![Pending::Key(self, other)];
        let mut visited = BTreeSet::new();
        while let Some(next) = pending.pop() {
            let order =
                match next {
                    Pending::Key(left, right) => {
                        let order = left.data.cmp(&right.data);
                        if !order.is_eq() {
                            return order;
                        }
                        let pair = (Arc::as_ptr(&left.context), Arc::as_ptr(&right.context));
                        if pair.0 == pair.1 || !visited.insert(pair) {
                            continue;
                        }
                        match (&*left.context, &*right.context) {
                            (Context::Closed, Context::Closed) => Ordering::Equal,
                            (Context::Closed, _)
                            | (Context::Function(_), Context::Components(_)) => Ordering::Less,
                            (_, Context::Closed)
                            | (Context::Components(_), Context::Function(_)) => Ordering::Greater,
                            (Context::Function(left), Context::Function(right)) => left.cmp(right),
                            (Context::Components(left), Context::Components(right)) => {
                                pending.push(Pending::Children(left.iter(), right.iter()));
                                Ordering::Equal
                            }
                        }
                    }
                    Pending::Children(mut left, mut right) => match (left.next(), right.next()) {
                        (Some((lk, lv)), Some((rk, rv))) => {
                            let order = lk.cmp(rk);
                            if !order.is_eq() {
                                return order;
                            }
                            pending.push(Pending::Children(left, right));
                            pending.push(Pending::Key(lv, rv));
                            Ordering::Equal
                        }
                        (None, None) => Ordering::Equal,
                        (None, Some(_)) => Ordering::Less,
                        (Some(_), None) => Ordering::Greater,
                    },
                };
            if !order.is_eq() {
                return order;
            }
        }
        Ordering::Equal
    }
}
impl PartialOrd for DataInstanceKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl PartialEq for DataInstanceKey {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}
impl Eq for DataInstanceKey {}
