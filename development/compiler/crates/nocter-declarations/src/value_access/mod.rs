//! Declaration authority for the bounded future private core.
//!
//! This product cannot be converted to a released borrow-based declaration program. Its
//! source boundary admits closed data and named/formal function witnesses. It does not flatten the
//! complete future aggregate/callable/outcome grammar into released borrow-based types.
use std::{collections::BTreeMap, sync::Arc};

use nocter_model::{
    AccessCodeId, AccessParameterId, Arena, ArenaBuilder, BuiltinType, CompileTimeGuarantee,
    GenericParameterId, NominalTypeId, SparseArena, Symbol, SymbolTable, TypeAuthority, TypeId,
    ValueInputOperation,
};

use crate::{DefinitionError, GenericParameter, GenericParameterDomain};

mod body_types;
mod closures;
pub use body_types::{
    BodyTypeError, BodyTypes, CanonicalBodyTypes, PayloadDependencies, PayloadReadiness,
    ResolvedBodyTypes,
};
pub use closures::Closure;
mod aggregate;
pub use aggregate::{AggregateForm, AggregateSchema};
mod aliases;
pub use aliases::TypeAlias;
mod code_bindings;
mod data;
mod data_instances;
pub use data_instances::{DataDomainJoinFailure, DataInstanceKey, InstanceSchema};
mod functions;
mod generic_parameters;
pub use code_bindings::{
    CodeBindingError, CodeInstanceKey, CodeTemplateKey, ConcreteWitness, InputBindingError,
};
pub use functions::{
    FunctionConstraint, FunctionDataPattern, FunctionDefinition, FunctionInput, FunctionMismatch,
    FunctionPattern, FunctionPatternMismatch, FunctionPosition, FunctionStructure, FunctionWitness,
    ValueDefinition,
};
mod namespace;
pub use namespace::{Namespace, QualifiedNameError, ScopeBindingError, ScopeId};
mod publication;
pub use publication::{HeaderFailure, HeaderPublication, HeaderStep};
mod nominals;
use data::supported;
pub use data::{ArraySchema, Component, DataReference, PayloadSchema, TupleData};
pub use nominals::{
    Enumeration, Field, FieldDefinition, NominalSchema, Record, Variant, VariantDefinition,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DeclarationId {
    Code(AccessCodeId),
    Nominal(NominalTypeId),
    Alias(nocter_model::TypeAliasId),
}
impl From<AccessCodeId> for DeclarationId {
    fn from(id: AccessCodeId) -> Self {
        Self::Code(id)
    }
}
impl From<NominalTypeId> for DeclarationId {
    fn from(id: NominalTypeId) -> Self {
        Self::Nominal(id)
    }
}
impl From<nocter_model::TypeAliasId> for DeclarationId {
    fn from(id: nocter_model::TypeAliasId) -> Self {
        Self::Alias(id)
    }
}

/// The data identity of a type-owned behavior scope.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BehaviorOwner {
    Nominal(NominalTypeId),
    Builtin(BuiltinType),
}

#[cfg(test)]
mod tests;

