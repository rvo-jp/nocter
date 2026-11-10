//! Closed data identity, source-name resolution and payload projection. No source syntax lives here.
use super::{AggregateForm, DeclarationId, Program, Record};
use nocter_model::{BuiltinType, FieldPath, NominalTypeId, Symbol, SymbolTable, TypeId, TypeKind};
use std::collections::BTreeMap;

/// The exact closed shapes admitted by this declaration product, not a general type classifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PayloadSchema<'a> {
    Builtin(BuiltinType),
    Aggregate(super::AggregateSchema<'a>),
    Array(ArraySchema),
    Function(super::FunctionWitness),
}

/// A construction-time type reference, resolved once before metadata is published.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataReference {
    Builtin(BuiltinType),
    Record(NominalTypeId),
    Tuple(TupleData),
    Array {
        element: Box<DataReference>,
        length: u64,
    },
}

/// A homogeneous closed array descriptor; cardinality never expands its element schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArraySchema {
    element: TypeId,
    length: u64,
}
impl ArraySchema {
    #[must_use]
    pub const fn element(self) -> TypeId {
        self.element
    }
    #[must_use]
    pub const fn length(self) -> u64 {
        self.length
    }
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.length == 0
    }
}
/// A structural data reference with the same minimum arity as semantic tuple types.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TupleData(Box<[DataReference]>);
impl TupleData {
    #[must_use]
    pub fn new(
        first: DataReference,
        second: DataReference,
        remaining: impl IntoIterator<Item = DataReference>,
    ) -> Self {
        let mut fields = vec![first, second];
        fields.extend(remaining);
        Self(fields.into())
    }
    #[must_use]
    pub fn as_slice(&self) -> &[DataReference] {
        &self.0
    }
}
impl DataReference {
    pub(super) fn visit_leaves(&self, visit: impl FnMut(&Self)) {
        self.visit(false, visit);
    }
    pub(super) fn visit_payload_leaves(&self, visit: impl FnMut(&Self)) {
        self.visit(true, visit);
    }
    fn visit(&self, payload_only: bool, mut visit: impl FnMut(&Self)) {
        let mut pending = vec![self];
        while let Some(data) = pending.pop() {
            match data {
                Self::Tuple(fields) => pending.extend(fields.as_slice().iter().rev()),
                Self::Array { element, length } => {
                    if !payload_only || *length != 0 {
                        pending.push(element);
                    }
                }
                _ => visit(data),
            }
        }
    }
}
impl From<BuiltinType> for DataReference {
    fn from(data: BuiltinType) -> Self {
        Self::Builtin(data)
    }
}

impl Program {
    #[must_use]
    pub fn data_type(&self, data: BuiltinType) -> Option<TypeId> {
        supported(data).then(|| self.types.store().builtin(data))
    }
    #[must_use]
    pub fn data_kind(&self, data: TypeId) -> Option<BuiltinType> {
        match self.types.store().get(data)? {
            TypeKind::Builtin(builtin) if supported(*builtin) => Some(*builtin),
            _ => None,
        }
    }
    /// Source-name lookup; semantic equality continues to use resolved `TypeId` only.
    #[must_use]
    pub fn type_named(&self, spelling: &str) -> Option<TypeId> {
        match data_reference(&self.symbols, &self.namespace, spelling)? {
            DataReference::Builtin(data) => self.data_type(data),
            DataReference::Record(id) => self.records.records.get(id).map(Record::data),
            DataReference::Tuple(_) | DataReference::Array { .. } => None,
        }
    }
    /// Structural Copy evidence for the payload, never permission to duplicate an access.
    #[must_use]
    pub fn data_copyable(&self, data: TypeId) -> Option<bool> {
        // Structural tuples can share child types exponentially. Visit each shape once, just as
        // nominal Copy publication does, instead of expanding the represented payload tree.
        let mut copyable = BTreeMap::new();
        let mut pending = vec![(data, false)];
        while let Some((current, expanded)) = pending.pop() {
            if copyable.contains_key(&current) {
                continue;
            }
            let children = match self.data_schema(current)? {
                PayloadSchema::Builtin(builtin) => {
                    copyable.insert(current, builtin_copyable(builtin)?);
                    continue;
                }
                PayloadSchema::Array(array) => {
                    if array.is_empty() {
                        copyable.insert(current, true);
                        continue;
                    }
                    vec![array.element()]
                }
                PayloadSchema::Aggregate(schema) => match schema.form() {
                    AggregateForm::Record(record) => {
                        copyable.insert(current, record.copyable());
                        continue;
                    }
                    AggregateForm::Tuple(fields) => fields.to_vec(),
                },
                PayloadSchema::Function(witness) => match witness.origin {
                    nocter_model::ValueFunctionOrigin::Declaration(_) => {
                        copyable.insert(current, true);
                        continue;
                    }
                    nocter_model::ValueFunctionOrigin::Closure(id) => {
                        vec![self.closure(id)?.environment()]
                    }
                    _ => {
                        copyable.insert(current, false);
                        continue;
                    }
                },
            };
            if expanded {
                copyable.insert(
                    current,
                    children
                        .iter()
                        .all(|child| copyable.get(child) == Some(&true)),
                );
            } else {
                pending.push((current, true));
                pending.extend(children.into_iter().rev().map(|child| (child, false)));
            }
        }
        copyable.get(&data).copied()
    }

