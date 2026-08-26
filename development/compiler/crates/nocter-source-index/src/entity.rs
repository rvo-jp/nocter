use nocter_model::{
    AssociatedTypeId, BodyId, BodyNodeId, BodyScopeId, BuiltinType, CallableId, CaptureId,
    ConstantId, ConstructionId, DeclarationSiteId, DropId, FieldId, GenericParameterId, ImportId,
    InstanceId, InterfaceId, InterfaceImplementationId, LocalBindingId, ModuleId, NominalTypeId,
    OpaqueTypeId, PackageId, PackageTargetId, ParameterId, RequirementId, TestId, TypeAliasId,
    VariantId,
};

/// A syntax-independent identity that can have source projections.
///
/// Type IDs are intentionally absent. One structural type can occur in many source positions;
/// those positions attach to the declaration or expression that uses it rather than redefining
/// structural type identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SemanticEntity {
    Package(PackageId),
    PackageTarget(PackageTargetId),
    Module(ModuleId),
    BuiltinType(BuiltinType),
    Import(ImportId),
    DeclarationSite(DeclarationSiteId),
    NominalType(NominalTypeId),
    TypeAlias(TypeAliasId),
    Interface(InterfaceId),
    AssociatedType(AssociatedTypeId),
    Constant(ConstantId),
    Callable(CallableId),
    Construction(ConstructionId),
    Instance(InstanceId),
    InterfaceImplementation(InterfaceImplementationId),
    Drop(DropId),
    Test(TestId),
    Field(FieldId),
    Variant(VariantId),
    GenericParameter(GenericParameterId),
    Parameter(ParameterId),
    Requirement(RequirementId),
    Body(BodyId),
    BodyScope(BodyId, BodyScopeId),
    BodyNode(BodyId, BodyNodeId),
    LocalBinding(BodyId, LocalBindingId),
    Capture(BodyId, CaptureId),
    OpaqueType(OpaqueTypeId),
}
