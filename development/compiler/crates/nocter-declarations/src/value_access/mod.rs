//! Declaration authority for the bounded future private core.
//!
//! This product cannot be converted to a released borrow-based declaration program. Its
//! source boundary admits closed data and named/formal function witnesses. It does not flatten the
//! complete future aggregate/callable/outcome grammar into released borrow-based types.
use std::collections::BTreeMap;

use nocter_model::{
    AccessCallableId, AccessParameterId, Arena, ArenaBuilder, BuiltinType, CompileTimeGuarantee,
    NominalTypeId, SparseArena, Symbol, SymbolTable, TypeAuthority, TypeId, ValueInputOperation,
};

use crate::DefinitionError;

mod body_types;
mod closures;
pub use body_types::{BodyTypeError, BodyTypes, CanonicalBodyTypes};
pub use closures::Closure;
mod aggregate;
pub use aggregate::{AggregateSchema, Component};
mod data;
mod functions;
pub use functions::{
    FunctionConstraint, FunctionDefinition, FunctionInput, FunctionMismatch, FunctionWitness,
    ValueDefinition,
};
mod namespace;
mod publication;
pub use publication::{HeaderPublication, HeaderStep};
mod records;
pub use data::{DataReference, PayloadSchema, TupleData};
use data::{data_reference, supported};
pub use records::{Field, FieldDefinition, Record};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DeclarationId {
    Callable(AccessCallableId),
    Record(NominalTypeId),
}
impl From<AccessCallableId> for DeclarationId {
    fn from(id: AccessCallableId) -> Self {
        Self::Callable(id)
    }
}
impl From<NominalTypeId> for DeclarationId {
    fn from(id: NominalTypeId) -> Self {
        Self::Record(id)
    }
}

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParameterDefinition {
    pub name: Symbol,
    pub data: ValueDefinition,
    pub operation: ValueInputOperation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HeaderPosition {
    Name,
    Parameter(usize),
    Result,
    Field(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclarationRule {
    MissingDefinition,
    InvalidInitializerHeader,
    UnknownSymbol,
    ReservedBuiltinName,
    DuplicateCallable { first: AccessCallableId },
    DuplicateParameter { first: usize },
    ParameterShadowsCallable { declaration: AccessCallableId },
    ParameterShadowsRecord { declaration: NominalTypeId },
    UnsupportedDataType { data: BuiltinType },
    DuplicateRecord { first: NominalTypeId },
    RecordShadowsCallable { declaration: AccessCallableId },
    DuplicateField { first: usize },
    UnknownRecordType { record: NominalTypeId },
    UnknownDataName { name: Symbol },
    RecursivePayload { through: NominalTypeId },
    NonCopyField,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeclarationViolation {
    pub declaration: DeclarationId,
    pub position: HeaderPosition,
    pub rule: DeclarationRule,
}

/// Source role of executable code. A constant owns an initialization thunk, never a function witness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallableKind {
    Function(CompileTimeGuarantee),
    ConstantInitializer,
    StaticInitializer,
}
impl CallableKind {
    #[must_use]
    pub const fn is_initializer(self) -> bool {
        matches!(self, Self::ConstantInitializer | Self::StaticInitializer)
    }
    #[must_use]
    pub const fn compile_time(self) -> CompileTimeGuarantee {
        match self {
            Self::Function(capability) => capability,
            Self::ConstantInitializer | Self::StaticInitializer => {
                CompileTimeGuarantee::Evaluatable
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Callable {
    name: Symbol,
    kind: CallableKind,
    parameters: Box<[AccessParameterId]>,
    result: TypeId,
    result_contract: Option<FunctionConstraint>,
}

impl Callable {
    #[must_use]
    pub const fn kind(&self) -> CallableKind {
        self.kind
    }
    /// Authored capability only; body-domain acceptance and evaluation are separate obligations.
    #[must_use]
    pub const fn compile_time(&self) -> CompileTimeGuarantee {
        self.kind.compile_time()
    }
    #[must_use]
    pub const fn result_contract(&self) -> Option<&FunctionConstraint> {
        self.result_contract.as_ref()
    }
    #[must_use]
    pub const fn name(&self) -> Symbol {
        self.name
    }
    #[must_use]
    pub fn parameters(&self) -> &[AccessParameterId] {
        &self.parameters
    }
    #[must_use]
    pub const fn result(&self) -> TypeId {
        self.result
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Parameter {
    owner: AccessCallableId,
    name: Symbol,
    position: usize,
    data: TypeId,
    operation: ValueInputOperation,
    contract: Option<FunctionConstraint>,
}

impl Parameter {
    #[must_use]
    pub const fn contract(&self) -> Option<&FunctionConstraint> {
        self.contract.as_ref()
    }

    #[must_use]
    pub const fn owner(&self) -> AccessCallableId {
        self.owner
    }
    #[must_use]
    pub const fn name(&self) -> Symbol {
        self.name
    }
    #[must_use]
    pub const fn position(&self) -> usize {
        self.position
    }
    #[must_use]
    pub const fn data(&self) -> TypeId {
        self.data
    }
    #[must_use]
    pub const fn operation(&self) -> ValueInputOperation {
        self.operation
    }
}

/// Construction provenance of the type suffix, independent of whether that suffix is empty.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TypeScope {
    Headers,
    Body(AccessCallableId),
    Canonical,
}

/// Immutable declaration metadata; body acceptance and native execution are separate products.
#[derive(Clone, Debug)]
pub struct Program {
    symbols: std::sync::Arc<SymbolTable>,
    types: TypeAuthority,
    header_prefix: nocter_model::TypeStore,
    type_scope: TypeScope,
    callables: SparseArena<AccessCallableId, Callable>,
    parameters: SparseArena<AccessParameterId, Parameter>,
    namespace: std::sync::Arc<BTreeMap<Symbol, DeclarationId>>,
    records: records::Schemas,
    closures: BTreeMap<nocter_model::ValueClosure, Closure>,
}

impl Program {
    #[must_use]
    pub const fn headers_complete(&self) -> bool {
        self.callables.is_complete()
            && self.parameters.is_complete()
            && self.records.records.is_complete()
            && self.records.fields.is_complete()
    }
    /// A complete published contract, including its established transitive record schemas.
    #[must_use]
    pub fn header_ready(&self, declaration: impl Into<DeclarationId>) -> bool {
        match declaration.into() {
            DeclarationId::Callable(id) => self.callables.get(id).is_some(),
            DeclarationId::Record(id) => self.records.records.get(id).is_some(),
        }
    }
    /// Retains every established contract and its exact type prefix from the same construction.
    #[must_use]
    pub fn preserves_headers(&self, older: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.namespace, &older.namespace)
            && self.records.records.preserves(&older.records.records)
            && self.records.fields.preserves(&older.records.fields)
            && self.callables.preserves(&older.callables)
            && self.parameters.preserves(&older.parameters)
            && self.header_prefix.preserves_prefix(&older.header_prefix)
    }
    #[must_use]
    pub fn symbols(&self) -> &SymbolTable {
        &self.symbols
    }
    #[must_use]
    pub fn records(&self) -> &SparseArena<NominalTypeId, Record> {
        &self.records.records
    }
    #[must_use]
    pub fn fields(&self) -> &SparseArena<nocter_model::AccessFieldId, Field> {
        &self.records.fields
    }
    /// Closed name identity, independently of whether its typed contract is ready.
    #[must_use]
    pub fn declaration(&self, name: Symbol) -> Option<DeclarationId> {
        self.namespace.get(&name).copied()
    }
    #[must_use]
    pub fn resolve_record(&self, name: Symbol) -> Option<NominalTypeId> {
        match self.namespace.get(&name)? {
            DeclarationId::Record(id) => Some(*id),
            DeclarationId::Callable(_) => None,
        }
    }
    #[must_use]
    pub fn callables(&self) -> &SparseArena<AccessCallableId, Callable> {
        &self.callables
    }
    #[must_use]
    pub fn parameters(&self) -> &SparseArena<AccessParameterId, Parameter> {
        &self.parameters
    }
    #[must_use]
    pub fn resolve(&self, name: Symbol) -> Option<AccessCallableId> {
        self.resolve_code(name).filter(|id| {
            self.callables
                .get(*id)
                .is_some_and(|c| matches!(c.kind(), CallableKind::Function(_)))
        })
    }
    /// Lookup any source code declaration for collision checks; not a function-value grant.
    #[must_use]
    pub fn resolve_code(&self, name: Symbol) -> Option<AccessCallableId> {
        match self.namespace.get(&name)? {
            DeclarationId::Callable(id) => Some(*id),
            DeclarationId::Record(_) => None,
        }
    }
    #[must_use]
    pub fn resolve_constant(&self, name: Symbol) -> Option<AccessCallableId> {
        self.resolve_code(name).filter(|id| {
            self.callables
                .get(*id)
                .is_some_and(|c| c.kind() == CallableKind::ConstantInitializer)
        })
    }
    #[must_use]
    pub fn resolve_static(&self, name: Symbol) -> Option<nocter_model::ValueStatic> {
        let initializer = self.resolve_code(name)?;
        let id = nocter_model::ValueStatic { initializer };
        self.static_declaration(id)?;
        Some(id)
    }
    #[must_use]
    pub fn static_declaration(&self, id: nocter_model::ValueStatic) -> Option<&Callable> {
        self.callables
            .get(id.initializer)
            .filter(|value| value.kind() == CallableKind::StaticInitializer)
    }
}

#[derive(Debug)]
struct Definition {
    kind: CallableKind,
    parameters: Box<[ParameterDefinition]>,
    result_data: ValueDefinition,
}

#[derive(Debug)]
struct Reservation {
    name: Symbol,
    parameters: Box<[AccessParameterId]>,
    definition: Option<Definition>,
}

/// Identity and authored position, without a provisional data type or input operation.
#[derive(Clone, Copy, Debug)]
struct ParameterSlot {
    owner: AccessCallableId,
    position: usize,
}

/// Construction-only identity reservation. No header can be defined before names are closed.
#[derive(Debug)]
pub struct Reservations {
    symbols: SymbolTable,
    callables: ArenaBuilder<AccessCallableId, Reservation>,
    parameters: ArenaBuilder<AccessParameterId, ParameterSlot>,
    records: records::Builder,
}

impl Reservations {
    #[must_use]
    pub fn new(symbols: SymbolTable) -> Self {
        Self {
            symbols,
            callables: ArenaBuilder::new(),
            parameters: ArenaBuilder::new(),
            records: records::Builder::default(),
        }
    }

    pub fn reserve(&mut self, name: Symbol, parameter_count: usize) -> AccessCallableId {
        let owner = self.callables.next_id();
        let parameters = (0..parameter_count)
            .map(|position| self.parameters.insert(ParameterSlot { owner, position }))
            .collect();
        self.callables.insert(Reservation {
            name,
            parameters,
            definition: None,
        })
    }

    /// Reserve records in canonical namespace order, independently of definition order.
    pub fn reserve_record(&mut self, name: Symbol, field_count: usize) -> NominalTypeId {
        self.records.reserve(name, field_count)
    }

    /// Close the complete name domain before resolving types or checking definitions.
    /// # Errors
    /// Rejects unknown, reserved or colliding names without inventing missing type/header data.
    pub fn finish(self) -> Result<Builder, Vec<DeclarationViolation>> {
        let mut violations = Vec::new();
        let namespace = namespace::close(
            &self.symbols,
            &self.callables,
            &self.records,
            &mut violations,
        );
        if !violations.is_empty() {
            return Err(violations);
        }
        Ok(Builder {
            symbols: self.symbols,
            callables: self.callables,
            parameters: self.parameters.finish(),
            records: self.records,
            namespace,
        })
    }
}

/// Define closed reserved headers in any order. Validation opens the sole publication writer;
/// `finish` closes the complete program through that same path.
/// There is no operation for appending another declaration after reservation closure.
#[derive(Debug)]
pub struct Builder {
    symbols: SymbolTable,
    callables: ArenaBuilder<AccessCallableId, Reservation>,
    parameters: Arena<AccessParameterId, ParameterSlot>,
    records: records::Builder,
    namespace: BTreeMap<Symbol, DeclarationId>,
}

impl Builder {
    /// Reserved input identities in authored order; this grants no typed parameter contract.
    #[must_use]
    pub fn parameter_ids(&self, id: AccessCallableId) -> Option<&[AccessParameterId]> {
        self.callables
            .get(id)
            .map(|reserved| reserved.parameters.as_ref())
    }

    /// Reserved field identities in authored order; this grants no payload or Copy evidence.
    #[must_use]
    pub fn field_ids(&self, id: NominalTypeId) -> Option<&[nocter_model::AccessFieldId]> {
        self.records.field_ids(id)
    }

    /// Defines an ordinary or explicitly Copy record. Every field stores a value, not an access.
    ///
    /// # Errors
    /// Rejects an unknown reservation, a mismatched reserved field count, or a second definition
    /// without changing the first one.
    pub fn define_record(
        &mut self,
        id: NominalTypeId,
        copy: bool,
        fields: impl Into<Box<[FieldDefinition]>>,
    ) -> Result<(), DefinitionError> {
        self.records.define(id, copy, fields.into())
    }

    /// Resolve a header type through the single construction namespace, after all reservations.
    /// # Errors
    /// Rejects a symbol with no known builtin or record declaration.
    pub fn resolve_data(&self, name: Symbol) -> Result<DataReference, DeclarationRule> {
        let spelling = self
            .symbols
            .spelling(name)
            .ok_or(DeclarationRule::UnknownSymbol)?;
        data_reference(&self.symbols, &self.namespace, spelling)
            .ok_or(DeclarationRule::UnknownDataName { name })
    }

    /// # Errors
    /// Rejects a missing reservation, a mismatched reserved input count, or a second definition
    /// without replacing the first one.
    pub fn define(
        &mut self,
        id: AccessCallableId,
        kind: CallableKind,
        parameters: impl Into<Box<[ParameterDefinition]>>,
        result_data: impl Into<ValueDefinition>,
    ) -> Result<(), DefinitionError> {
        let reservation = self
            .callables
            .get_mut(id)
            .ok_or(DefinitionError::UnknownId)?;
        if reservation.definition.is_some() {
            return Err(DefinitionError::AlreadyDefined);
        }
        let parameters = parameters.into();
        if parameters.len() != reservation.parameters.len() {
            return Err(DefinitionError::WrongArity {
                expected: reservation.parameters.len(),
                actual: parameters.len(),
            });
        }
        reservation.definition = Some(Definition {
            kind,
            parameters,
            result_data: result_data.into(),
        });
        Ok(())
    }

    /// # Errors
    /// Returns callable and record header/shape violations without publishing a partial program.
    pub fn finish(self) -> Result<Program, Vec<DeclarationViolation>> {
        self.into_publication().map(HeaderPublication::finish)
    }

    /// Validate closed source definitions before publishing individual callable contracts.
    /// # Errors
    /// Rejects incomplete or invalid definitions without furnishing a ready view.
    pub fn into_publication(self) -> Result<HeaderPublication, Vec<DeclarationViolation>> {
        let reserved = self.callables.finish();
        let mut violations = Vec::new();
        let namespace = self.namespace;
        for (id, reservation) in reserved.iter() {
            validate_definition(&self.symbols, &namespace, id, reservation, &mut violations);
        }
        let records = match self.records.validate(&self.symbols) {
            Ok(records) => records,
            Err(errors) => {
                violations.extend(errors);
                return Err(violations);
            }
        };
        for (id, reservation) in reserved.iter() {
            if let Some(definition) = &reservation.definition {
                let mut validate = |position, data: &DataReference| {
                    if let DataReference::Record(record) = data
                        && !records.contains(*record)
                    {
                        violations.push(DeclarationViolation {
                            declaration: DeclarationId::Callable(id),
                            position,
                            rule: DeclarationRule::UnknownRecordType { record: *record },
                        });
                    }
                };
                for (position, parameter) in definition.parameters.iter().enumerate() {
                    parameter
                        .data
                        .visit_data(|data| validate(HeaderPosition::Parameter(position), data));
                }
                definition
                    .result_data
                    .visit_data(|data| validate(HeaderPosition::Result, data));
            }
        }
        if !violations.is_empty() {
            return Err(violations);
        }
        Ok(HeaderPublication::new(
            self.symbols,
            reserved,
            self.parameters,
            namespace,
            records,
        ))
    }
}

fn validate_definition(
    symbols: &SymbolTable,
    namespace: &BTreeMap<Symbol, DeclarationId>,
    id: AccessCallableId,
    reservation: &Reservation,
    violations: &mut Vec<DeclarationViolation>,
) {
    let Some(definition) = &reservation.definition else {
        violations.push(DeclarationViolation {
            declaration: DeclarationId::Callable(id),
            position: HeaderPosition::Name,
            rule: DeclarationRule::MissingDefinition,
        });
        return;
    };
    if definition.kind.is_initializer()
        && (!definition.parameters.is_empty()
            || matches!(definition.result_data, ValueDefinition::Function(_)))
    {
        violations.push(DeclarationViolation {
            declaration: DeclarationId::Callable(id),
            position: HeaderPosition::Result,
            rule: DeclarationRule::InvalidInitializerHeader,
        });
    }
    let mut seen = BTreeMap::new();
    for (position, parameter) in definition.parameters.iter().enumerate() {
        let rule = match symbols.spelling(parameter.name) {
            None => Some(DeclarationRule::UnknownSymbol),
            Some(name) if BuiltinType::from_spelling(name).is_some() => {
                Some(DeclarationRule::ReservedBuiltinName)
            }
            Some(_) => {
                if let Some(&first) = seen.get(&parameter.name) {
                    Some(DeclarationRule::DuplicateParameter { first })
                } else {
                    seen.insert(parameter.name, position);
                    namespace
                        .get(&parameter.name)
                        .map(|&declaration| match declaration {
                            DeclarationId::Callable(declaration) => {
                                DeclarationRule::ParameterShadowsCallable { declaration }
                            }
                            DeclarationId::Record(declaration) => {
                                DeclarationRule::ParameterShadowsRecord { declaration }
                            }
                        })
                }
            }
        };
        if let Some(rule) = rule {
            violations.push(DeclarationViolation {
                declaration: DeclarationId::Callable(id),
                position: HeaderPosition::Parameter(position),
                rule,
            });
        }
        parameter.data.visit_data(|data| {
            validate_data(id, HeaderPosition::Parameter(position), data, violations);
        });
    }
    definition
        .result_data
        .visit_data(|data| validate_data(id, HeaderPosition::Result, data, violations));
}

fn validate_data(
    callable: AccessCallableId,
    position: HeaderPosition,
    data: &DataReference,
    violations: &mut Vec<DeclarationViolation>,
) {
    if let DataReference::Builtin(data) = data
        && !supported(*data)
    {
        violations.push(DeclarationViolation {
            declaration: DeclarationId::Callable(callable),
            position,
            rule: DeclarationRule::UnsupportedDataType { data: *data },
        });
    }
}
