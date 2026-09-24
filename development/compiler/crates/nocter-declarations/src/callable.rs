use std::fmt;

use nocter_model::{
    ArgumentPackType, BodyId, BorrowCapability, CallableCapability, CallableId, CompilationTarget,
    ConstantId, ConstructionId, DeclarationSiteId, DropId, GenericParameterId, InstanceId,
    InterfaceId, ModuleId, ParameterId, RequirementId, StaticId, Symbol, TestId, TypeId, TypeKind,
    TypeStore, VariantId,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LiteralShape {
    Sequence,
    Mapping,
    String,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CallableKind {
    Function,
    Primitive,
    Method,
    ConstructionFunction,
    Literal(LiteralShape),
    Coercion,
    Equality,
    Ordering,
    Index,
    Expansion,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CallableOwner {
    Module(ModuleId),
    Construction(ConstructionId),
    Instance(InstanceId),
    Interface(InterfaceId),
}

/// Execution mode fixed once from a callable's declaration modifier.
///
/// A deferred body produces `output`; invocation itself produces the declaration's outer
/// `future output` value. Result shape, aliases, and generic substitution never change this fact.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CallableExecution {
    Immediate,
    Deferred { output: TypeId },
}

/// One source-independent incompatibility between a callable's kind, execution, and guarantees.
///
/// Declaration consumers use this closed policy instead of independently interpreting modifier
/// combinations. Source diagnostics remain the responsibility of the checking boundary that owns
/// current source projection.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CallableContractViolation {
    DeferredNoAllocation,
    DeferredBlocking,
    DeferredCompileTime,
    PrimitiveCompileTime,
}

const fn callable_contract_violation(
    kind: CallableKind,
    deferred: bool,
    guarantees: nocter_model::CallableGuarantees,
) -> Option<CallableContractViolation> {
    use nocter_language::CallableModifier;
    use nocter_model::{AllocationGuarantee, CompileTimeGuarantee, NonblockingGuarantee};

    if deferred {
        let conflicts = [
            (
                CallableModifier::NoAllocation,
                matches!(guarantees.allocation(), AllocationGuarantee::NoAllocation),
                CallableContractViolation::DeferredNoAllocation,
            ),
            (
                CallableModifier::Blocking,
                matches!(guarantees.nonblocking(), NonblockingGuarantee::Unspecified),
                CallableContractViolation::DeferredBlocking,
            ),
            (
                CallableModifier::CompileTime,
                matches!(guarantees.compile_time(), CompileTimeGuarantee::Evaluatable),
                CallableContractViolation::DeferredCompileTime,
            ),
        ];
        let mut index = 0;
        while index < conflicts.len() {
            let (modifier, present, violation) = conflicts[index];
            if present && !CallableModifier::Async.is_compatible_with(modifier) {
                return Some(violation);
            }
            index += 1;
        }
    }
    if matches!(kind, CallableKind::Primitive)
        && matches!(guarantees.compile_time(), CompileTimeGuarantee::Evaluatable)
    {
        return Some(CallableContractViolation::PrimitiveCompileTime);
    }
    None
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProvenanceOrigin {
    Receiver,
    Parameter(ParameterId),
}

/// Caller-managed origins retained by one declaration result.
///
/// This is distinct from structural callable-type provenance. A method may name its receiver,
/// while ordinary callable contracts normalize only their explicit parameter positions.
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct CallableProvenance(Box<[ProvenanceOrigin]>);

impl CallableProvenance {
    #[must_use]
    pub fn empty() -> Self {
        Self(Box::new([]))
    }

    /// Creates a sorted, unique declaration-origin set.
    ///
    /// # Errors
    ///
    /// Returns [`DuplicateCallableOrigin`] when an origin occurs more than once.
    pub fn from_origins(
        origins: impl IntoIterator<Item = ProvenanceOrigin>,
    ) -> Result<Self, DuplicateCallableOrigin> {
        let mut origins: Vec<_> = origins.into_iter().collect();
        origins.sort_unstable();
        if let Some(duplicate) = origins
            .windows(2)
            .find(|pair| pair[0] == pair[1])
            .map(|pair| pair[0])
        {
            return Err(DuplicateCallableOrigin(duplicate));
        }
        Ok(Self(origins.into_boxed_slice()))
    }

    #[must_use]
    pub const fn origins(&self) -> &[ProvenanceOrigin] {
        &self.0
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct DuplicateCallableOrigin(ProvenanceOrigin);

impl DuplicateCallableOrigin {
    #[must_use]
    pub const fn origin(self) -> ProvenanceOrigin {
        self.0
    }
}

impl fmt::Debug for DuplicateCallableOrigin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("DuplicateCallableOrigin")
            .field(&self.0)
            .finish()
    }
}

impl fmt::Display for DuplicateCallableOrigin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("callable result origin occurs more than once")
    }
}

