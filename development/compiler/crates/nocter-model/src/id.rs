use std::fmt;

pub(crate) trait SemanticId: Copy {
    fn new(index: usize) -> Self;
    fn index(self) -> usize;
}

macro_rules! semantic_ids {
    ($($name:ident),+ $(,)?) => {
        $(
            #[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
            pub struct $name(usize);

            impl SemanticId for $name {
                fn new(index: usize) -> Self {
                    Self(index)
                }

                fn index(self) -> usize {
                    self.0
                }
            }

            impl fmt::Debug for $name {
                fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                    write!(formatter, "{}({})", stringify!($name), self.0)
                }
            }
        )+
    };
}

semantic_ids! {
    PackageId,
    PackageTargetId,
    ModuleId,
    ImportId,
    DeclarationSiteId,
    NominalTypeId,
    TypeAliasId,
    InterfaceId,
    AssociatedTypeId,
    CallableId,
    ConstructionId,
    InstanceId,
    ConformanceId,
    DropId,
    TestId,
    FieldId,
    VariantId,
    GenericParameterId,
    ParameterId,
    RequirementId,
    BodyId,
    BodyNodeId,
    PlaceId,
    LoopId,
    BodyScopeId,
    LocalBindingId,
    CaptureId,
    OpaqueTypeId,
    TypeId,
}