    /// The exact closed payload shape, without borrow types or generic eligibility fallbacks.
    #[must_use]
    pub fn data_schema(&self, data: TypeId) -> Option<PayloadSchema<'_>> {
        match self.types.store().get(data)? {
            TypeKind::Builtin(builtin) if supported(*builtin) => {
                Some(PayloadSchema::Builtin(*builtin))
            }
            TypeKind::ValueFunction { origin, invocation } => {
                let witness = super::FunctionWitness {
                    origin: *origin,
                    invocation: *invocation,
                };
                // Schema/property queries validate identity, not an allocated signature copy.
                self.function_data(witness)
                    .map(|_| PayloadSchema::Function(witness))
            }
            TypeKind::Nominal {
                definition,
                arguments,
            } if arguments.as_slice().is_empty() => {
                self.records.records.get(*definition).map(|record| {
                    PayloadSchema::Aggregate(super::AggregateSchema::record(record, self.fields()))
                })
            }
            TypeKind::FixedArray { element, length } => Some(PayloadSchema::Array(ArraySchema {
                element: *element,
                length: length.closed_value()?,
            })),
            TypeKind::Tuple(fields) => Some(PayloadSchema::Aggregate(
                super::AggregateSchema::tuple(fields.as_slice()),
            )),
            _ => None,
        }
    }
    /// Validate one semantic component against its containing aggregate.
    #[must_use]
    pub fn component(
        &self,
        data: TypeId,
        field: nocter_model::ValueField,
    ) -> Option<super::Component> {
        let PayloadSchema::Aggregate(schema) = self.data_schema(data)? else {
            return None;
        };
        schema.component(field)
    }
    /// Positional encoding of an established logical selection; this grants no projection rights.
    #[must_use]
    pub fn component_position(&self, field: nocter_model::ValueField) -> Option<usize> {
        match field {
            nocter_model::ValueField::Record(id) => Some(self.fields().get(id)?.position()),
            nocter_model::ValueField::Tuple(position) => Some(position),
        }
    }
    /// Validates an already selected logical path against its exact containing aggregate schemas.
    /// No name lookup, permission inference or physical offset participates in this projection.
    #[must_use]
    pub fn project_data(&self, mut data: TypeId, path: &FieldPath) -> Option<TypeId> {
        self.data_schema(data)?;
        for &field in path.fields() {
            data = self.component(data, field)?.data;
        }
        Some(data)
    }
}

pub(super) fn supported(data: BuiltinType) -> bool {
    builtin_copyable(data).is_some()
}

pub(super) fn builtin_copyable(data: BuiltinType) -> Option<bool> {
    match data {
        BuiltinType::Bool | BuiltinType::I32 | BuiltinType::Usize | BuiltinType::Void => Some(true),
        BuiltinType::Error => Some(false),
        _ => None,
    }
}

pub(super) fn data_reference(
    symbols: &SymbolTable,
    namespace: &BTreeMap<Symbol, DeclarationId>,
    spelling: &str,
) -> Option<DataReference> {
    if let Some(data) = BuiltinType::from_spelling(spelling) {
        return Some(DataReference::Builtin(data));
    }
    match namespace.get(&symbols.get(spelling)?)? {
        DeclarationId::Record(id) => Some(DataReference::Record(*id)),
        DeclarationId::Code(_) => None,
    }
}
pub(super) fn resolved_data(
    types: &mut nocter_model::TypeTransaction,
    data: &DataReference,
) -> TypeId {
    let mut pending = vec![(data, false)];
    let mut resolved = Vec::new();
    while let Some((data, expanded)) = pending.pop() {
        let data = match data {
            DataReference::Builtin(data) => types.builtin(*data),
            DataReference::Record(id) => types
                .intern(super::records::nominal_key(*id))
                .expect("validated nominal identity"),
            DataReference::Array { element, length } => {
                if !expanded {
                    pending.push((data, true));
                    pending.push((element, false));
                    continue;
                }
                let element = resolved.pop().expect("resolved array element");
                types
                    .intern(TypeKind::FixedArray {
                        element,
                        length: (*length).into(),
                    })
                    .expect("validated closed array reference")
            }
            DataReference::Tuple(fields) => {
                if !expanded {
                    pending.push((data, true));
                    pending.extend(fields.as_slice().iter().rev().map(|field| (field, false)));
                    continue;
                }
                let fields = resolved.split_off(resolved.len() - fields.as_slice().len());
                types
                    .intern(TypeKind::Tuple(nocter_model::TupleElements::new(
                        fields[0],
                        fields[1],
                        fields[2..].iter().copied(),
                    )))
                    .expect("validated closed tuple reference")
            }
        };
        resolved.push(data);
    }
    resolved.pop().expect("one complete data reference")
}

#[cfg(test)]
mod tests;