impl std::error::Error for DuplicateCallableOrigin {}

/// One callable input whose value provenance is bounded by other callable inputs.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CallableInputConstraint {
    target: ParameterId,
    sources: CallableProvenance,
}

impl CallableInputConstraint {
    #[must_use]
    pub const fn new(target: ParameterId, sources: CallableProvenance) -> Self {
        Self { target, sources }
    }

    #[must_use]
    pub const fn target(&self) -> ParameterId {
        self.target
    }

    #[must_use]
    pub const fn sources(&self) -> &CallableProvenance {
        &self.sources
    }
}

/// Canonical input-provenance constraints for one callable declaration.
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct CallableInputProvenance(Box<[CallableInputConstraint]>);

impl CallableInputProvenance {
    #[must_use]
    pub fn empty() -> Self {
        Self(Box::new([]))
    }

    /// Creates a target-sorted constraint set.
    ///
    /// # Errors
    ///
    /// Each receiver or parameter may own at most one input constraint.
    pub fn from_constraints(
        constraints: impl IntoIterator<Item = CallableInputConstraint>,
    ) -> Result<Self, DuplicateInputConstraint> {
        let mut constraints: Vec<_> = constraints.into_iter().collect();
        constraints.sort_unstable_by_key(CallableInputConstraint::target);
        if let Some(target) = constraints
            .windows(2)
            .find(|pair| pair[0].target() == pair[1].target())
            .map(|pair| pair[0].target())
        {
            return Err(DuplicateInputConstraint(target));
        }
        Ok(Self(constraints.into_boxed_slice()))
    }

    #[must_use]
    pub const fn constraints(&self) -> &[CallableInputConstraint] {
        &self.0
    }

    #[must_use]
    pub fn sources(&self, target: ParameterId) -> Option<&CallableProvenance> {
        self.0
            .binary_search_by_key(&target, CallableInputConstraint::target)
            .ok()
            .map(|index| self.0[index].sources())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DuplicateInputConstraint(ParameterId);

impl DuplicateInputConstraint {
    #[must_use]
    pub const fn target(self) -> ParameterId {
        self.0
    }
}

impl fmt::Display for DuplicateInputConstraint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("callable input has more than one provenance constraint")
    }
}

impl std::error::Error for DuplicateInputConstraint {}

/// The source-level provenance contract retained before body checking.
///
/// A declared contract is already an exact caller-visible upper bound. An inferred contract must
/// remain unresolved until a source body or trusted primitive definition produces its checked
/// provenance summary. Keeping that distinction here prevents declaration lowering from guessing
/// body semantics.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum CallableProvenanceContract {
    Inferred,
    Declared(CallableProvenance),
}

impl CallableProvenanceContract {
    #[must_use]
    pub const fn inferred() -> Self {
        Self::Inferred
    }

    #[must_use]
    pub const fn declared(provenance: CallableProvenance) -> Self {
        Self::Declared(provenance)
    }

    #[must_use]
    pub const fn declared_origins(&self) -> Option<&[ProvenanceOrigin]> {
        match self {
            Self::Inferred => None,
            Self::Declared(provenance) => Some(provenance.origins()),
        }
    }
}

/// Whether a callable's result provenance was authored in its public signature.
///
/// In addition to preserving source-facing presentation, an explicit contract authorizes an
/// abstract associated result to retain an invocation-place loan. Without that fact a lending
/// interface would lose its receiver relation before its associated item is specialized. A plain
/// generic result remains fixed independently of the invocation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ProvenanceAnnotation {
    Elided,
    Explicit { includes_static: bool },
}

/// One callable contract after header resolution and before body checking.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableDeclaration {
    site: DeclarationSiteId,
    owner: CallableOwner,
    kind: CallableKind,
    name: Option<Symbol>,
    receiver: Option<ParameterId>,
    generic_parameters: Box<[GenericParameterId]>,
    parameters: Box<[ParameterId]>,
    result: TypeId,
    execution: CallableExecution,
    guarantees: nocter_model::CallableGuarantees,
    input_provenance: CallableInputProvenance,
    provenance: CallableProvenanceContract,
    provenance_annotation: ProvenanceAnnotation,
    requirements: Box<[RequirementId]>,
    body: Option<BodyId>,
    target_gate: Option<CompilationTarget>,
}

