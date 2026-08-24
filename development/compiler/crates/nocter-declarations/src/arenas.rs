use std::fmt;

use nocter_model::{
    Arena, ArenaBuilder, AssociatedTypeId, BodyId, CallableId, ConformanceId, ConstantId,
    ConstructionId, DropId, FieldId, GenericParameterId, InstanceId, InterfaceId, NominalTypeId,
    OpaqueTypeId, ParameterId, RequirementId, TestId, TypeAliasId, VariantId,
};

use crate::{
    AssociatedTypeDeclaration, Body, CallableDeclaration, ConformanceDeclaration,
    ConstantDeclaration, ConstructionDeclaration, DropDeclaration, FieldDeclaration,
    GenericParameter, InstanceDeclaration, InterfaceDeclaration, NominalTypeDeclaration,
    OpaqueTypeDeclaration, Parameter, Requirement, TestDeclaration, TypeAliasDeclaration,
    VariantDeclaration,
};

#[derive(Debug)]
struct DefinitionSlots<I, T> {
    slots: ArenaBuilder<I, Option<T>>,
}

impl<I, T> Default for DefinitionSlots<I, T> {
    fn default() -> Self {
        Self {
            slots: ArenaBuilder::default(),
        }
    }
}

macro_rules! definition_slots {
    ($id:ty) => {
        impl<T> DefinitionSlots<$id, T> {
            fn reserve(&mut self) -> $id {
                self.slots.insert(None)
            }

            fn define(&mut self, id: $id, value: T) -> Result<(), DefinitionError> {
                let slot = self.slots.get_mut(id).ok_or(DefinitionError::UnknownId)?;
                if slot.is_some() {
                    return Err(DefinitionError::AlreadyDefined);
                }
                *slot = Some(value);
                Ok(())
            }

            fn finish(self, kind: &'static str) -> Result<Arena<$id, T>, IncompleteDefinition> {
                self.slots
                    .try_finish_with(|_, value| value.ok_or(IncompleteDefinition { kind }))
            }
        }
    };
}

definition_slots!(NominalTypeId);
definition_slots!(TypeAliasId);
definition_slots!(InterfaceId);
definition_slots!(AssociatedTypeId);
definition_slots!(ConstantId);
definition_slots!(CallableId);
definition_slots!(ConstructionId);
definition_slots!(InstanceId);
definition_slots!(ConformanceId);
definition_slots!(DropId);
definition_slots!(TestId);
definition_slots!(VariantId);
definition_slots!(OpaqueTypeId);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefinitionError {
    UnknownId,
    AlreadyDefined,
}

impl fmt::Display for DefinitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownId => {
                formatter.write_str("definition ID was not reserved by this builder")
            }
            Self::AlreadyDefined => {
                formatter.write_str("reserved definition was already completed")
            }
        }
    }
}

impl std::error::Error for DefinitionError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IncompleteDefinition {
    kind: &'static str,
}

impl IncompleteDefinition {
    #[must_use]
    pub const fn kind(self) -> &'static str {
        self.kind
    }
}

impl fmt::Display for IncompleteDefinition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "reserved {} definition was not completed",
            self.kind
        )
    }
}

impl std::error::Error for IncompleteDefinition {}

/// All resolved declaration domains in one immutable compile unit.
#[derive(Debug)]
pub struct DeclarationArenas {
    nominal_types: Arena<NominalTypeId, NominalTypeDeclaration>,
    type_aliases: Arena<TypeAliasId, TypeAliasDeclaration>,
    interfaces: Arena<InterfaceId, InterfaceDeclaration>,
    associated_types: Arena<AssociatedTypeId, AssociatedTypeDeclaration>,
    constants: Arena<ConstantId, ConstantDeclaration>,
    callables: Arena<CallableId, CallableDeclaration>,
    constructions: Arena<ConstructionId, ConstructionDeclaration>,
    instances: Arena<InstanceId, InstanceDeclaration>,
    conformances: Arena<ConformanceId, ConformanceDeclaration>,
    drops: Arena<DropId, DropDeclaration>,
    tests: Arena<TestId, TestDeclaration>,
    fields: Arena<FieldId, FieldDeclaration>,
    variants: Arena<VariantId, VariantDeclaration>,
    generic_parameters: Arena<GenericParameterId, GenericParameter>,
    parameters: Arena<ParameterId, Parameter>,
    requirements: Arena<RequirementId, Requirement>,
    bodies: Arena<BodyId, Body>,
    opaque_types: Arena<OpaqueTypeId, OpaqueTypeDeclaration>,
}

