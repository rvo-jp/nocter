//! Declaration authority for the bounded future private-function language.
//!
//! This product cannot be converted to a released borrow-based declaration program. Its
//! source boundary still admits four built-ins. Nominal payload schemas are validated here before
//! source projection and body checking can admit records; they are not a flattened representation
//! of the future aggregate/outcome type grammar.
use std::collections::BTreeMap;

use nocter_model::{
    AccessCallableId, AccessParameterId, Arena, ArenaBuilder, BuiltinType, NominalTypeId, Symbol,
    SymbolTable, TypeAuthority, TypeId, ValueInputOperation,
};

use crate::DefinitionError;

mod data;
mod records;
pub use data::DataReference;
use data::{data_reference, resolved_data, supported};
pub use records::{Field, FieldDefinition, PayloadSchema, Record};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclarationId {
    Callable(AccessCallableId),
    Record(NominalTypeId),
}

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParameterDefinition {
    pub name: Symbol,
    pub data: DataReference,
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
    NonCopyField { data: DataReference },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeclarationViolation {
    pub declaration: DeclarationId,
    pub position: HeaderPosition,
    pub rule: DeclarationRule,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Callable {
    name: Symbol,
    parameters: Box<[AccessParameterId]>,
    result: TypeId,
}

impl Callable {
    #[must_use]
    pub const fn name(&self) -> Symbol {
        self.name
    }
    #[must_use]
    pub const fn parameters(&self) -> &[AccessParameterId] {
        &self.parameters
    }
    #[must_use]
    pub const fn result(&self) -> TypeId {
        self.result
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Parameter {
    owner: AccessCallableId,
    name: Symbol,
    position: usize,
    data: TypeId,
    operation: ValueInputOperation,
}

impl Parameter {
    #[must_use]
    pub const fn owner(self) -> AccessCallableId {
        self.owner
    }
    #[must_use]
    pub const fn name(self) -> Symbol {
        self.name
    }
    #[must_use]
    pub const fn position(self) -> usize {
        self.position
    }
    #[must_use]
    pub const fn data(self) -> TypeId {
        self.data
    }
    #[must_use]
    pub const fn operation(self) -> ValueInputOperation {
        self.operation
    }
}

/// Immutable declaration metadata; body acceptance and native execution are separate products.
#[derive(Clone, Debug)]
pub struct Program {
    symbols: SymbolTable,
    types: TypeAuthority,
    callables: Arena<AccessCallableId, Callable>,
    parameters: Arena<AccessParameterId, Parameter>,
    namespace: BTreeMap<Symbol, AccessCallableId>,
    records: records::Schemas,
}

impl Program {
    #[must_use]
    pub const fn symbols(&self) -> &SymbolTable {
        &self.symbols
    }
    #[must_use]
    pub const fn records(&self) -> &Arena<NominalTypeId, Record> {
        &self.records.records
    }
    #[must_use]
    pub const fn fields(&self) -> &Arena<nocter_model::AccessFieldId, Field> {
        &self.records.fields
    }
    #[must_use]
    pub fn resolve_record(&self, name: Symbol) -> Option<NominalTypeId> {
        self.records.namespace.get(&name).copied()
    }
    #[must_use]
    pub const fn callables(&self) -> &Arena<AccessCallableId, Callable> {
        &self.callables
    }
    #[must_use]
    pub const fn parameters(&self) -> &Arena<AccessParameterId, Parameter> {
        &self.parameters
    }
    #[must_use]
    pub fn resolve(&self, name: Symbol) -> Option<AccessCallableId> {
        self.namespace.get(&name).copied()
    }
}

#[derive(Debug)]
struct Definition {
    parameters: Box<[ParameterDefinition]>,
    result_data: DataReference,
}

#[derive(Debug)]
struct Reservation {
    name: Symbol,
    definition: Option<Definition>,
}

/// Reserve in canonical namespace order, then define in any order. Only `finish` publishes metadata.
#[derive(Debug)]
pub struct Builder {
    symbols: SymbolTable,
    reservations: ArenaBuilder<AccessCallableId, Reservation>,
    records: records::Builder,
}

impl Builder {
    #[must_use]
    pub fn new(symbols: SymbolTable) -> Self {
        Self {
            symbols,
            reservations: ArenaBuilder::new(),
            records: records::Builder::default(),
        }
    }

    pub fn reserve(&mut self, name: Symbol) -> AccessCallableId {
        self.reservations.insert(Reservation {
            name,
            definition: None,
        })
    }

    /// Reserve records in canonical namespace order, independently of definition order.
    pub fn reserve_record(&mut self, name: Symbol) -> NominalTypeId {
        self.records.reserve(name)
    }

    /// Defines an ordinary or explicitly Copy record. Every field stores a value, not an access.
    ///
    /// # Errors
    /// Rejects an unknown reservation or a second definition without changing the first one.
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
        data_reference(&self.symbols, self.records.namespace(), spelling)
            .ok_or(DeclarationRule::UnknownDataName { name })
    }

    /// # Errors
    /// Rejects a missing reservation or a second definition without replacing the first one.
    pub fn define(
        &mut self,
        id: AccessCallableId,
        parameters: impl Into<Box<[ParameterDefinition]>>,
        result_data: impl Into<DataReference>,
    ) -> Result<(), DefinitionError> {
        let reservation = self
            .reservations
            .get_mut(id)
            .ok_or(DefinitionError::UnknownId)?;
        if reservation.definition.is_some() {
            return Err(DefinitionError::AlreadyDefined);
        }
        reservation.definition = Some(Definition {
            parameters: parameters.into(),
            result_data: result_data.into(),
        });
        Ok(())
    }

    /// # Errors
    /// Returns callable and record header/shape violations without publishing a partial program.
    pub fn finish(self) -> Result<Program, Vec<DeclarationViolation>> {
        let reserved = self.reservations.finish();
        let mut violations = Vec::new();
        let namespace = validate_namespace(&self.symbols, &reserved, &mut violations);
        for (id, reservation) in reserved.iter() {
            validate_definition(&self.symbols, &namespace, id, reservation, &mut violations);
        }
        let records = match self.records.validate(&self.symbols, &namespace) {
            Ok(records) => records,
            Err(errors) => {
                violations.extend(errors);
                return Err(violations);
            }
        };
        for (id, reservation) in reserved.iter() {
            if let Some(definition) = &reservation.definition {
                for (position, parameter) in definition.parameters.iter().enumerate() {
                    if let Some(declaration) = records.resolve(parameter.name) {
                        violations.push(DeclarationViolation {
                            declaration: DeclarationId::Callable(id),
                            position: HeaderPosition::Parameter(position),
                            rule: DeclarationRule::ParameterShadowsRecord { declaration },
                        });
                    }
                }
            }
        }
        for (id, reservation) in reserved.iter() {
            if let Some(definition) = &reservation.definition {
                for (position, data) in definition
                    .parameters
                    .iter()
                    .enumerate()
                    .map(|(i, p)| (HeaderPosition::Parameter(i), p.data))
                    .chain(std::iter::once((
                        HeaderPosition::Result,
                        definition.result_data,
                    )))
                {
                    if let DataReference::Record(record) = data
                        && !records.contains(record)
                    {
                        violations.push(DeclarationViolation {
                            declaration: DeclarationId::Callable(id),
                            position,
                            rule: DeclarationRule::UnknownRecordType { record },
                        });
                    }
                }
            }
        }
        if !violations.is_empty() {
            return Err(violations);
        }
        let (types, records) = records.publish();
        let mut parameters = ArenaBuilder::new();
        let callables = reserved.try_map(
            |id, reservation| -> Result<Callable, Vec<DeclarationViolation>> {
                let definition = reservation.definition.as_ref().ok_or_else(|| {
                    vec![DeclarationViolation {
                        declaration: DeclarationId::Callable(id),
                        position: HeaderPosition::Name,
                        rule: DeclarationRule::MissingDefinition,
                    }]
                })?;
                let parameter_ids = definition
                    .parameters
                    .iter()
                    .enumerate()
                    .map(|(position, parameter)| {
                        parameters.insert(Parameter {
                            owner: id,
                            name: parameter.name,
                            position,
                            data: resolved_data(&types, &records, parameter.data),
                            operation: parameter.operation,
                        })
                    })
                    .collect::<Vec<_>>();
                Ok(Callable {
                    name: reservation.name,
                    parameters: parameter_ids.into_boxed_slice(),
                    result: resolved_data(&types, &records, definition.result_data),
                })
            },
        )?;
        Ok(Program {
            symbols: self.symbols,
            types,
            callables,
            parameters: parameters.finish(),
            namespace,
            records,
        })
    }
}

fn validate_namespace(
    symbols: &SymbolTable,
    reserved: &Arena<AccessCallableId, Reservation>,
    violations: &mut Vec<DeclarationViolation>,
) -> BTreeMap<Symbol, AccessCallableId> {
    let mut namespace = BTreeMap::new();
    for (id, reservation) in reserved.iter() {
        let rule = match symbols.spelling(reservation.name) {
            None => Some(DeclarationRule::UnknownSymbol),
            Some(name) if BuiltinType::from_spelling(name).is_some() => {
                Some(DeclarationRule::ReservedBuiltinName)
            }
            Some(_) => {
                if let Some(&first) = namespace.get(&reservation.name) {
                    Some(DeclarationRule::DuplicateCallable { first })
                } else {
                    namespace.insert(reservation.name, id);
                    None
                }
            }
        };
        if let Some(rule) = rule {
            violations.push(DeclarationViolation {
                declaration: DeclarationId::Callable(id),
                position: HeaderPosition::Name,
                rule,
            });
        }
    }
    namespace
}

fn validate_definition(
    symbols: &SymbolTable,
    namespace: &BTreeMap<Symbol, AccessCallableId>,
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
                } else if let Some(&declaration) = namespace.get(&parameter.name) {
                    Some(DeclarationRule::ParameterShadowsCallable { declaration })
                } else {
                    seen.insert(parameter.name, position);
                    None
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
        validate_data(
            id,
            HeaderPosition::Parameter(position),
            parameter.data,
            violations,
        );
    }
    validate_data(
        id,
        HeaderPosition::Result,
        definition.result_data,
        violations,
    );
}

fn validate_data(
    callable: AccessCallableId,
    position: HeaderPosition,
    data: DataReference,
    violations: &mut Vec<DeclarationViolation>,
) {
    if let DataReference::Builtin(data) = data
        && !supported(data)
    {
        violations.push(DeclarationViolation {
            declaration: DeclarationId::Callable(callable),
            position,
            rule: DeclarationRule::UnsupportedDataType { data },
        });
    }
}
