use nocter_model::{
    AssociatedTypeId, BodyId, CallableId, ConformanceId, ConstructionId, DeclarationSiteId, DropId,
    FieldId, GenericParameterId, ImportId, InstanceId, InterfaceId, ModuleId, NominalTypeId,
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
    Import(ImportId),
    DeclarationSite(DeclarationSiteId),
    NominalType(NominalTypeId),
    TypeAlias(TypeAliasId),
    Interface(InterfaceId),
    AssociatedType(AssociatedTypeId),
    Callable(CallableId),
    Construction(ConstructionId),
    Instance(InstanceId),
    Conformance(ConformanceId),
    Drop(DropId),
    Test(TestId),
    Field(FieldId),
    Variant(VariantId),
    GenericParameter(GenericParameterId),
    Parameter(ParameterId),
    Requirement(RequirementId),
    Body(BodyId),
    OpaqueType(OpaqueTypeId),
}