macro_rules! arena_accessors {
    ($($name:ident: $id:ty => $value:ty),+ $(,)?) => {
        $(
            #[must_use]
            pub const fn $name(&self) -> &Arena<$id, $value> {
                &self.$name
            }
        )+
    };
}

impl DeclarationArenas {
    arena_accessors! {
        nominal_types: NominalTypeId => NominalTypeDeclaration,
        type_aliases: TypeAliasId => TypeAliasDeclaration,
        interfaces: InterfaceId => InterfaceDeclaration,
        associated_types: AssociatedTypeId => AssociatedTypeDeclaration,
        constants: ConstantId => ConstantDeclaration,
        callables: CallableId => CallableDeclaration,
        constructions: ConstructionId => ConstructionDeclaration,
        instances: InstanceId => InstanceDeclaration,
        conformances: ConformanceId => ConformanceDeclaration,
        drops: DropId => DropDeclaration,
        tests: TestId => TestDeclaration,
        fields: FieldId => FieldDeclaration,
        variants: VariantId => VariantDeclaration,
        generic_parameters: GenericParameterId => GenericParameter,
        parameters: ParameterId => Parameter,
        requirements: RequirementId => Requirement,
        bodies: BodyId => Body,
        opaque_types: OpaqueTypeId => OpaqueTypeDeclaration,
    }

    /// Returns the complete owner-plus-callable generic domain in semantic identity order.
    #[must_use]
    pub fn callable_generic_domain(
        &self,
        callable: CallableId,
    ) -> Option<Box<[GenericParameterId]>> {
        let declaration = self.callables.get(callable)?;
        let owner = match declaration.owner() {
            crate::CallableOwner::Module(_) => &[][..],
            crate::CallableOwner::Construction(id) => {
                self.constructions.get(id)?.generic_parameters()
            }
            crate::CallableOwner::Instance(id) => self.instances.get(id)?.generic_parameters(),
            crate::CallableOwner::Interface(id) => self.interfaces.get(id)?.generic_parameters(),
            crate::CallableOwner::Conformance(id) => {
                self.conformances.get(id)?.generic_parameters()
            }
        };
        let mut complete = owner
            .iter()
            .chain(declaration.generic_parameters())
            .copied()
            .collect::<Vec<_>>();
        complete.sort_unstable();
        complete.dedup();
        Some(complete.into_boxed_slice())
    }

    /// Returns the complete generic domain visible to one declared body.
    #[must_use]
    pub fn body_generic_domain(&self, body: BodyId) -> Option<Box<[GenericParameterId]>> {
        match self.bodies.get(body)?.owner() {
            crate::BodyOwner::Callable(callable) => self.callable_generic_domain(callable),
            crate::BodyOwner::Drop(drop) => self
                .drops
                .get(drop)
                .map(|declaration| Box::from(declaration.generic_parameters())),
            crate::BodyOwner::Test(_) => Some(Box::new([])),
        }
    }
}

