//! Closed data identity, source-name resolution and payload projection. No source syntax lives here.
use super::{PayloadSchema, Program, Record, records};
use nocter_model::{
    BuiltinType, FieldPath, NominalTypeId, Symbol, SymbolTable, TypeAuthority, TypeId, TypeKind,
};
use std::collections::BTreeMap;

/// A construction-time type reference, resolved once before metadata is published.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DataReference {
    Builtin(BuiltinType),
    Record(NominalTypeId),
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
        match data_reference(&self.symbols, &self.records.namespace, spelling)? {
            DataReference::Builtin(data) => self.data_type(data),
            DataReference::Record(id) => self.records.records.get(id).map(Record::data),
        }
    }
    /// Structural Copy evidence for the payload, never permission to duplicate an access.
    #[must_use]
    pub fn data_copyable(&self, data: TypeId) -> Option<bool> {
        self.data_schema(data).and_then(|schema| match schema {
            PayloadSchema::Builtin(data) => builtin_copyable(data),
            PayloadSchema::Record(record) => Some(record.copyable()),
            PayloadSchema::Function(witness) => match witness.origin {
                nocter_model::ValueFunctionOrigin::Declaration(_) => Some(true),
                nocter_model::ValueFunctionOrigin::Closure(id) => {
                    self.data_copyable(self.closure(id)?.environment())
                }
                _ => Some(false),
            },
        })
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
            } if arguments.as_slice().is_empty() => self
                .records
                .records
                .get(*definition)
                .map(PayloadSchema::Record),
            _ => None,
        }
    }
    /// Validates an already selected logical path against its exact record/field owners.
    /// No name lookup, permission inference or physical offset participates in this projection.
    #[must_use]
    pub fn project_data(&self, mut data: TypeId, path: &FieldPath) -> Option<TypeId> {
        self.data_schema(data)?;
        for &field in path.fields() {
            let PayloadSchema::Record(record) = self.data_schema(data)? else {
                return None;
            };
            let field = self.fields().get(field)?;
            if self.records().get(field.owner())?.data() != record.data() {
                return None;
            }
            data = field.data();
        }
        Some(data)
    }
}

pub(super) fn supported(data: BuiltinType) -> bool {
    builtin_copyable(data).is_some()
}

pub(super) fn builtin_copyable(data: BuiltinType) -> Option<bool> {
    match data {
        BuiltinType::Bool | BuiltinType::I32 | BuiltinType::Void => Some(true),
        BuiltinType::Error => Some(false),
        _ => None,
    }
}

pub(super) fn data_reference(
    symbols: &SymbolTable,
    namespace: &BTreeMap<Symbol, NominalTypeId>,
    spelling: &str,
) -> Option<DataReference> {
    if let Some(data) = BuiltinType::from_spelling(spelling) {
        return Some(DataReference::Builtin(data));
    }
    namespace
        .get(&symbols.get(spelling)?)
        .copied()
        .map(DataReference::Record)
}
pub(super) fn resolved_data(
    types: &TypeAuthority,
    records: &records::Schemas,
    data: DataReference,
) -> TypeId {
    match data {
        DataReference::Builtin(data) => types.store().builtin(data),
        DataReference::Record(id) => records
            .records
            .get(id)
            .expect("validated record reference")
            .data(),
    }
}
