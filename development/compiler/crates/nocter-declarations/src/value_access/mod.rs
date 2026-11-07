//! Declaration authority for the bounded future private-function language.
//!
//! This product cannot be converted to a released borrow-based declaration program. Its data
//! shapes are currently the four built-ins admitted by the first access grammar, not a flattened
//! representation of the future aggregate/outcome type grammar.
use std::collections::BTreeMap;

use nocter_model::{
    AccessCallableId, AccessParameterId, Arena, ArenaBuilder, BuiltinType, Symbol, SymbolTable,
    TypeAuthority, TypeId, TypeKind, ValueAccessMode, ValueAccessType,
};

use crate::DefinitionError;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParameterDefinition {
    pub name: Symbol,
    pub data: BuiltinType,
    pub mode: ValueAccessMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HeaderPosition {
    Name,
    Parameter(usize),
    Result,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclarationRule {
    MissingDefinition,
    UnknownSymbol,
    ReservedBuiltinName,
    DuplicateCallable { first: AccessCallableId },
    DuplicateParameter { first: usize },
    ParameterShadowsCallable { declaration: AccessCallableId },
    UnsupportedDataType { data: BuiltinType },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeclarationViolation {
    pub callable: AccessCallableId,
    pub position: HeaderPosition,
    pub rule: DeclarationRule,
}

/// Only the result's own referent R. This is never a promise that payload D/storage S are empty.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResultReferent {
    /// X or an independent concrete I Copy answer; the declared permission remains unchanged.
    Value,
    /// An upper bound over eligible caller inputs, including an empty or alternative set.
    Inputs(Box<[AccessParameterId]>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Callable {
    name: Symbol,
    parameters: Box<[AccessParameterId]>,
    result: ValueAccessType,
    referent: ResultReferent,
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
    pub const fn result(&self) -> ValueAccessType {
        self.result
    }
    #[must_use]
    pub const fn result_referent(&self) -> &ResultReferent {
        &self.referent
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Parameter {
    owner: AccessCallableId,
    name: Symbol,
    position: usize,
    ty: ValueAccessType,
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
    pub const fn ty(self) -> ValueAccessType {
        self.ty
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
}

impl Program {
    #[must_use]
    pub const fn symbols(&self) -> &SymbolTable {
        &self.symbols
    }
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
    /// Structural Copy evidence for the payload, never permission to duplicate an access.
    #[must_use]
    pub fn data_copyable(&self, data: TypeId) -> Option<bool> {
        self.data_kind(data).map(|data| data != BuiltinType::Error)
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
    result_data: BuiltinType,
    result_mode: ValueAccessMode,
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
}

impl Builder {
    #[must_use]
    pub fn new(symbols: SymbolTable) -> Self {
        Self {
            symbols,
            reservations: ArenaBuilder::new(),
        }
    }

    pub fn reserve(&mut self, name: Symbol) -> AccessCallableId {
        self.reservations.insert(Reservation {
            name,
            definition: None,
        })
    }

    /// # Errors
    /// Rejects a missing reservation or a second definition without replacing the first one.
    pub fn define(
        &mut self,
        id: AccessCallableId,
        parameters: impl Into<Box<[ParameterDefinition]>>,
        result_data: BuiltinType,
        result_mode: ValueAccessMode,
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
            result_data,
            result_mode,
        });
        Ok(())
    }

    /// # Errors
    /// Returns all header violations in reservation/parameter order, with no partial program.
    pub fn finish(self) -> Result<Program, Vec<DeclarationViolation>> {
        let reserved = self.reservations.finish();
        let mut violations = Vec::new();
        let namespace = validate_namespace(&self.symbols, &reserved, &mut violations);
        for (id, reservation) in reserved.iter() {
            validate_definition(&self.symbols, &namespace, id, reservation, &mut violations);
        }
        if !violations.is_empty() {
            return Err(violations);
        }
        let types = TypeAuthority::new();
        let mut parameters = ArenaBuilder::new();
        let callables = reserved.try_map(
            |id, reservation| -> Result<Callable, Vec<DeclarationViolation>> {
                let definition = reservation.definition.as_ref().ok_or_else(|| {
                    vec![DeclarationViolation {
                        callable: id,
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
                            ty: ValueAccessType::new(
                                types.store().builtin(parameter.data),
                                parameter.mode,
                            ),
                        })
                    })
                    .collect::<Vec<_>>();
                let referent = result_referent(definition, &parameter_ids);
                Ok(Callable {
                    name: reservation.name,
                    parameters: parameter_ids.into_boxed_slice(),
                    result: ValueAccessType::new(
                        types.store().builtin(definition.result_data),
                        definition.result_mode,
                    ),
                    referent,
                })
            },
        )?;
        Ok(Program {
            symbols: self.symbols,
            types,
            callables,
            parameters: parameters.finish(),
            namespace,
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
                callable: id,
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
            callable: id,
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
                callable: id,
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
    data: BuiltinType,
    violations: &mut Vec<DeclarationViolation>,
) {
    if !supported(data) {
        violations.push(DeclarationViolation {
            callable,
            position,
            rule: DeclarationRule::UnsupportedDataType { data },
        });
    }
}

fn result_referent(definition: &Definition, parameters: &[AccessParameterId]) -> ResultReferent {
    if definition.result_mode == ValueAccessMode::Own
        || definition.result_mode == ValueAccessMode::Read
            && definition.result_data != BuiltinType::Error
    {
        return ResultReferent::Value;
    }
    ResultReferent::Inputs(
        definition
            .parameters
            .iter()
            .zip(parameters)
            .filter_map(|(input, &id)| {
                (input.data == definition.result_data
                    && match definition.result_mode {
                        ValueAccessMode::Read => input.mode != ValueAccessMode::Own,
                        ValueAccessMode::Update => input.mode == ValueAccessMode::Update,
                        ValueAccessMode::Own => false,
                    })
                .then_some(id)
            })
            .collect(),
    )
}

fn supported(data: BuiltinType) -> bool {
    matches!(
        data,
        BuiltinType::Bool | BuiltinType::I32 | BuiltinType::Void | BuiltinType::Error
    )
}