/// Two-pass builder for mutually referential declaration headers.
///
/// Identity-bearing declarations are reserved in canonical order, then completed after their
/// generic parameters, members, and resolved header types exist. Finishing rejects every unfilled
/// reservation and exposes only immutable arenas.
#[derive(Debug, Default)]
pub struct DeclarationArenaBuilder {
    nominal_types: DefinitionSlots<NominalTypeId, NominalTypeDeclaration>,
    type_aliases: DefinitionSlots<TypeAliasId, TypeAliasDeclaration>,
    interfaces: DefinitionSlots<InterfaceId, InterfaceDeclaration>,
    associated_types: DefinitionSlots<AssociatedTypeId, AssociatedTypeDeclaration>,
    constants: DefinitionSlots<ConstantId, ConstantDeclaration>,
    callables: DefinitionSlots<CallableId, CallableDeclaration>,
    constructions: DefinitionSlots<ConstructionId, ConstructionDeclaration>,
    instances: DefinitionSlots<InstanceId, InstanceDeclaration>,
    conformances: DefinitionSlots<ConformanceId, ConformanceDeclaration>,
    drops: DefinitionSlots<DropId, DropDeclaration>,
    tests: DefinitionSlots<TestId, TestDeclaration>,
    fields: ArenaBuilder<FieldId, FieldDeclaration>,
    variants: DefinitionSlots<VariantId, VariantDeclaration>,
    generic_parameters: ArenaBuilder<GenericParameterId, GenericParameter>,
    parameters: ArenaBuilder<ParameterId, Parameter>,
    requirements: ArenaBuilder<RequirementId, Requirement>,
    bodies: ArenaBuilder<BodyId, Body>,
    opaque_types: DefinitionSlots<OpaqueTypeId, OpaqueTypeDeclaration>,
}

macro_rules! reservation_methods {
    ($reserve:ident, $define:ident, $field:ident, $id:ty, $value:ty) => {
        pub fn $reserve(&mut self) -> $id {
            self.$field.reserve()
        }

        /// Completes a previously reserved declaration identity.
        ///
        /// # Errors
        ///
        /// Returns [`DefinitionError`] when the identity is unknown or already complete.
        pub fn $define(&mut self, id: $id, value: $value) -> Result<(), DefinitionError> {
            self.$field.define(id, value)
        }
    };
}

