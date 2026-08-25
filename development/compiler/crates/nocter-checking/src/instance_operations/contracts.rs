use nocter_model::{
    BorrowCapability, CallableCapability, CallableId, DeclarationSiteId, RequirementId, Symbol,
    TypeId,
};

/// One instance-owned method whose static declaration shape was validated during preparation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedInstanceMethod {
    callable: CallableId,
    site: DeclarationSiteId,
    name: Symbol,
    receiver_capability: CallableCapability,
}

impl CheckedInstanceMethod {
    pub(super) const fn new(
        callable: CallableId,
        site: DeclarationSiteId,
        name: Symbol,
        receiver_capability: CallableCapability,
    ) -> Self {
        Self {
            callable,
            site,
            name,
            receiver_capability,
        }
    }

    #[must_use]
    pub const fn callable(&self) -> CallableId {
        self.callable
    }

    #[must_use]
    pub const fn site(&self) -> DeclarationSiteId {
        self.site
    }

    #[must_use]
    pub const fn name(&self) -> Symbol {
        self.name
    }

    #[must_use]
    pub const fn receiver_capability(&self) -> CallableCapability {
        self.receiver_capability
    }
}

/// One validated borrow coercion before its instance pattern is specialized.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedInstanceCoercion {
    callable: CallableId,
    site: DeclarationSiteId,
    receiver_capability: BorrowCapability,
    result_capability: BorrowCapability,
    target: TypeId,
    requirements: Box<[RequirementId]>,
}

impl CheckedInstanceCoercion {
    pub(super) fn new(
        callable: CallableId,
        site: DeclarationSiteId,
        receiver_capability: BorrowCapability,
        result_capability: BorrowCapability,
        target: TypeId,
        requirements: impl Into<Box<[RequirementId]>>,
    ) -> Self {
        Self {
            callable,
            site,
            receiver_capability,
            result_capability,
            target,
            requirements: requirements.into(),
        }
    }

    #[must_use]
    pub const fn callable(&self) -> CallableId {
        self.callable
    }

    #[must_use]
    pub const fn site(&self) -> DeclarationSiteId {
        self.site
    }

    #[must_use]
    pub const fn receiver_capability(&self) -> BorrowCapability {
        self.receiver_capability
    }

    #[must_use]
    pub const fn result_capability(&self) -> BorrowCapability {
        self.result_capability
    }

    #[must_use]
    pub const fn target(&self) -> TypeId {
        self.target
    }

    #[must_use]
    pub const fn requirements(&self) -> &[RequirementId] {
        &self.requirements
    }
}

/// One validated index operation before its instance pattern is specialized.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedInstanceIndex {
    callable: CallableId,
    site: DeclarationSiteId,
    capability: BorrowCapability,
    index: TypeId,
    result: TypeId,
    requirements: Box<[RequirementId]>,
}

impl CheckedInstanceIndex {
    pub(super) fn new(
        callable: CallableId,
        site: DeclarationSiteId,
        capability: BorrowCapability,
        index: TypeId,
        result: TypeId,
        requirements: impl Into<Box<[RequirementId]>>,
    ) -> Self {
        Self {
            callable,
            site,
            capability,
            index,
            result,
            requirements: requirements.into(),
        }
    }

    #[must_use]
    pub const fn callable(&self) -> CallableId {
        self.callable
    }

    #[must_use]
    pub const fn site(&self) -> DeclarationSiteId {
        self.site
    }

    #[must_use]
    pub const fn capability(&self) -> BorrowCapability {
        self.capability
    }

    #[must_use]
    pub const fn index(&self) -> TypeId {
        self.index
    }

    #[must_use]
    pub const fn result(&self) -> TypeId {
        self.result
    }

    #[must_use]
    pub const fn requirements(&self) -> &[RequirementId] {
        &self.requirements
    }
}

/// One validated equality or ordering operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedInstanceComparison {
    callable: CallableId,
    site: DeclarationSiteId,
    requirements: Box<[RequirementId]>,
}

impl CheckedInstanceComparison {
    pub(super) fn new(
        callable: CallableId,
        site: DeclarationSiteId,
        requirements: impl Into<Box<[RequirementId]>>,
    ) -> Self {
        Self {
            callable,
            site,
            requirements: requirements.into(),
        }
    }

    #[must_use]
    pub const fn callable(&self) -> CallableId {
        self.callable
    }

    #[must_use]
    pub const fn site(&self) -> DeclarationSiteId {
        self.site
    }

    #[must_use]
    pub const fn requirements(&self) -> &[RequirementId] {
        &self.requirements
    }
}

/// One validated sequence-expansion operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedInstanceExpansion {
    callable: CallableId,
    site: DeclarationSiteId,
    capability: nocter_declarations::ExpansionCapability,
    result: TypeId,
    requirements: Box<[RequirementId]>,
}

impl CheckedInstanceExpansion {
    pub(super) fn new(
        callable: CallableId,
        site: DeclarationSiteId,
        capability: nocter_declarations::ExpansionCapability,
        result: TypeId,
        requirements: impl Into<Box<[RequirementId]>>,
    ) -> Self {
        Self {
            callable,
            site,
            capability,
            result,
            requirements: requirements.into(),
        }
    }

    #[must_use]
    pub const fn callable(&self) -> CallableId {
        self.callable
    }

    #[must_use]
    pub const fn site(&self) -> DeclarationSiteId {
        self.site
    }

    #[must_use]
    pub const fn capability(&self) -> nocter_declarations::ExpansionCapability {
        self.capability
    }

    #[must_use]
    pub const fn result(&self) -> TypeId {
        self.result
    }

    #[must_use]
    pub const fn requirements(&self) -> &[RequirementId] {
        &self.requirements
    }
}

/// A statically validated instance member. Selection may specialize this contract but never
/// reinterpret its declaration kind or signature shape.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedInstanceMember {
    Method(CheckedInstanceMethod),
    Coercion(CheckedInstanceCoercion),
    Equality(CheckedInstanceComparison),
    Ordering(CheckedInstanceComparison),
    Index(CheckedInstanceIndex),
    Expansion(CheckedInstanceExpansion),
}

impl CheckedInstanceMember {
    #[must_use]
    pub const fn callable(&self) -> CallableId {
        match self {
            Self::Method(contract) => contract.callable(),
            Self::Coercion(contract) => contract.callable(),
            Self::Equality(contract) | Self::Ordering(contract) => contract.callable(),
            Self::Index(contract) => contract.callable(),
            Self::Expansion(contract) => contract.callable(),
        }
    }
}
