use nocter_model::{
    AssociatedTypeId, BodyId, CallableId, CompilationTarget, ConformanceId, ConstructionId,
    DeclarationSiteId, DropId, FieldId, GenericParameterId, InstanceId, InterfaceId, NominalTypeId,
    ParameterId, RequirementId, Symbol, TypeAliasId, TypeId, VariantId,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GenericOwner {
    NominalType(NominalTypeId),
    TypeAlias(TypeAliasId),
    Interface(InterfaceId),
    Callable(CallableId),
    Construction(ConstructionId),
    Instance(InstanceId),
    Conformance(ConformanceId),
    Drop(DropId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenericParameter {
    owner: GenericOwner,
    name: Symbol,
    position: usize,
}

impl GenericParameter {
    #[must_use]
    pub const fn new(owner: GenericOwner, name: Symbol, position: usize) -> Self {
        Self {
            owner,
            name,
            position,
        }
    }

    #[must_use]
    pub const fn owner(self) -> GenericOwner {
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
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct InterfaceApplication {
    interface: InterfaceId,
    arguments: Box<[TypeId]>,
}

impl InterfaceApplication {
    #[must_use]
    pub fn new(interface: InterfaceId, arguments: impl Into<Box<[TypeId]>>) -> Self {
        Self {
            interface,
            arguments: arguments.into(),
        }
    }

    #[must_use]
    pub const fn interface(&self) -> InterfaceId {
        self.interface
    }

    #[must_use]
    pub const fn arguments(&self) -> &[TypeId] {
        &self.arguments
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NominalShape {
    Struct {
        copy_declared: bool,
        fields: Box<[FieldId]>,
    },
    Enum {
        variants: Box<[VariantId]>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NominalTypeDeclaration {
    site: DeclarationSiteId,
    name: Symbol,
    generic_parameters: Box<[GenericParameterId]>,
    requirements: Box<[RequirementId]>,
    shape: NominalShape,
    target_gate: Option<CompilationTarget>,
}

impl NominalTypeDeclaration {
    #[must_use]
    pub fn new(
        site: DeclarationSiteId,
        name: Symbol,
        generic_parameters: impl Into<Box<[GenericParameterId]>>,
        requirements: impl Into<Box<[RequirementId]>>,
        shape: NominalShape,
        target_gate: Option<CompilationTarget>,
    ) -> Self {
        Self {
            site,
            name,
            generic_parameters: generic_parameters.into(),
            requirements: requirements.into(),
            shape,
            target_gate,
        }
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
    pub const fn generic_parameters(&self) -> &[GenericParameterId] {
        &self.generic_parameters
    }

    #[must_use]
    pub const fn requirements(&self) -> &[RequirementId] {
        &self.requirements
    }

    #[must_use]
    pub const fn shape(&self) -> &NominalShape {
        &self.shape
    }

    #[must_use]
    pub const fn target_gate(&self) -> Option<CompilationTarget> {
        self.target_gate
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FieldDeclaration {
    site: DeclarationSiteId,
    owner: NominalTypeId,
    name: Symbol,
    ty: TypeId,
}

impl FieldDeclaration {
    #[must_use]
    pub const fn new(
        site: DeclarationSiteId,
        owner: NominalTypeId,
        name: Symbol,
        ty: TypeId,
    ) -> Self {
        Self {
            site,
            owner,
            name,
            ty,
        }
    }

    #[must_use]
    pub const fn site(self) -> DeclarationSiteId {
        self.site
    }

    #[must_use]
    pub const fn owner(self) -> NominalTypeId {
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariantDeclaration {
    site: DeclarationSiteId,
    owner: NominalTypeId,
    name: Symbol,
    payload: Box<[ParameterId]>,
}

impl VariantDeclaration {
    #[must_use]
    pub fn new(
        site: DeclarationSiteId,
        owner: NominalTypeId,
        name: Symbol,
        payload: impl Into<Box<[ParameterId]>>,
    ) -> Self {
        Self {
            site,
            owner,
            name,
            payload: payload.into(),
        }
    }

    #[must_use]
    pub const fn site(&self) -> DeclarationSiteId {
        self.site
    }

    #[must_use]
    pub const fn owner(&self) -> NominalTypeId {
        self.owner
    }

    #[must_use]
    pub const fn name(&self) -> Symbol {
        self.name
    }

    #[must_use]
    pub const fn payload(&self) -> &[ParameterId] {
        &self.payload
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeAliasDeclaration {
    site: DeclarationSiteId,
    name: Symbol,
    generic_parameters: Box<[GenericParameterId]>,
    target: TypeId,
    requirements: Box<[RequirementId]>,
    target_gate: Option<CompilationTarget>,
}

impl TypeAliasDeclaration {
    #[must_use]
    pub fn new(
        site: DeclarationSiteId,
        name: Symbol,
        generic_parameters: impl Into<Box<[GenericParameterId]>>,
        target: TypeId,
        requirements: impl Into<Box<[RequirementId]>>,
        target_gate: Option<CompilationTarget>,
    ) -> Self {
        Self {
            site,
            name,
            generic_parameters: generic_parameters.into(),
            target,
            requirements: requirements.into(),
            target_gate,
        }
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
    pub const fn generic_parameters(&self) -> &[GenericParameterId] {
        &self.generic_parameters
    }

    #[must_use]
    pub const fn target(&self) -> TypeId {
        self.target
    }

    #[must_use]
    pub const fn requirements(&self) -> &[RequirementId] {
        &self.requirements
    }

    #[must_use]
    pub const fn target_gate(&self) -> Option<CompilationTarget> {
        self.target_gate
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InterfaceDeclaration {
    site: DeclarationSiteId,
    name: Symbol,
    generic_parameters: Box<[GenericParameterId]>,
    requirements: Box<[RequirementId]>,
    associated_types: Box<[AssociatedTypeId]>,
    methods: Box<[CallableId]>,
    target_gate: Option<CompilationTarget>,
}

impl InterfaceDeclaration {
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        site: DeclarationSiteId,
        name: Symbol,
        generic_parameters: impl Into<Box<[GenericParameterId]>>,
        requirements: impl Into<Box<[RequirementId]>>,
        associated_types: impl Into<Box<[AssociatedTypeId]>>,
        methods: impl Into<Box<[CallableId]>>,
        target_gate: Option<CompilationTarget>,
    ) -> Self {
        Self {
            site,
            name,
            generic_parameters: generic_parameters.into(),
            requirements: requirements.into(),
            associated_types: associated_types.into(),
            methods: methods.into(),
            target_gate,
        }
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
    pub const fn generic_parameters(&self) -> &[GenericParameterId] {
        &self.generic_parameters
    }

    #[must_use]
    pub const fn requirements(&self) -> &[RequirementId] {
        &self.requirements
    }

    #[must_use]
    pub const fn associated_types(&self) -> &[AssociatedTypeId] {
        &self.associated_types
    }

    #[must_use]
    pub const fn methods(&self) -> &[CallableId] {
        &self.methods
    }

    #[must_use]
    pub const fn target_gate(&self) -> Option<CompilationTarget> {
        self.target_gate
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssociatedTypeDeclaration {
    site: DeclarationSiteId,
    interface: InterfaceId,
    name: Symbol,
    bounds: Box<[RequirementId]>,
}

impl AssociatedTypeDeclaration {
    #[must_use]
    pub fn new(
        site: DeclarationSiteId,
        interface: InterfaceId,
        name: Symbol,
        bounds: impl Into<Box<[RequirementId]>>,
    ) -> Self {
        Self {
            site,
            interface,
            name,
            bounds: bounds.into(),
        }
    }

    #[must_use]
    pub const fn site(&self) -> DeclarationSiteId {
        self.site
    }

    #[must_use]
    pub const fn interface(&self) -> InterfaceId {
        self.interface
    }

    #[must_use]
    pub const fn name(&self) -> Symbol {
        self.name
    }

    #[must_use]
    pub const fn bounds(&self) -> &[RequirementId] {
        &self.bounds
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstructionDeclaration {
    site: DeclarationSiteId,
    target: TypeId,
    generic_parameters: Box<[GenericParameterId]>,
    members: Box<[CallableId]>,
}

impl ConstructionDeclaration {
    #[must_use]
    pub fn new(
        site: DeclarationSiteId,
        target: TypeId,
        generic_parameters: impl Into<Box<[GenericParameterId]>>,
        members: impl Into<Box<[CallableId]>>,
    ) -> Self {
        Self {
            site,
            target,
            generic_parameters: generic_parameters.into(),
            members: members.into(),
        }
    }

    #[must_use]
    pub const fn site(&self) -> DeclarationSiteId {
        self.site
    }

    #[must_use]
    pub const fn target(&self) -> TypeId {
        self.target
    }

    #[must_use]
    pub const fn generic_parameters(&self) -> &[GenericParameterId] {
        &self.generic_parameters
    }

    #[must_use]
    pub const fn members(&self) -> &[CallableId] {
        &self.members
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstanceDeclaration {
    site: DeclarationSiteId,
    target: TypeId,
    generic_parameters: Box<[GenericParameterId]>,
    requirements: Box<[RequirementId]>,
    members: Box<[CallableId]>,
}

impl InstanceDeclaration {
    #[must_use]
    pub fn new(
        site: DeclarationSiteId,
        target: TypeId,
        generic_parameters: impl Into<Box<[GenericParameterId]>>,
        requirements: impl Into<Box<[RequirementId]>>,
        members: impl Into<Box<[CallableId]>>,
    ) -> Self {
        Self {
            site,
            target,
            generic_parameters: generic_parameters.into(),
            requirements: requirements.into(),
            members: members.into(),
        }
    }

    #[must_use]
    pub const fn site(&self) -> DeclarationSiteId {
        self.site
    }

    #[must_use]
    pub const fn target(&self) -> TypeId {
        self.target
    }

    #[must_use]
    pub const fn generic_parameters(&self) -> &[GenericParameterId] {
        &self.generic_parameters
    }

    #[must_use]
    pub const fn requirements(&self) -> &[RequirementId] {
        &self.requirements
    }

    #[must_use]
    pub const fn members(&self) -> &[CallableId] {
        &self.members
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssociatedTypeBinding {
    declaration: AssociatedTypeId,
    ty: TypeId,
}

impl AssociatedTypeBinding {
    #[must_use]
    pub const fn new(declaration: AssociatedTypeId, ty: TypeId) -> Self {
        Self { declaration, ty }
    }

    #[must_use]
    pub const fn declaration(self) -> AssociatedTypeId {
        self.declaration
    }

    #[must_use]
    pub const fn ty(self) -> TypeId {
        self.ty
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConformanceDeclaration {
    site: DeclarationSiteId,
    interface: InterfaceApplication,
    target: TypeId,
    generic_parameters: Box<[GenericParameterId]>,
    requirements: Box<[RequirementId]>,
    associated_types: Box<[AssociatedTypeBinding]>,
    methods: Box<[CallableId]>,
}

impl ConformanceDeclaration {
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        site: DeclarationSiteId,
        interface: InterfaceApplication,
        target: TypeId,
        generic_parameters: impl Into<Box<[GenericParameterId]>>,
        requirements: impl Into<Box<[RequirementId]>>,
        associated_types: impl Into<Box<[AssociatedTypeBinding]>>,
        methods: impl Into<Box<[CallableId]>>,
    ) -> Self {
        Self {
            site,
            interface,
            target,
            generic_parameters: generic_parameters.into(),
            requirements: requirements.into(),
            associated_types: associated_types.into(),
            methods: methods.into(),
        }
    }

    #[must_use]
    pub const fn site(&self) -> DeclarationSiteId {
        self.site
    }

    #[must_use]
    pub const fn interface(&self) -> &InterfaceApplication {
        &self.interface
    }

    #[must_use]
    pub const fn target(&self) -> TypeId {
        self.target
    }

    #[must_use]
    pub const fn generic_parameters(&self) -> &[GenericParameterId] {
        &self.generic_parameters
    }

    #[must_use]
    pub const fn requirements(&self) -> &[RequirementId] {
        &self.requirements
    }

    #[must_use]
    pub const fn associated_types(&self) -> &[AssociatedTypeBinding] {
        &self.associated_types
    }

    #[must_use]
    pub const fn methods(&self) -> &[CallableId] {
        &self.methods
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpaqueTypeDeclaration {
    owner: CallableId,
    generic_parameters: Box<[GenericParameterId]>,
    interface: InterfaceApplication,
    associated_types: Box<[AssociatedTypeBinding]>,
}

impl OpaqueTypeDeclaration {
    #[must_use]
    pub fn new(
        owner: CallableId,
        generic_parameters: impl Into<Box<[GenericParameterId]>>,
        interface: InterfaceApplication,
        associated_types: impl Into<Box<[AssociatedTypeBinding]>>,
    ) -> Self {
        Self {
            owner,
            generic_parameters: generic_parameters.into(),
            interface,
            associated_types: associated_types.into(),
        }
    }

    #[must_use]
    pub const fn owner(&self) -> CallableId {
        self.owner
    }

    #[must_use]
    pub const fn generic_parameters(&self) -> &[GenericParameterId] {
        &self.generic_parameters
    }

    #[must_use]
    pub const fn interface(&self) -> &InterfaceApplication {
        &self.interface
    }

    #[must_use]
    pub const fn associated_types(&self) -> &[AssociatedTypeBinding] {
        &self.associated_types
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DropDeclaration {
    site: DeclarationSiteId,
    target: TypeId,
    generic_parameters: Box<[GenericParameterId]>,
    receiver: ParameterId,
    body: BodyId,
}

impl DropDeclaration {
    #[must_use]
    pub fn new(
        site: DeclarationSiteId,
        target: TypeId,
        generic_parameters: impl Into<Box<[GenericParameterId]>>,
        receiver: ParameterId,
        body: BodyId,
    ) -> Self {
        Self {
            site,
            target,
            generic_parameters: generic_parameters.into(),
            receiver,
            body,
        }
    }

    #[must_use]
    pub const fn site(&self) -> DeclarationSiteId {
        self.site
    }

    #[must_use]
    pub const fn target(&self) -> TypeId {
        self.target
    }

    #[must_use]
    pub const fn generic_parameters(&self) -> &[GenericParameterId] {
        &self.generic_parameters
    }

    #[must_use]
    pub const fn receiver(&self) -> ParameterId {
        self.receiver
    }

    #[must_use]
    pub const fn body(&self) -> BodyId {
        self.body
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TestDeclaration {
    site: DeclarationSiteId,
    name: Symbol,
    body: BodyId,
}

impl TestDeclaration {
    #[must_use]
    pub const fn new(site: DeclarationSiteId, name: Symbol, body: BodyId) -> Self {
        Self { site, name, body }
    }

    #[must_use]
    pub const fn site(self) -> DeclarationSiteId {
        self.site
    }

    #[must_use]
    pub const fn name(self) -> Symbol {
        self.name
    }

    #[must_use]
    pub const fn body(self) -> BodyId {
        self.body
    }
}
