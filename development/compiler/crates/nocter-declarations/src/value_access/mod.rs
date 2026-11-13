//! Declaration authority for the bounded future private core.
//!
//! This product cannot be converted to a released borrow-based declaration program. Its
//! source boundary admits closed data and named/formal function witnesses. It does not flatten the
//! complete future aggregate/callable/outcome grammar into released borrow-based types.
use std::{collections::BTreeMap, sync::Arc};

use nocter_model::{
    AccessCodeId, AccessParameterId, Arena, ArenaBuilder, BuiltinType, CompileTimeGuarantee,
    NominalTypeId, SparseArena, Symbol, SymbolTable, TypeAuthority, TypeId, ValueInputOperation,
};

use crate::DefinitionError;

mod body_types;
mod closures;
pub use body_types::{
    BodyTypeError, BodyTypes, CanonicalBodyTypes, PayloadReadiness, ResolvedBodyTypes,
};
pub use closures::Closure;
mod aggregate;
pub use aggregate::{AggregateForm, AggregateSchema};
mod data;
mod functions;
pub use functions::{
    FunctionConstraint, FunctionDataPattern, FunctionDefinition, FunctionInput, FunctionMismatch,
    FunctionPattern, FunctionPatternMismatch, FunctionPosition, FunctionWitness, ValueDefinition,
};
mod namespace;
pub use namespace::{Namespace, QualifiedNameError, ScopeBindingError, ScopeId};
mod publication;
pub use publication::{HeaderFailure, HeaderPublication, HeaderStep};
mod records;
use data::supported;
pub use data::{ArraySchema, Component, DataReference, PayloadSchema, TupleData};
pub use records::{Field, FieldDefinition, Record};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DeclarationId {
    Code(AccessCodeId),
    Record(NominalTypeId),
}
impl From<AccessCodeId> for DeclarationId {
    fn from(id: AccessCodeId) -> Self {
        Self::Code(id)
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
    DuplicateValue { first: AccessCodeId },
    DuplicateParameter { first: usize },
    ParameterShadowsValue { declaration: AccessCodeId },
    ParameterShadowsNamespace { name: Symbol },
    NameConflictsWithNamespace { name: Symbol },
    InconsistentBindingAudience { name: Symbol },
    UnsupportedDataType { data: BuiltinType },
    UnsupportedCallableData,
    DuplicateRecord { first: NominalTypeId },
    DuplicateField { first: usize },
    UnknownRecordType { record: NominalTypeId },
    UnknownDataName { name: Symbol },
    QualifiedDataName { error: QualifiedNameError },
    RecursivePayload { through: NominalTypeId },
    NonCopyField,
    RepeatedFailure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeclarationViolation {
    pub declaration: DeclarationId,
    pub position: HeaderPosition,
    pub rule: DeclarationRule,
}

/// Source-independent owner of an anonymous required type expression.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TypeExpressionSource {
    pub declaration: DeclarationId,
    pub position: HeaderPosition,
}

/// Source role of executable code. A constant owns an initialization thunk, never a function witness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodeKind {
    Function(CompileTimeGuarantee),
    /// A bodyless callable declaration; primitive authority is a separate lowering receipt.
    Primitive,
    ConstantInitializer,
    StaticInitializer,
    TypeExpression(TypeExpressionSource),
}
impl CodeKind {
    #[must_use]
    pub const fn is_callable(self) -> bool {
        matches!(self, Self::Function(_) | Self::Primitive)
    }
    #[must_use]
    pub const fn is_initializer(self) -> bool {
        matches!(
            self,
            Self::ConstantInitializer | Self::StaticInitializer | Self::TypeExpression(_)
        )
    }
    #[must_use]
    pub const fn compile_time(self) -> CompileTimeGuarantee {
        match self {
            Self::Function(capability) => capability,
            Self::Primitive => CompileTimeGuarantee::RuntimeOnly,
            Self::ConstantInitializer | Self::StaticInitializer | Self::TypeExpression(_) => {
                CompileTimeGuarantee::Evaluatable
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Code {
    name: Option<Symbol>,
    kind: CodeKind,
    parameters: Box<[AccessParameterId]>,
    result: TypeId,
    result_contract: Option<FunctionConstraint>,
}

impl Code {
    #[must_use]
    pub const fn kind(&self) -> CodeKind {
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
    pub const fn name(&self) -> Option<Symbol> {
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
    owner: AccessCodeId,
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
    pub const fn owner(&self) -> AccessCodeId {
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
    Body(nocter_model::ValueCodeRoot),
    Canonical,
}

/// Immutable declaration metadata; body acceptance and native execution are separate products.
#[derive(Clone, Debug)]
pub struct Program {
    symbols: std::sync::Arc<SymbolTable>,
    types: TypeAuthority,
    header_prefix: nocter_model::TypeStore,
    type_scope: TypeScope,
    codes: SparseArena<AccessCodeId, Code>,
    parameters: SparseArena<AccessParameterId, Parameter>,
    namespace: std::sync::Arc<namespace::Names>,
    records: records::Schemas,
    closures: BTreeMap<nocter_model::ValueClosure, Closure>,
    closure_parameters: BTreeMap<nocter_model::ValueParameter, FunctionConstraint>,
}

impl Program {
    /// Validate names in one physical header scope without publishing typed inputs.
    /// # Errors
    /// Rejects an unknown scope or named declaration identity.
    pub fn parameter_name_violations_in(
        &self,
        scope: ScopeId,
        code: AccessCodeId,
        names: impl IntoIterator<Item = Symbol>,
    ) -> Result<Vec<DeclarationViolation>, DefinitionError> {
        if !self.has_scope(scope)
            || !self
                .namespace
                .declarations()
                .any(|id| id == DeclarationId::Code(code))
        {
            return Err(DefinitionError::UnknownId);
        }
        let mut violations = Vec::new();
        validate_parameter_names(
            &self.symbols,
            &self.namespace,
            scope,
            code,
            names,
            &mut violations,
        );
        Ok(violations)
    }

    #[must_use]
    pub const fn headers_complete(&self) -> bool {
        self.codes.is_complete()
            && self.parameters.is_complete()
            && self.records.records.is_complete()
            && self.records.fields.is_complete()
    }
    /// A complete published contract, including its established transitive record schemas.
    #[must_use]
    pub fn header_ready(&self, declaration: impl Into<DeclarationId>) -> bool {
        match declaration.into() {
            DeclarationId::Code(id) => self.codes.get(id).is_some(),
            DeclarationId::Record(id) => self.records.records.get(id).is_some(),
        }
    }
    /// Retains every established contract and its exact type prefix from the same construction.
    #[must_use]
    pub fn preserves_headers(&self, older: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.namespace, &older.namespace)
            && self.records.records.preserves(&older.records.records)
            && self.records.fields.preserves(&older.records.fields)
            && self.codes.preserves(&older.codes)
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
    pub fn declaration(&self, namespace: Namespace, name: Symbol) -> Option<DeclarationId> {
        self.declaration_in(ScopeId::ROOT, namespace, name)
    }
    /// Closed lookup through explicitly constructed lexical parents only.
    /// Independent source/module scopes have no implicit fallback or transitive exposure.
    #[must_use]
    pub fn declaration_in(
        &self,
        scope: ScopeId,
        namespace: Namespace,
        name: Symbol,
    ) -> Option<DeclarationId> {
        self.namespace.get(scope, namespace, name)
    }
    /// Query an outward name using its audience and an explicit original logical caller.
    /// This grants a declaration identity, not header or value readiness.
    #[must_use]
    pub fn declaration_visible_from(
        &self,
        scope: ScopeId,
        namespace: Namespace,
        name: Symbol,
        caller: &nocter_model::ModuleIdentity,
    ) -> Option<DeclarationId> {
        self.namespace.visible_from(scope, namespace, name, caller)
    }
    /// Namespace presence is a name fact, never a callable/value/type grant.
    #[must_use]
    pub fn has_namespace_in(&self, scope: ScopeId, name: Symbol) -> bool {
        self.namespace.has_namespace(scope, name)
    }
    /// Resolve a finite qualified path while retaining the original source scope's caller module.
    /// # Errors
    /// Reports the failed path position; no fallback, interning or typed readiness is implied.
    pub fn declaration_path_in(
        &self,
        scope: ScopeId,
        namespace: Namespace,
        path: &[Option<Symbol>],
    ) -> Result<DeclarationId, QualifiedNameError> {
        self.namespace.lookup_path(scope, namespace, path)
    }
    /// Resolve the namespace/declaration prefix once, leaving record/tuple components to typing.
    /// An uninterned spelling is `None`; suffix spellings are never required to name declarations.
    /// # Errors
    /// Reports the first failed namespace/declaration position without fallback or type readiness.
    pub fn declaration_prefix_in(
        &self,
        scope: ScopeId,
        namespace: Namespace,
        path: &[Option<Symbol>],
    ) -> Result<(DeclarationId, usize), QualifiedNameError> {
        self.namespace.lookup_prefix(scope, namespace, path)
    }
    #[must_use]
    pub fn has_scope(&self, scope: ScopeId) -> bool {
        self.namespace.contains_scope(scope)
    }
    #[must_use]
    pub fn resolve_record(&self, name: Symbol) -> Option<NominalTypeId> {
        self.resolve_record_in(ScopeId::ROOT, name)
    }
    #[must_use]
    pub fn resolve_record_in(&self, scope: ScopeId, name: Symbol) -> Option<NominalTypeId> {
        match self.declaration_in(scope, Namespace::Type, name)? {
            DeclarationId::Record(id) => Some(id),
            DeclarationId::Code(_) => None,
        }
    }
    #[must_use]
    pub fn codes(&self) -> &SparseArena<AccessCodeId, Code> {
        &self.codes
    }
    #[must_use]
    pub fn parameters(&self) -> &SparseArena<AccessParameterId, Parameter> {
        &self.parameters
    }
    #[must_use]
    pub fn resolve(&self, name: Symbol) -> Option<AccessCodeId> {
        self.resolve_in(ScopeId::ROOT, name)
    }
    #[must_use]
    pub fn resolve_in(&self, scope: ScopeId, name: Symbol) -> Option<AccessCodeId> {
        self.resolve_code_in(scope, name)
            .filter(|id| self.codes.get(*id).is_some_and(|c| c.kind().is_callable()))
    }
    /// Lookup any source code declaration for collision checks; not a function-value grant.
    #[must_use]
    pub fn resolve_code(&self, name: Symbol) -> Option<AccessCodeId> {
        self.resolve_code_in(ScopeId::ROOT, name)
    }
    #[must_use]
    pub fn resolve_code_in(&self, scope: ScopeId, name: Symbol) -> Option<AccessCodeId> {
        match self.declaration_in(scope, Namespace::Value, name)? {
            DeclarationId::Code(id) => Some(id),
            DeclarationId::Record(_) => None,
        }
    }
    #[must_use]
    pub fn resolve_constant(&self, name: Symbol) -> Option<AccessCodeId> {
        self.resolve_constant_in(ScopeId::ROOT, name)
    }
    #[must_use]
    pub fn resolve_constant_in(&self, scope: ScopeId, name: Symbol) -> Option<AccessCodeId> {
        self.resolve_code_in(scope, name).filter(|id| {
            self.codes
                .get(*id)
                .is_some_and(|c| c.kind() == CodeKind::ConstantInitializer)
        })
    }
    #[must_use]
    pub fn resolve_static(&self, name: Symbol) -> Option<nocter_model::ValueStatic> {
        self.resolve_static_in(ScopeId::ROOT, name)
    }
    #[must_use]
    pub fn resolve_static_in(
        &self,
        scope: ScopeId,
        name: Symbol,
    ) -> Option<nocter_model::ValueStatic> {
        let initializer = self.resolve_code_in(scope, name)?;
        let id = nocter_model::ValueStatic { initializer };
        self.static_declaration(id)?;
        Some(id)
    }
    /// Presentation of any named contract, including one whose schema is not ready.
    #[must_use]
    pub fn declaration_label(&self, declaration: DeclarationId) -> Option<String> {
        match declaration {
            DeclarationId::Code(id) if self.codes.get(id).is_some() => return self.code_label(id),
            DeclarationId::Record(id) if self.records.records.get(id).is_some() => {
                return self
                    .symbols
                    .spelling(self.records.records.get(id)?.name())
                    .map(str::to_owned);
            }
            _ => {}
        }
        let name = self.namespace.name(declaration)?;
        self.symbols.spelling(name).map(str::to_owned)
    }
    /// Presentation only: anonymous code is labeled by its real owning declaration.
    #[must_use]
    pub fn root_label(&self, root: nocter_model::ValueCodeRoot) -> Option<String> {
        match root {
            nocter_model::ValueCodeRoot::Declaration(id) => self.code_label(id),
            nocter_model::ValueCodeRoot::Expression(id) => Some(format!(
                "expression {} in {}",
                id.slot,
                self.declaration_label(id.owner.into())?
            )),
        }
    }
    /// Presentation only: anonymous code is labeled by its real owning declaration.
    #[must_use]
    pub fn code_label(&self, id: AccessCodeId) -> Option<String> {
        let code = self.codes.get(id)?;
        if let Some(name) = code.name() {
            return self.symbols.spelling(name).map(str::to_owned);
        }
        let CodeKind::TypeExpression(source) = code.kind() else {
            return None;
        };
        Some(format!(
            "type expression in {}",
            self.declaration_label(source.declaration)?
        ))
    }
    #[must_use]
    pub fn static_declaration(&self, id: nocter_model::ValueStatic) -> Option<&Code> {
        self.codes
            .get(id.initializer)
            .filter(|value| value.kind() == CodeKind::StaticInitializer)
    }
}

#[derive(Debug)]
struct Definition {
    kind: CodeKind,
    parameters: Box<[ParameterDefinition]>,
    result_data: ValueDefinition,
}

#[derive(Debug)]
struct Reservation {
    scope: ScopeId,
    name: Option<Symbol>,
    parameters: Box<[AccessParameterId]>,
    definition: Option<Definition>,
}

/// Identity and authored position, without a provisional data type or input operation.
#[derive(Clone, Copy, Debug)]
struct ParameterSlot {
    owner: AccessCodeId,
    position: usize,
}

/// Construction-only identity reservation. Authored headers load only after names are closed.
#[derive(Debug)]
pub struct Reservations {
    symbols: Arc<SymbolTable>,
    scopes: namespace::Scopes,
    codes: ArenaBuilder<AccessCodeId, Reservation>,
    parameters: ArenaBuilder<AccessParameterId, ParameterSlot>,
    records: records::Builder,
}

impl Reservations {
    #[must_use]
    pub fn new(symbols: SymbolTable) -> Self {
        Self::with_symbols(Arc::new(symbols))
    }
    /// Share one exact immutable spelling domain with the source declaration inventory.
    #[must_use]
    pub fn with_symbols(symbols: Arc<SymbolTable>) -> Self {
        Self {
            symbols,
            scopes: namespace::Scopes::default(),
            codes: ArenaBuilder::new(),
            parameters: ArenaBuilder::new(),
            records: records::Builder::default(),
        }
    }

    /// Create another independent lookup scope before closing the namespace.
    /// # Panics
    /// Panics if the scope identity space is exhausted.
    pub fn new_scope(&mut self) -> ScopeId {
        self.scopes.new_scope()
    }
    /// Create a lexical child of an existing scope. Its caller module is inherited from that parent.
    /// The parent is fixed at construction; sibling and unrelated scopes remain inaccessible.
    /// # Errors
    /// Rejects an unknown parent without reserving a scope.
    /// # Panics
    /// Panics if the scope identity space is exhausted.
    pub fn new_lexical_scope(&mut self, parent: ScopeId) -> Result<ScopeId, ScopeBindingError> {
        self.scopes.new_lexical_scope(parent)
    }
    /// Bind an existing declaration under a name in one scope, without cloning its identity.
    /// Repeated equal bindings have set semantics. Ordinary name conflicts close at `finish`.
    /// # Errors
    /// Rejects an unknown scope or target without changing the reservation.
    pub fn bind(
        &mut self,
        scope: ScopeId,
        name: Symbol,
        declaration: impl Into<DeclarationId>,
    ) -> Result<(), DefinitionError> {
        self.bind_with_audience(scope, name, declaration.into(), None)
    }
    fn bind_with_audience(
        &mut self,
        scope: ScopeId,
        name: Symbol,
        declaration: DeclarationId,
        audience: Option<Arc<nocter_model::ModuleAudience>>,
    ) -> Result<(), DefinitionError> {
        let known = match declaration {
            DeclarationId::Code(id) => self.codes.get(id).is_some_and(|code| code.name.is_some()),
            DeclarationId::Record(id) => self.records.field_ids(id).is_some(),
        };
        if !self.scopes.contains(scope) || !known {
            return Err(DefinitionError::UnknownId);
        }
        self.scopes.bind(scope, name, declaration, audience);
        Ok(())
    }

    /// Bind an outward declaration under one shared normalized audience.
    /// # Errors
    /// Rejects unknown scopes/targets before changing construction state.
    pub fn bind_export(
        &mut self,
        scope: ScopeId,
        name: Symbol,
        declaration: impl Into<DeclarationId>,
        audience: Arc<nocter_model::ModuleAudience>,
    ) -> Result<(), DefinitionError> {
        self.bind_with_audience(scope, name, declaration.into(), Some(audience))
    }
    /// Associate a lookup scope with its original logical caller, independently of syntax.
    /// # Errors
    /// Rejects an unknown scope or a different existing module without replacement.
    pub fn set_scope_module(
        &mut self,
        scope: ScopeId,
        module: nocter_model::ModuleIdentity,
    ) -> Result<(), ScopeBindingError> {
        self.scopes.set_module(scope, module)
    }
    /// Bind a namespace path step, never a value, type, declaration or payload stand-in.
    /// # Errors
    /// Rejects unknown scopes/symbols, reserved builtin names and conflicting links without replacement.
    pub fn bind_namespace(
        &mut self,
        scope: ScopeId,
        name: Symbol,
        target: ScopeId,
        audience: Option<Arc<nocter_model::ModuleAudience>>,
    ) -> Result<(), ScopeBindingError> {
        self.scopes
            .bind_namespace(&self.symbols, scope, name, target, audience)
    }
    pub fn reserve(&mut self, name: Symbol, parameter_count: usize) -> AccessCodeId {
        self.reserve_known_scope(ScopeId::ROOT, name, parameter_count)
    }
    /// Reserve a named callable/value in one known scope.
    /// # Errors
    /// Rejects an unknown scope before reserving code or parameters.
    pub fn reserve_in(
        &mut self,
        scope: ScopeId,
        name: Symbol,
        parameter_count: usize,
    ) -> Result<AccessCodeId, DefinitionError> {
        if !self.scopes.contains(scope) {
            return Err(DefinitionError::UnknownId);
        }
        Ok(self.reserve_known_scope(scope, name, parameter_count))
    }
    fn reserve_known_scope(
        &mut self,
        scope: ScopeId,
        name: Symbol,
        parameter_count: usize,
    ) -> AccessCodeId {
        let owner = self.codes.next_id();
        let parameters = (0..parameter_count)
            .map(|position| self.parameters.insert(ParameterSlot { owner, position }))
            .collect();
        self.codes.insert(Reservation {
            scope,
            name: Some(name),
            parameters,
            definition: None,
        })
    }

    /// Reserve an anonymous required expression in an existing authored type slot.
    /// It has no namespace name, function witness, formal inputs or source return target.
    /// # Errors
    /// Rejects an unknown/non-type slot without reserving code.
    pub fn reserve_type_expression(
        &mut self,
        source: TypeExpressionSource,
    ) -> Result<AccessCodeId, DefinitionError> {
        let scope = match source.declaration {
            DeclarationId::Code(id) => self.codes.get(id).ok_or(DefinitionError::UnknownId)?.scope,
            DeclarationId::Record(id) => {
                self.records.scope(id).ok_or(DefinitionError::UnknownId)?
            }
        };
        self.reserve_type_expression_in(scope, source)
    }

    /// Reserve a physical header expression in its own lookup scope. Paired headers can share
    /// their semantic owner while their count suppliers resolve through different source scopes.
    /// # Errors
    /// Rejects an unknown scope or non-type owner slot before allocating anonymous code.
    pub fn reserve_type_expression_in(
        &mut self,
        scope: ScopeId,
        source: TypeExpressionSource,
    ) -> Result<AccessCodeId, DefinitionError> {
        if !self.scopes.contains(scope) {
            return Err(DefinitionError::UnknownId);
        }
        let valid = match source.declaration {
            DeclarationId::Code(id) => self.codes.get(id).is_some_and(|code| {
                code.name.is_some() && match source.position {
                    HeaderPosition::Parameter(index) => index < code.parameters.len(),
                    HeaderPosition::Result => true,
                    _ => false,
                }
            }),
            DeclarationId::Record(id) => self.records.field_ids(id).is_some_and(|fields| {
                matches!(source.position, HeaderPosition::Field(index) if index < fields.len())
            }),
        };
        if !valid {
            return Err(DefinitionError::UnknownId);
        }
        Ok(self.codes.insert(Reservation {
            scope,
            name: None,
            parameters: Box::new([]),
            definition: Some(Definition {
                kind: CodeKind::TypeExpression(source),
                parameters: Box::new([]),
                result_data: BuiltinType::Usize.into(),
            }),
        }))
    }

    /// Reserve records in canonical namespace order, independently of definition order.
    pub fn reserve_record(&mut self, name: Symbol, field_count: usize) -> NominalTypeId {
        self.records.reserve(ScopeId::ROOT, name, field_count)
    }
    /// Reserve a nominal identity and its fields in one known scope.
    /// # Errors
    /// Rejects an unknown scope before reserving any nominal or field slot.
    pub fn reserve_record_in(
        &mut self,
        scope: ScopeId,
        name: Symbol,
        field_count: usize,
    ) -> Result<NominalTypeId, DefinitionError> {
        if !self.scopes.contains(scope) {
            return Err(DefinitionError::UnknownId);
        }
        Ok(self.records.reserve(scope, name, field_count))
    }

    /// Close the complete name domain before resolving types or checking definitions.
    /// # Errors
    /// Rejects unknown, reserved or colliding names without inventing missing type/header data.
    pub fn finish(self) -> Result<Builder, Vec<DeclarationViolation>> {
        let mut violations = Vec::new();
        let namespace = namespace::close(
            &self.symbols,
            &self.codes,
            &self.records,
            self.scopes,
            &mut violations,
        );
        if !violations.is_empty() {
            return Err(violations);
        }
        Ok(Builder {
            symbols: self.symbols,
            codes: self.codes,
            parameters: self.parameters.finish(),
            records: self.records,
            namespace: namespace.into(),
        })
    }
}

/// Define raw reserved headers in any order, without granting typed readiness.
/// `finish` validates and closes the complete program through the sole publication writer.
/// There is no operation for appending another declaration after reservation closure.
#[derive(Debug)]
pub struct Builder {
    symbols: Arc<SymbolTable>,
    codes: ArenaBuilder<AccessCodeId, Reservation>,
    parameters: Arena<AccessParameterId, ParameterSlot>,
    records: records::Builder,
    namespace: Arc<namespace::Names>,
}

impl Builder {
    /// Reserved input identities in authored order; this grants no typed parameter contract.
    #[must_use]
    pub fn parameter_ids(&self, id: AccessCodeId) -> Option<&[AccessParameterId]> {
        self.codes
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
        self.resolve_data_in(ScopeId::ROOT, name)
    }
    /// Resolve one header leaf through its actual source lookup scope.
    /// # Errors
    /// Rejects unknown scopes, symbols and data names without publishing a schema.
    pub fn resolve_data_in(
        &self,
        scope: ScopeId,
        name: Symbol,
    ) -> Result<DataReference, DeclarationRule> {
        data::resolve_data_name(&self.symbols, &self.namespace, scope, name)
    }

    /// # Errors
    /// Rejects a missing reservation, a mismatched reserved input count, or a second definition
    /// without replacing the first one.
    pub fn define(
        &mut self,
        id: AccessCodeId,
        kind: CodeKind,
        parameters: impl Into<Box<[ParameterDefinition]>>,
        result_data: impl Into<ValueDefinition>,
    ) -> Result<(), DefinitionError> {
        let reservation = self.codes.get_mut(id).ok_or(DefinitionError::UnknownId)?;
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
        match self.into_publication().finish() {
            Ok(program) => Ok(program),
            Err(HeaderFailure::Rejected(violations)) => Err(violations),
            Err(HeaderFailure::UnknownId | HeaderFailure::Inconsistent) => {
                unreachable!("reserved header domain lost integrity")
            }
        }
    }

    /// Open the sole publication writer over this closed domain. Definitions may still be absent.
    /// No new declaration can be reserved; each requested contract validates before publication.
    #[must_use]
    pub fn into_publication(self) -> HeaderPublication {
        HeaderPublication::new(self)
    }
}

fn validate_definition(
    symbols: &SymbolTable,
    namespace: &namespace::Names,
    id: AccessCodeId,
    reservation: &Reservation,
    records: &records::Builder,
    violations: &mut Vec<DeclarationViolation>,
) {
    let Some(definition) = &reservation.definition else {
        violations.push(DeclarationViolation {
            declaration: DeclarationId::Code(id),
            position: HeaderPosition::Name,
            rule: DeclarationRule::MissingDefinition,
        });
        return;
    };
    if reservation.name.is_none() != matches!(definition.kind, CodeKind::TypeExpression(_))
        || (matches!(definition.kind, CodeKind::TypeExpression(_))
            && definition.result_data != ValueDefinition::Data(BuiltinType::Usize.into()))
    {
        violations.push(DeclarationViolation {
            declaration: DeclarationId::Code(id),
            position: HeaderPosition::Result,
            rule: DeclarationRule::InvalidInitializerHeader,
        });
    }
    if definition.kind.is_initializer()
        && (!definition.parameters.is_empty()
            || matches!(definition.result_data, ValueDefinition::Function(_)))
    {
        violations.push(DeclarationViolation {
            declaration: DeclarationId::Code(id),
            position: HeaderPosition::Result,
            rule: DeclarationRule::InvalidInitializerHeader,
        });
    }
    validate_parameter_names(
        symbols,
        namespace,
        reservation.scope,
        id,
        definition.parameters.iter().map(|parameter| parameter.name),
        violations,
    );
    for (position, parameter) in definition.parameters.iter().enumerate() {
        validate_value(
            id,
            HeaderPosition::Parameter(position),
            &parameter.data,
            records,
            violations,
        );
    }
    validate_value(
        id,
        HeaderPosition::Result,
        &definition.result_data,
        records,
        violations,
    );
}

fn validate_parameter_names(
    symbols: &SymbolTable,
    namespace: &namespace::Names,
    scope: ScopeId,
    id: AccessCodeId,
    names: impl IntoIterator<Item = Symbol>,
    violations: &mut Vec<DeclarationViolation>,
) {
    let mut seen = BTreeMap::new();
    for (position, name) in names.into_iter().enumerate() {
        let rule = match symbols.spelling(name) {
            None => Some(DeclarationRule::UnknownSymbol),
            Some(name) if BuiltinType::from_spelling(name).is_some() => {
                Some(DeclarationRule::ReservedBuiltinName)
            }
            Some(_) => {
                if let Some(&first) = seen.get(&name) {
                    Some(DeclarationRule::DuplicateParameter { first })
                } else {
                    seen.insert(name, position);
                    if namespace.has_namespace(scope, name) {
                        Some(DeclarationRule::ParameterShadowsNamespace { name })
                    } else {
                        namespace
                            .get(scope, Namespace::Value, name)
                            .map(|declaration| match declaration {
                                DeclarationId::Code(declaration) => {
                                    DeclarationRule::ParameterShadowsValue { declaration }
                                }
                                DeclarationId::Record(_) => unreachable!("value namespace"),
                            })
                    }
                }
            }
        };
        if let Some(rule) = rule {
            violations.push(DeclarationViolation {
                declaration: DeclarationId::Code(id),
                position: HeaderPosition::Parameter(position),
                rule,
            });
        }
    }
}

fn validate_value(
    callable: AccessCodeId,
    position: HeaderPosition,
    value: &ValueDefinition,
    records: &records::Builder,
    violations: &mut Vec<DeclarationViolation>,
) {
    if value.has_repeated_failure() {
        violations.push(DeclarationViolation {
            declaration: DeclarationId::Code(callable),
            position,
            rule: DeclarationRule::RepeatedFailure,
        });
    }
    value.visit_data(|data| validate_data(callable, position, data, records, violations));
}

fn validate_data(
    callable: AccessCodeId,
    position: HeaderPosition,
    data: &DataReference,
    records: &records::Builder,
    violations: &mut Vec<DeclarationViolation>,
) {
    if let Some(rule) = data_rule(data, records) {
        violations.push(DeclarationViolation {
            declaration: DeclarationId::Code(callable),
            position,
            rule,
        });
    }
}

/// One raw-leaf validity policy for callable slots and record fields, independent of readiness.
fn data_rule(data: &DataReference, records: &records::Builder) -> Option<DeclarationRule> {
    match data {
        DataReference::Builtin(data) if !supported(*data) => {
            Some(DeclarationRule::UnsupportedDataType { data: *data })
        }
        DataReference::Record(record) if !records.contains(*record) => {
            Some(DeclarationRule::UnknownRecordType { record: *record })
        }
        _ => None,
    }
}
