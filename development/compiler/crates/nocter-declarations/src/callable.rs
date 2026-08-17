use std::fmt;

use nocter_model::{
    BodyId, CallableCapability, CallableId, ConformanceId, ConstructionId, DeclarationSiteId,
    DropId, GenericParameterId, InstanceId, InterfaceId, ModuleId, ParameterId, RequirementId,
    Symbol, TestId, TypeId, VariantId,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LiteralShape {
    Sequence,
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
    Conformance(ConformanceId),
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
    provenance: CallableProvenanceContract,
    requirements: Box<[RequirementId]>,
    body: Option<BodyId>,
    target_gate: Option<Symbol>,
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
        provenance: CallableProvenanceContract,
        requirements: impl Into<Box<[RequirementId]>>,
        body: Option<BodyId>,
        target_gate: Option<Symbol>,
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
            provenance,
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

    #[must_use]
    pub const fn result(&self) -> TypeId {
        self.result
    }

    #[must_use]
    pub const fn provenance(&self) -> &CallableProvenanceContract {
        &self.provenance
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
    pub const fn target_gate(&self) -> Option<Symbol> {
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
    Ordinary { position: usize, variadic: bool },
    Receiver(CallableCapability),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Parameter {
    owner: ParameterOwner,
    name: Symbol,
    ty: TypeId,
    role: ParameterRole,
}

impl Parameter {
    #[must_use]
    pub const fn new(owner: ParameterOwner, name: Symbol, ty: TypeId, role: ParameterRole) -> Self {
        Self {
            owner,
            name,
            ty,
            role,
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
        self.ty
    }

    #[must_use]
    pub const fn role(self) -> ParameterRole {
        self.role
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BodyOwner {
    Callable(CallableId),
    Drop(DropId),
    Test(TestId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Body {
    owner: BodyOwner,
}

impl Body {
    #[must_use]
    pub const fn new(owner: BodyOwner) -> Self {
        Self { owner }
    }

    #[must_use]
    pub const fn owner(self) -> BodyOwner {
        self.owner
    }
}