impl CallableDeclaration {
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        site: DeclarationSiteId,
        owner: CallableOwner,
        kind: CallableKind,
        name: Option<Symbol>,
        receiver: Option<ParameterId>,
        generic_parameters: impl Into<Box<[GenericParameterId]>>,
        parameters: impl Into<Box<[ParameterId]>>,
        result: TypeId,
        execution: CallableExecution,
        guarantees: nocter_model::CallableGuarantees,
        input_provenance: CallableInputProvenance,
        provenance: CallableProvenanceContract,
        provenance_annotation: ProvenanceAnnotation,
        requirements: impl Into<Box<[RequirementId]>>,
        body: Option<BodyId>,
        target_gate: Option<CompilationTarget>,
    ) -> Self {
        Self {
            site,
            owner,
            kind,
            name,
            receiver,
            generic_parameters: generic_parameters.into(),
            parameters: parameters.into(),
            result,
            execution,
            guarantees,
            input_provenance,
            provenance,
            provenance_annotation,
            requirements: requirements.into(),
            body,
            target_gate,
        }
    }

    #[must_use]
    pub const fn site(&self) -> DeclarationSiteId {
        self.site
    }

    #[must_use]
    pub const fn owner(&self) -> CallableOwner {
        self.owner
    }

    #[must_use]
    pub const fn kind(&self) -> CallableKind {
        self.kind
    }

    #[must_use]
    pub const fn name(&self) -> Option<Symbol> {
        self.name
    }

    #[must_use]
    pub const fn receiver(&self) -> Option<ParameterId> {
        self.receiver
    }

    #[must_use]
    pub const fn generic_parameters(&self) -> &[GenericParameterId] {
        &self.generic_parameters
    }

    #[must_use]
    pub const fn parameters(&self) -> &[ParameterId] {
        &self.parameters
    }

    /// Returns the type produced by invoking this callable.
    ///
    /// For deferred execution this is `future Output`; use [`Self::body_result`] when checking or
    /// presenting the authored body contract.
    #[must_use]
    pub const fn result(&self) -> TypeId {
        self.result
    }

    /// Returns the type produced by the callable body.
    #[must_use]
    pub const fn body_result(&self) -> TypeId {
        match self.execution {
            CallableExecution::Immediate => self.result,
            CallableExecution::Deferred { output } => output,
        }
    }

    #[must_use]
    pub const fn execution(&self) -> CallableExecution {
        self.execution
    }

    #[must_use]
    pub const fn guarantees(&self) -> nocter_model::CallableGuarantees {
        self.guarantees
    }

    /// Returns the first incompatibility in the declaration's canonical modifier-policy order.
    #[must_use]
    pub const fn contract_violation(&self) -> Option<CallableContractViolation> {
        callable_contract_violation(
            self.kind,
            matches!(self.execution, CallableExecution::Deferred { .. }),
            self.guarantees,
        )
    }

    #[must_use]
    pub const fn input_provenance(&self) -> &CallableInputProvenance {
        &self.input_provenance
    }

    #[must_use]
    pub const fn provenance(&self) -> &CallableProvenanceContract {
        &self.provenance
    }

    #[must_use]
    pub const fn provenance_annotation(&self) -> ProvenanceAnnotation {
        self.provenance_annotation
    }

    #[must_use]
    pub const fn requirements(&self) -> &[RequirementId] {
        &self.requirements
    }

    #[must_use]
    pub const fn body(&self) -> Option<BodyId> {
        self.body
    }

    #[must_use]
    pub const fn target_gate(&self) -> Option<CompilationTarget> {
        self.target_gate
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ParameterOwner {
    Callable(CallableId),
    Variant(VariantId),
    Drop(DropId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ParameterRole {
    Ordinary { position: usize },
    ArgumentPack { position: usize },
    Receiver(CallableCapability),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Parameter {
    owner: ParameterOwner,
    name: Symbol,
    shape: ParameterShape,
    role: ParameterRole,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ParameterShape {
    Value(TypeId),
    ArgumentPack(ArgumentPackType),
}

impl Parameter {
    #[must_use]
    pub const fn new(owner: ParameterOwner, name: Symbol, ty: TypeId, role: ParameterRole) -> Self {
        Self {
            owner,
            name,
            shape: match role {
                ParameterRole::ArgumentPack { .. } => {
                    ParameterShape::ArgumentPack(ArgumentPackType::Values(ty))
                }
                ParameterRole::Ordinary { .. } | ParameterRole::Receiver(_) => {
                    ParameterShape::Value(ty)
                }
            },
            role,
        }
    }

    #[must_use]
    pub const fn new_argument_pack(
        owner: ParameterOwner,
        name: Symbol,
        position: usize,
        pack: ArgumentPackType,
    ) -> Self {
        Self {
            owner,
            name,
            shape: ParameterShape::ArgumentPack(pack),
            role: ParameterRole::ArgumentPack { position },
        }
    }

    #[must_use]
    pub const fn owner(self) -> ParameterOwner {
        self.owner
    }

    #[must_use]
    pub const fn name(self) -> Symbol {
        self.name
    }

    #[must_use]
    pub const fn ty(self) -> TypeId {
        match self.shape {
            ParameterShape::Value(ty) => ty,
            ParameterShape::ArgumentPack(pack) => pack.primary(),
        }
    }

    #[must_use]
    pub const fn argument_pack(self) -> Option<ArgumentPackType> {
        match self.shape {
            ParameterShape::Value(_) => None,
            ParameterShape::ArgumentPack(pack) => Some(pack),
        }
    }

    #[must_use]
    pub const fn role(self) -> ParameterRole {
        self.role
    }

    /// Returns the semantic value type observed inside the parameter's body.
    ///
    /// Receiver declarations store their owner type and capability separately. This projection is
    /// the sole read-only boundary that turns that contract into the borrow type used by body and
    /// compile-time consumers.
    #[must_use]
    pub fn value_type(self, types: &TypeStore) -> Option<TypeId> {
        match self.value_type_contract() {
            ParameterValueTypeShape::Declared(ty) => types.get(ty).map(|_| ty),
            ParameterValueTypeShape::Borrowed {
                capability,
                referent,
            } => types.identity(&TypeKind::Borrow {
                capability,
                referent,
            }),
        }
    }

    /// Returns the structural contract for the value observed inside the parameter's body.
    #[must_use]
    pub fn value_type_contract(self) -> ParameterValueTypeShape {
        match self.role {
            ParameterRole::Ordinary { .. }
            | ParameterRole::ArgumentPack { .. }
            | ParameterRole::Receiver(CallableCapability::Owned) => {
                ParameterValueTypeShape::Declared(self.ty())
            }
            ParameterRole::Receiver(CallableCapability::Readonly) => {
                ParameterValueTypeShape::Borrowed {
                    capability: BorrowCapability::Readonly,
                    referent: self.ty(),
                }
            }
            ParameterRole::Receiver(CallableCapability::ReadWrite) => {
                ParameterValueTypeShape::Borrowed {
                    capability: BorrowCapability::ReadWrite,
                    referent: self.ty(),
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParameterValueTypeShape {
    Declared(TypeId),
    Borrowed {
        capability: BorrowCapability,
        referent: TypeId,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BodyOwner {
    Callable(CallableId),
    Constant(ConstantId),
    Static(StaticId),
    Drop(DropId),
    Test(TestId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BodyForm {
    Block,
    Expression,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Body {
    owner: BodyOwner,
    form: BodyForm,
}

impl Body {
    #[must_use]
    pub const fn block(owner: BodyOwner) -> Self {
        Self {
            owner,
            form: BodyForm::Block,
        }
    }

    #[must_use]
    pub const fn expression(owner: BodyOwner) -> Self {
        Self {
            owner,
            form: BodyForm::Expression,
        }
    }

    #[must_use]
    pub const fn owner(self) -> BodyOwner {
        self.owner
    }

    #[must_use]
    pub const fn form(self) -> BodyForm {
        self.form
    }
}

#[cfg(test)]
mod tests {
    use nocter_model::CallableGuarantees;

    use super::{CallableContractViolation, CallableKind, callable_contract_violation};

    #[test]
    fn callable_contract_policy_is_closed_over_kind_execution_and_guarantees() {
        assert_eq!(
            callable_contract_violation(
                CallableKind::Function,
                true,
                CallableGuarantees::no_allocation(),
            ),
            Some(CallableContractViolation::DeferredNoAllocation)
        );
        assert_eq!(
            callable_contract_violation(
                CallableKind::Function,
                true,
                CallableGuarantees::default().admit_blocking(),
            ),
            Some(CallableContractViolation::DeferredBlocking)
        );
        assert_eq!(
            callable_contract_violation(
                CallableKind::Function,
                true,
                CallableGuarantees::default().admit_compile_time_evaluation(),
            ),
            Some(CallableContractViolation::DeferredCompileTime)
        );
        assert_eq!(
            callable_contract_violation(
                CallableKind::Primitive,
                false,
                CallableGuarantees::default().admit_compile_time_evaluation(),
            ),
            Some(CallableContractViolation::PrimitiveCompileTime)
        );
        assert_eq!(
            callable_contract_violation(
                CallableKind::Function,
                false,
                CallableGuarantees::no_allocation().admit_compile_time_evaluation(),
            ),
            None
        );
    }
}