impl DeclarationArenaBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    reservation_methods!(
        reserve_nominal_type,
        define_nominal_type,
        nominal_types,
        NominalTypeId,
        NominalTypeDeclaration
    );
    reservation_methods!(
        reserve_type_alias,
        define_type_alias,
        type_aliases,
        TypeAliasId,
        TypeAliasDeclaration
    );
    reservation_methods!(
        reserve_interface,
        define_interface,
        interfaces,
        InterfaceId,
        InterfaceDeclaration
    );
    reservation_methods!(
        reserve_associated_type,
        define_associated_type,
        associated_types,
        AssociatedTypeId,
        AssociatedTypeDeclaration
    );
    reservation_methods!(
        reserve_constant,
        define_constant,
        constants,
        ConstantId,
        ConstantDeclaration
    );
    reservation_methods!(
        reserve_callable,
        define_callable,
        callables,
        CallableId,
        CallableDeclaration
    );
    reservation_methods!(
        reserve_construction,
        define_construction,
        constructions,
        ConstructionId,
        ConstructionDeclaration
    );
    reservation_methods!(
        reserve_instance,
        define_instance,
        instances,
        InstanceId,
        InstanceDeclaration
    );
    reservation_methods!(
        reserve_conformance,
        define_conformance,
        conformances,
        ConformanceId,
        ConformanceDeclaration
    );
    reservation_methods!(reserve_drop, define_drop, drops, DropId, DropDeclaration);
    reservation_methods!(reserve_test, define_test, tests, TestId, TestDeclaration);
    reservation_methods!(
        reserve_variant,
        define_variant,
        variants,
        VariantId,
        VariantDeclaration
    );
    reservation_methods!(
        reserve_opaque_type,
        define_opaque_type,
        opaque_types,
        OpaqueTypeId,
        OpaqueTypeDeclaration
    );

    pub fn add_field(&mut self, value: FieldDeclaration) -> FieldId {
        self.fields.insert(value)
    }

    pub fn add_generic_parameter(&mut self, value: GenericParameter) -> GenericParameterId {
        self.generic_parameters.insert(value)
    }

    pub fn add_parameter(&mut self, value: Parameter) -> ParameterId {
        self.parameters.insert(value)
    }

    #[must_use]
    pub fn parameter(&self, id: ParameterId) -> Option<Parameter> {
        self.parameters.get(id).copied()
    }

    pub fn add_requirement(&mut self, value: Requirement) -> RequirementId {
        self.requirements.insert(value)
    }

    pub fn add_body(&mut self, value: Body) -> BodyId {
        self.bodies.insert(value)
    }

    /// Freezes every declaration domain after proving that all reservations are complete.
    ///
    /// # Errors
    ///
    /// Returns [`IncompleteDefinition`] for the first unfilled identity domain.
    pub fn finish(self) -> Result<DeclarationArenas, IncompleteDefinition> {
        Ok(DeclarationArenas {
            nominal_types: self.nominal_types.finish("nominal type")?,
            type_aliases: self.type_aliases.finish("type alias")?,
            interfaces: self.interfaces.finish("interface")?,
            associated_types: self.associated_types.finish("associated type")?,
            constants: self.constants.finish("constant")?,
            callables: self.callables.finish("callable")?,
            constructions: self.constructions.finish("construction")?,
            instances: self.instances.finish("instance")?,
            conformances: self.conformances.finish("conformance")?,
            drops: self.drops.finish("drop")?,
            tests: self.tests.finish("test")?,
            fields: self.fields.finish(),
            variants: self.variants.finish("variant")?,
            generic_parameters: self.generic_parameters.finish(),
            parameters: self.parameters.finish(),
            requirements: self.requirements.finish(),
            bodies: self.bodies.finish(),
            opaque_types: self.opaque_types.finish("opaque type")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use nocter_model::{BuiltinType, SymbolTable};

    use super::{DeclarationArenaBuilder, DefinitionError};
    use crate::{
        CallableDeclaration, CallableKind, CallableOwner, CallableProvenance,
        CallableProvenanceContract, DeclarationProgramBuilder, ModuleNamespace, ModulePath,
    };

    #[test]
    fn reservations_must_be_completed_exactly_once() {
        let symbols = SymbolTable::from_spellings(["app"]);
        let app_name = symbols.get("app").unwrap();
        let mut program =
            DeclarationProgramBuilder::new(nocter_model::CompilationTarget::Arm64Darwin, symbols);
        let package = program
            .add_package(
                nocter_model::PackageIdentity::new("workspace:app"),
                app_name,
            )
            .unwrap();
        let module = program.add_module(package, ModulePath::root()).unwrap();
        program
            .define_module_namespace(module, ModuleNamespace::default())
            .unwrap();
        let site = program
            .add_declaration_site(module, crate::Visibility::Private)
            .unwrap();
        let result = program.types_mut().builtin(BuiltinType::Void);
        let declarations = program.declarations_mut();
        let callable = declarations.reserve_callable();
        let definition = || {
            CallableDeclaration::new(
                site,
                CallableOwner::Module(module),
                CallableKind::Function,
                Some(app_name),
                None,
                [],
                [],
                result,
                CallableProvenanceContract::declared(CallableProvenance::empty()),
                crate::ProvenanceAnnotation::Elided,
                [],
                None,
                None,
            )
        };

        assert_eq!(
            declarations
                .define_callable(callable, definition())
                .unwrap(),
            ()
        );
        assert_eq!(
            declarations
                .define_callable(callable, definition())
                .unwrap_err(),
            DefinitionError::AlreadyDefined
        );
        assert_eq!(
            program.finish().unwrap().declarations().callables().len(),
            1
        );
    }

    #[test]
    fn incomplete_reservations_cannot_enter_a_program() {
        let mut builder = DeclarationArenaBuilder::new();
        let _callable = builder.reserve_callable();

        assert_eq!(builder.finish().unwrap_err().kind(), "callable");
    }

    #[test]
    fn recursively_named_associated_types_are_reserved_before_definition() {
        let mut builder = DeclarationArenaBuilder::new();
        let _associated = builder.reserve_associated_type();

        assert_eq!(builder.finish().unwrap_err().kind(), "associated type");
    }
}