mod parameter_binding;
pub use parameter_binding::ParameterBinding;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParameterDefinition {
    pub binding: ParameterBinding,
    pub data: ValueDefinition,
    pub operation: ValueInputOperation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HeaderPosition {
    Name,
    Parameter(usize),
    ParameterName { input: usize, leaf: usize },
    Result,
    Field(usize),
    Variant(usize),
    VariantField { variant: usize, field: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclarationRule {
    MissingDefinition,
    InvalidInitializerHeader,
    UnknownSymbol,
    ReservedBuiltinName,
    DuplicateValue { first: AccessCodeId },
    DuplicateParameter { first: HeaderPosition },
    DuplicateCallableInputName,
    ParameterPatternMismatch,
    ParameterShadowsValue { declaration: AccessCodeId },
    ParameterShadowsNamespace { name: Symbol },
    NameConflictsWithNamespace { name: Symbol },
    InconsistentBindingAudience { name: Symbol },
    UnsupportedDataType { data: BuiltinType },
    UnsupportedCallableData,
    DuplicateRecord { first: NominalTypeId },
    DuplicateTypeName { first: DeclarationId },
    DuplicateField { first: usize },
    DuplicateVariant { first: usize },
    EmptyEnumeration,
    UnknownRecordType { record: NominalTypeId },
    UnknownDataName { name: Symbol },
    UnboundGenericParameter(GenericParameterDomain),
    UnreadyAlias { alias: nocter_model::TypeAliasId },
    QualifiedDataName { error: QualifiedNameError },
    RecursivePayload { through: NominalTypeId },
    NonCopyField,
    DestructorOnCopy,
    RepeatedFailure,
    RecursiveAlias,
    UnsupportedAliasTarget,
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
    DeferredFunction,
    /// A bodyless callable declaration; primitive authority is a separate lowering receipt.
    Primitive,
    /// A package test body selected by a test plan and absent from source lookup.
    Test(Symbol),
    /// The nominal family's implicit lifecycle body, outside the callable namespace.
    Destructor(NominalTypeId),
    ConstantInitializer,
    StaticInitializer,
    TypeExpression(TypeExpressionSource),
}
impl CodeKind {
    #[must_use]
    pub const fn is_callable(self) -> bool {
        matches!(
            self,
            Self::Function(_) | Self::DeferredFunction | Self::Primitive
        )
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
            Self::DeferredFunction | Self::Primitive | Self::Test(_) | Self::Destructor(_) => {
                CompileTimeGuarantee::RuntimeOnly
            }
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
    result_contracts: Box<[FunctionConstraint]>,
    type_contracts: std::sync::Arc<[FunctionConstraint]>,
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
    pub fn result_contracts(&self) -> &[FunctionConstraint] {
        &self.result_contracts
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
    /// Invocation result. A deferred function returns its future at this boundary.
    pub const fn result(&self) -> TypeId {
        self.result
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Parameter {
    owner: AccessCodeId,
    binding: ParameterBinding,
    position: usize,
    data: TypeId,
    operation: ValueInputOperation,
    contract: Option<FunctionConstraint>,
    components: Box<[FunctionConstraint]>,
    type_contracts: std::sync::Arc<[FunctionConstraint]>,
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
    pub const fn binding(&self) -> &ParameterBinding {
        &self.binding
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
    destructors: Arc<BTreeMap<NominalTypeId, AccessCodeId>>,
    parameters: SparseArena<AccessParameterId, Parameter>,
    generic_parameters: Arc<generic_parameters::Parameters>,
    namespace: std::sync::Arc<namespace::Names>,
    nominals: nominals::Schemas,
    aliases: SparseArena<nocter_model::TypeAliasId, TypeAlias>,
    closures: BTreeMap<nocter_model::ValueClosure, Closure>,
    closure_parameters: BTreeMap<nocter_model::ValueParameter, FunctionConstraint>,
    callable_contracts: BTreeMap<nocter_model::ValueCallableContract, FunctionConstraint>,
    applications: BTreeMap<nocter_model::ValueAppliedResult, FunctionWitness>,
}

impl Program {
    /// Reserved compile-time slots in declaration order, independently of typed readiness.
    #[must_use]
    pub fn generic_parameter_ids(&self, code: AccessCodeId) -> Option<&[GenericParameterId]> {
        self.codes
            .is_reserved(code)
            .then(|| self.generic_parameters.ids(code))
    }

    /// One compile-time slot in the closed declaration domain.
    #[must_use]
    pub fn generic_parameter(
        &self,
        id: GenericParameterId,
    ) -> Option<GenericParameter<AccessCodeId>> {
        self.generic_parameters.get(id)
    }

    /// Resolve a binder in exactly this callable's own generic scope.
    #[must_use]
    pub fn generic_parameter_named(
        &self,
        code: AccessCodeId,
        name: Symbol,
    ) -> Option<(GenericParameterId, GenericParameter<AccessCodeId>)> {
        self.generic_parameters.named(code, name)
    }

    /// The original execution mode and, for a deferred callable, its body output.
    #[must_use]
    pub fn code_execution(&self, id: AccessCodeId) -> Option<crate::CallableExecution> {
        let code = self.direct_callable(id)?;
        if code.kind() == CodeKind::DeferredFunction {
            Some(crate::CallableExecution::Deferred {
                output: self.code_body_result(id)?,
            })
        } else {
            Some(crate::CallableExecution::Immediate)
        }
    }
    fn direct_callable(&self, id: AccessCodeId) -> Option<&Code> {
        let code = self.codes.get(id)?;
        (code.kind().is_callable() && self.generic_parameter_ids(id)?.is_empty()).then_some(code)
    }
    /// Execution for an exact function origin. Symbolic result, contract and formal origins
    /// carry a type constraint and cannot certify which body or constructor is selected.
    #[must_use]
    pub fn function_execution(
        &self,
        origin: nocter_model::ValueFunctionOrigin,
    ) -> Option<crate::CallableExecution> {
        use nocter_model::ValueFunctionOrigin;
        match origin {
            ValueFunctionOrigin::Declaration(id) => self.code_execution(id),
            ValueFunctionOrigin::Closure(id) => self
                .closure(id)
                .map(|_| crate::CallableExecution::Immediate),
            ValueFunctionOrigin::Parameter(_)
            | ValueFunctionOrigin::Contract(_)
            | ValueFunctionOrigin::Result(_)
            | ValueFunctionOrigin::Application(_) => None,
        }
    }
    /// The result checked inside a body or initializer, before wrapping any deferred invocation.
    #[must_use]
    pub fn code_body_result(&self, id: AccessCodeId) -> Option<TypeId> {
        let code = self.codes.get(id)?;
        if code.kind() != CodeKind::DeferredFunction {
            return Some(code.result());
        }
        match self.data_schema(code.result())? {
            PayloadSchema::Future(output) => Some(output),
            _ => None,
        }
    }
    /// Validate names in one physical header scope without publishing typed inputs.
    /// # Errors
    /// Rejects an unknown scope or named declaration identity.
    pub fn parameter_name_violations_in<'binding>(
        &self,
        scope: ScopeId,
        code: AccessCodeId,
        bindings: impl IntoIterator<Item = &'binding ParameterBinding>,
    ) -> Result<Vec<DeclarationViolation>, DefinitionError> {
        if !self.has_scope(scope) || !self.codes.is_reserved(code) {
            return Err(DefinitionError::UnknownId);
        }
        let mut violations = Vec::new();
        validate_parameter_names(
            &self.symbols,
            &self.namespace,
            scope,
            code,
            bindings,
            &mut violations,
        );
        Ok(violations)
    }

    #[must_use]
    pub const fn headers_complete(&self) -> bool {
        self.codes.is_complete()
            && self.parameters.is_complete()
            && self.nominals.types.is_complete()
            && self.nominals.fields.is_complete()
            && self.nominals.variants.is_complete()
            && self.aliases.is_complete()
    }
    /// A complete published contract, including its established transitive record schemas.
    #[must_use]
    pub fn header_ready(&self, declaration: impl Into<DeclarationId>) -> bool {
        match declaration.into() {
            DeclarationId::Code(id) => self.codes.get(id).is_some(),
            DeclarationId::Nominal(id) => self.nominals.types.get(id).is_some(),
            DeclarationId::Alias(id) => self.aliases.get(id).is_some(),
        }
    }
    /// Retains every established contract and its exact type prefix from the same construction.
    #[must_use]
    pub fn preserves_headers(&self, older: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.namespace, &older.namespace)
            && self.nominals.types.preserves(&older.nominals.types)
            && self.nominals.fields.preserves(&older.nominals.fields)
            && self.nominals.variants.preserves(&older.nominals.variants)
            && self.codes.preserves(&older.codes)
            && self.parameters.preserves(&older.parameters)
            && self.aliases.preserves(&older.aliases)
            && self.header_prefix.preserves_prefix(&older.header_prefix)
    }
    #[must_use]
    pub fn symbols(&self) -> &SymbolTable {
        &self.symbols
    }
    #[must_use]
    pub fn nominals(&self) -> &SparseArena<NominalTypeId, NominalSchema> {
        &self.nominals.types
    }
    #[must_use]
    pub fn record(&self, id: NominalTypeId) -> Option<&Record> {
        match self.nominals().get(id)? {
            NominalSchema::Record(record) => Some(record),
            NominalSchema::Enumeration(_) => None,
        }
    }
    #[must_use]
    pub fn enumeration(&self, id: NominalTypeId) -> Option<&Enumeration> {
        match self.nominals().get(id)? {
            NominalSchema::Record(_) => None,
            NominalSchema::Enumeration(schema) => Some(schema),
        }
    }
    #[must_use]
    pub fn variants(&self) -> &SparseArena<nocter_model::VariantId, Variant> {
        &self.nominals.variants
    }
    #[must_use]
    pub fn fields(&self) -> &SparseArena<nocter_model::AccessFieldId, Field> {
        &self.nominals.fields
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
    /// Finds a declaration authored in this exact scope, independently of visible aliases.
    #[must_use]
    pub fn authored_declaration_in(
        &self,
        scope: ScopeId,
        namespace: Namespace,
        name: Symbol,
    ) -> Option<DeclarationId> {
        self.namespace.authored(scope, namespace, name)
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
        self.namespace.lookup_path(scope, namespace, path, None)
    }
    /// Resolve the same authored path, requiring each selected binding to cover the audience.
    /// This checks path access; canonical declarations retain their own exposure rules.
    /// # Errors
    /// Reports the first failed path position, including a narrower binding at any component.
    pub fn declaration_path_for_audience(
        &self,
        scope: ScopeId,
        namespace: Namespace,
        path: &[Option<Symbol>],
        audience: &nocter_model::ModuleAudience,
    ) -> Result<DeclarationId, QualifiedNameError> {
        self.namespace
            .lookup_path(scope, namespace, path, Some(audience))
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
    pub fn resolve_nominal(&self, name: Symbol) -> Option<NominalTypeId> {
        self.resolve_nominal_in(ScopeId::ROOT, name)
    }
    #[must_use]
    pub fn resolve_nominal_in(&self, scope: ScopeId, name: Symbol) -> Option<NominalTypeId> {
        match self.declaration_in(scope, Namespace::Type, name)? {
            DeclarationId::Nominal(id) => Some(id),
            DeclarationId::Code(_) | DeclarationId::Alias(_) => None,
        }
    }
    #[must_use]
    pub fn codes(&self) -> &SparseArena<AccessCodeId, Code> {
        &self.codes
    }
    /// The family's published destructor declaration. Body and cleanup admission are separate.
    #[must_use]
    pub fn destructor(&self, owner: NominalTypeId) -> Option<AccessCodeId> {
        let id = self.reserved_destructor(owner)?;
        self.codes.get(id).map(|_| id)
    }
    /// A lifecycle obligation in the closed name domain, independent of header/body readiness.
    #[must_use]
    pub fn reserved_destructor(&self, owner: NominalTypeId) -> Option<AccessCodeId> {
        self.destructors.get(&owner).copied()
    }
    #[must_use]
    pub fn has_destructors(&self) -> bool {
        !self.destructors.is_empty()
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
            DeclarationId::Nominal(_) | DeclarationId::Alias(_) => None,
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
            DeclarationId::Nominal(id) if self.nominals().get(id).is_some() => {
                return self
                    .symbols
                    .spelling(self.nominals().get(id)?.name())
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
        match code.kind() {
            CodeKind::Test(name) => self.symbols.spelling(name).map(str::to_owned),
            CodeKind::Destructor(owner) => Some(format!(
                "drop for {}",
                self.declaration_label(owner.into())?
            )),
            CodeKind::TypeExpression(source) => Some(format!(
                "type expression in {}",
                self.declaration_label(source.declaration)?
            )),
            _ => None,
        }
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
    unqualified: bool,
    parameters: Box<[AccessParameterId]>,
    definition: Option<Definition>,
}

impl Reservation {
    fn source_name(&self) -> Option<Symbol> {
        self.name.filter(|_| self.unqualified)
    }
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
    destructors: BTreeMap<NominalTypeId, AccessCodeId>,
    parameters: ArenaBuilder<AccessParameterId, ParameterSlot>,
    generic_parameters: generic_parameters::Reservations,
    nominals: nominals::Builder,
    aliases: ArenaBuilder<nocter_model::TypeAliasId, aliases::Slot>,
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
            destructors: BTreeMap::new(),
            parameters: ArenaBuilder::new(),
            generic_parameters: generic_parameters::Reservations::default(),
            nominals: nominals::Builder::default(),
            aliases: ArenaBuilder::new(),
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
    /// Reserve a lexical scope with a fixed meaning for `Self`.
    /// # Errors
    /// Rejects an unknown parent or owner before reserving the scope.
    pub fn new_behavior_scope(
        &mut self,
        parent: ScopeId,
        owner: BehaviorOwner,
    ) -> Result<ScopeId, DefinitionError> {
        let known = match owner {
            BehaviorOwner::Nominal(record) => self.nominals.scope(record).is_some(),
            BehaviorOwner::Builtin(builtin) => supported(builtin),
        };
        if !known {
            return Err(DefinitionError::UnknownId);
        }
        let scope = self
            .scopes
            .new_lexical_scope(parent)
            .map_err(|_| DefinitionError::UnknownId)?;
        self.scopes.behavior_owners.insert(scope, owner);
        Ok(scope)
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
            DeclarationId::Code(id) => self
                .codes
                .get(id)
                .and_then(Reservation::source_name)
                .is_some(),
            DeclarationId::Nominal(id) => self.nominals.contains(id),
            DeclarationId::Alias(id) => self.aliases.get(id).is_some(),
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
        self.reserve_known_scope(ScopeId::ROOT, name, parameter_count, true)
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
        Ok(self.reserve_known_scope(scope, name, parameter_count, true))
    }

    /// Reserve code selected only through a type-owned member identity.
    /// # Errors
    /// Rejects an unknown scope or spelling before allocating any code or parameter slots.
    pub fn reserve_type_member(
        &mut self,
        scope: ScopeId,
        name: Symbol,
        parameter_count: usize,
    ) -> Result<AccessCodeId, DefinitionError> {
        if !self.scopes.contains(scope) || self.symbols.spelling(name).is_none() {
            return Err(DefinitionError::UnknownId);
        }
        Ok(self.reserve_known_scope(scope, name, parameter_count, false))
    }

    fn reserve_known_scope(
        &mut self,
        scope: ScopeId,
        name: Symbol,
        parameter_count: usize,
        unqualified: bool,
    ) -> AccessCodeId {
        let owner = self.codes.next_id();
        let parameters = (0..parameter_count)
            .map(|position| self.parameters.insert(ParameterSlot { owner, position }))
            .collect();
        self.codes.insert(Reservation {
            scope,
            name: Some(name),
            unqualified,
            parameters,
            definition: None,
        })
    }

    /// Reserve a named callable's authored compile-time slots before closing the namespace.
    /// # Errors
    /// Rejects unknown code or names, duplicate names, and a second list without reserving slots.
    pub fn reserve_callable_generics(
        &mut self,
        code: AccessCodeId,
        parameters: Vec<(Symbol, GenericParameterDomain)>,
    ) -> Result<(), DefinitionError> {
        if self.codes.get(code).is_none_or(|slot| slot.name.is_none())
            || parameters
                .iter()
                .any(|(name, _)| self.symbols.spelling(*name).is_none())
        {
            return Err(DefinitionError::UnknownId);
        }
        self.generic_parameters.reserve(code, parameters)
    }

    /// Whether a spelling names a binder of this reserved callable, before header publication.
    #[must_use]
    pub fn has_callable_generic_parameter(&self, code: AccessCodeId, spelling: &str) -> bool {
        self.symbols
            .get(spelling)
            .and_then(|name| self.generic_parameters.named(code, name))
            .is_some()
    }

    /// Reserve a test root in the authored source scope with its implicit `void!` result.
    /// # Errors
    /// Rejects an unknown scope or spelling before reserving code.
    pub fn reserve_test(
        &mut self,
        scope: ScopeId,
        name: Symbol,
    ) -> Result<AccessCodeId, DefinitionError> {
        if !self.scopes.contains(scope) || self.symbols.spelling(name).is_none() {
            return Err(DefinitionError::UnknownId);
        }
        Ok(self.codes.insert(Reservation {
            scope,
            name: None,
            unqualified: false,
            parameters: Box::new([]),
            definition: Some(Definition {
                kind: CodeKind::Test(name),
                parameters: Box::new([]),
                result_data: test_result(),
            }),
        }))
    }
    /// Reserve the one anonymous lifecycle body of a nominal family in its behavior scope.
    /// The input is implicit complete `edit self` and the result is unit.
    ///
    /// # Errors
    /// Rejects an unknown receiver spelling, a mismatched scope or a second destructor.
    pub fn reserve_destructor(
        &mut self,
        scope: ScopeId,
        owner: NominalTypeId,
    ) -> Result<AccessCodeId, DefinitionError> {
        if self.scopes.behavior_owners.get(&scope) != Some(&BehaviorOwner::Nominal(owner)) {
            return Err(DefinitionError::UnknownId);
        }
        let receiver = self.symbols.get("self").ok_or(DefinitionError::UnknownId)?;
        let std::collections::btree_map::Entry::Vacant(entry) = self.destructors.entry(owner)
        else {
            return Err(DefinitionError::AlreadyDefined);
        };
        let parameter = self.parameters.insert(ParameterSlot {
            owner: self.codes.next_id(),
            position: 0,
        });
        let code = self.codes.insert(Reservation {
            scope,
            name: None,
            unqualified: false,
            parameters: Box::new([parameter]),
            definition: Some(Definition {
                kind: CodeKind::Destructor(owner),
                parameters: Box::new([ParameterDefinition {
                    binding: ParameterBinding::Name(receiver),
                    data: ValueDefinition::Data(DataReference::Nominal(owner)),
                    operation: ValueInputOperation::Edit,
                }]),
                result_data: ValueDefinition::Data(BuiltinType::Void.into()),
            }),
        });
        entry.insert(code);
        Ok(code)
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
            DeclarationId::Nominal(id) => {
                self.nominals.scope(id).ok_or(DefinitionError::UnknownId)?
            }
            DeclarationId::Alias(id) => {
                self.aliases
                    .get(id)
                    .ok_or(DefinitionError::UnknownId)?
                    .scope
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
                code.name.is_some()
                    && match source.position {
                        HeaderPosition::Parameter(index) => index < code.parameters.len(),
                        HeaderPosition::Result => true,
                        _ => false,
                    }
            }),
            DeclarationId::Nominal(id) => self.nominals.field_at(id, source.position).is_some(),
            DeclarationId::Alias(id) => {
                self.aliases.get(id).is_some() && source.position == HeaderPosition::Result
            }
        };
        if !valid {
            return Err(DefinitionError::UnknownId);
        }
        Ok(self.codes.insert(Reservation {
            scope,
            name: None,
            unqualified: false,
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
        self.nominals.reserve(ScopeId::ROOT, name, field_count)
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
        Ok(self.nominals.reserve(scope, name, field_count))
    }

    /// Reserve a nominal identity and the declaration-order variant and payload field identities.
    /// # Errors
    /// Rejects an unknown scope before reserving slots.
    pub fn reserve_enum_in(
        &mut self,
        scope: ScopeId,
        name: Symbol,
        payload_counts: impl IntoIterator<Item = usize>,
    ) -> Result<NominalTypeId, DefinitionError> {
        if !self.scopes.contains(scope) {
            return Err(DefinitionError::UnknownId);
        }
        Ok(self.nominals.reserve_enum(scope, name, payload_counts))
    }

    /// Reserve a type name that will resolve to an existing data identity.
    /// # Errors
    /// Rejects a scope outside this construction.
    pub fn reserve_alias(
        &mut self,
        scope: ScopeId,
        name: Symbol,
    ) -> Result<nocter_model::TypeAliasId, DefinitionError> {
        if !self.scopes.contains(scope) {
            return Err(DefinitionError::UnknownId);
        }
        Ok(self.aliases.insert(aliases::Slot::new(scope, name)))
    }

    /// Close the complete name domain before resolving types or checking definitions.
    /// # Errors
    /// Rejects unknown, reserved or colliding names without inventing missing type/header data.
    pub fn finish(self) -> Result<Builder, Vec<DeclarationViolation>> {
        let mut violations = Vec::new();
        let namespace = namespace::close(
            &self.symbols,
            &self.codes,
            &self.nominals,
            &self.aliases,
            self.scopes,
            &mut violations,
        );
        if !violations.is_empty() {
            return Err(violations);
        }
        Ok(Builder {
            symbols: self.symbols,
            codes: self.codes,
            destructors: Arc::new(self.destructors),
            parameters: self.parameters.finish(),
            generic_parameters: Arc::new(self.generic_parameters.finish()),
            nominals: self.nominals,
            aliases: self.aliases,
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
    destructors: Arc<BTreeMap<NominalTypeId, AccessCodeId>>,
    parameters: Arena<AccessParameterId, ParameterSlot>,
    generic_parameters: Arc<generic_parameters::Parameters>,
    nominals: nominals::Builder,
    aliases: ArenaBuilder<nocter_model::TypeAliasId, aliases::Slot>,
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
        self.nominals.field_ids(id)
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
        self.nominals.define(id, copy, fields.into())
    }

    /// Define the variants and their fields in declaration order.
    /// # Errors
    /// Rejects a wrong reservation kind, arity mismatch or a second definition.
    pub fn define_enum(
        &mut self,
        id: NominalTypeId,
        copyable: bool,
        variants: impl Into<Box<[VariantDefinition]>>,
    ) -> Result<(), DefinitionError> {
        self.nominals.define_enum(id, copyable, variants.into())
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
    /// Rejects a missing reservation, a mismatched reserved input count, generic slots or a
    /// second definition without replacing the first one.
    pub fn define(
        &mut self,
        id: AccessCodeId,
        kind: CodeKind,
        parameters: impl Into<Box<[ParameterDefinition]>>,
        result_data: impl Into<ValueDefinition>,
    ) -> Result<(), DefinitionError> {
        if !self.generic_parameters.ids(id).is_empty() {
            return Err(DefinitionError::RequiresGenericDefinition);
        }
        self.define_reserved(id, kind, parameters.into(), result_data.into())
    }

    fn define_reserved(
        &mut self,
        id: AccessCodeId,
        kind: CodeKind,
        parameters: Box<[ParameterDefinition]>,
        result_data: ValueDefinition,
    ) -> Result<(), DefinitionError> {
        let reservation = self.codes.get_mut(id).ok_or(DefinitionError::UnknownId)?;
        if reservation.definition.is_some() {
            return Err(DefinitionError::AlreadyDefined);
        }
        if parameters.len() != reservation.parameters.len() {
            return Err(DefinitionError::WrongArity {
                expected: reservation.parameters.len(),
                actual: parameters.len(),
            });
        }
        reservation.definition = Some(Definition {
            kind,
            parameters,
            result_data,
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

fn test_result() -> ValueDefinition {
    ValueDefinition::Data(DataReference::Fallible(std::sync::Arc::new(
        BuiltinType::Void.into(),
    )))
}

fn validate_definition(
    definitions: &Builder,
    id: AccessCodeId,
    reservation: &Reservation,
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
    let anonymous = matches!(
        definition.kind,
        CodeKind::TypeExpression(_) | CodeKind::Test(_) | CodeKind::Destructor(_)
    );
    if reservation.name.is_none() != anonymous
        || (matches!(definition.kind, CodeKind::TypeExpression(_))
            && definition.result_data != ValueDefinition::Data(BuiltinType::Usize.into()))
        || (matches!(definition.kind, CodeKind::Test(_))
            && (!definition.parameters.is_empty() || definition.result_data != test_result()))
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
        &definitions.symbols,
        &definitions.namespace,
        reservation.scope,
        id,
        definition
            .parameters
            .iter()
            .map(|parameter| &parameter.binding),
        violations,
    );
    for (position, parameter) in definition.parameters.iter().enumerate() {
        let position = HeaderPosition::Parameter(position);
        if !parameter.binding.matches_data(&parameter.data) {
            violations.push(DeclarationViolation {
                declaration: DeclarationId::Code(id),
                position,
                rule: DeclarationRule::ParameterPatternMismatch,
            });
        }
        validate_value(id, position, &parameter.data, true, definitions, violations);
    }
    validate_value(
        id,
        HeaderPosition::Result,
        &definition.result_data,
        definition.kind.is_callable(),
        definitions,
        violations,
    );
}

fn validate_parameter_names<'binding>(
    symbols: &SymbolTable,
    namespace: &namespace::Names,
    scope: ScopeId,
    id: AccessCodeId,
    bindings: impl IntoIterator<Item = &'binding ParameterBinding>,
    violations: &mut Vec<DeclarationViolation>,
) {
    let mut seen = BTreeMap::new();
    let names = bindings
        .into_iter()
        .enumerate()
        .flat_map(|(input, binding)| {
            binding
                .names()
                .enumerate()
                .map(move |(leaf, &name)| (HeaderPosition::ParameterName { input, leaf }, name))
        });
    for (position, name) in names {
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
                                DeclarationId::Nominal(_) | DeclarationId::Alias(_) => {
                                    unreachable!("value namespace")
                                }
                            })
                    }
                }
            }
        };
        if let Some(rule) = rule {
            violations.push(DeclarationViolation {
                declaration: DeclarationId::Code(id),
                position,
                rule,
            });
        }
    }
}

fn validate_value(
    callable: AccessCodeId,
    position: HeaderPosition,
    value: &ValueDefinition,
    allow_components: bool,
    definitions: &Builder,
    violations: &mut Vec<DeclarationViolation>,
) {
    if value.has_repeated_failure() {
        violations.push(DeclarationViolation {
            declaration: DeclarationId::Code(callable),
            position,
            rule: DeclarationRule::RepeatedFailure,
        });
    }
    value.visit_data(|data| {
        if matches!(data, DataReference::Function(_))
            && allow_components
            && !matches!(value, ValueDefinition::Data(DataReference::Function(_)))
        {
            return;
        }
        if let Some((id, expected)) = data.generic_slot()
            && definitions
                .generic_parameters
                .get(id)
                .is_some_and(|slot| slot.owner() == callable && slot.domain() == expected)
        {
            return;
        }
        if let Some(rule) = data_rule(data, &definitions.nominals) {
            violations.push(DeclarationViolation {
                declaration: DeclarationId::Code(callable),
                position,
                rule,
            });
        }
    });
}

/// One raw-leaf validity policy for callable slots and record fields, independent of readiness.
fn data_rule(data: &DataReference, nominals: &nominals::Builder) -> Option<DeclarationRule> {
    if let Some((_, expected)) = data.generic_slot() {
        return Some(DeclarationRule::UnboundGenericParameter(expected));
    }
    match data {
        DataReference::Function(_) => Some(DeclarationRule::UnsupportedCallableData),
        DataReference::Builtin(data) if !supported(*data) => {
            Some(DeclarationRule::UnsupportedDataType { data: *data })
        }
        DataReference::Nominal(record) if !nominals.contains(*record) => {
            Some(DeclarationRule::UnknownRecordType { record: *record })
        }
        _ => None,
    }
}
