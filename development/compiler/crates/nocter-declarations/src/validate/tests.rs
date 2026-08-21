use nocter_model::{BuiltinType, CallableCapability, PackageIdentity, SymbolTable, TypeKind};

use crate::{
    Body, BodyOwner, BuiltinAttachment, CallableDeclaration, CallableKind, CallableOwner,
    CallableProvenance, CallableProvenanceContract, ConstructionDeclaration, DeclarationDomain,
    DeclarationProgramBuilder, DeclarationRule, DeclarationViolation, DropDeclaration,
    FieldDeclaration, GenericOwner, GenericParameter, InstanceDeclaration, ModuleNamespace,
    ModulePath, NominalShape, NominalTypeDeclaration, PackageTarget, PackageTargetKind, Parameter,
    ParameterOwner, ParameterRole, ProgramBuildError, ProgramIntegrityError,
    ProgramValidationError, ProvenanceOrigin, VariantDeclaration, Visibility,
};

#[test]
fn package_target_names_and_positions_are_unique_within_their_typed_domains() {
    let symbols = SymbolTable::from_spellings(["app", "run"]);
    let app_name = symbols.get("app").unwrap();
    let run_name = symbols.get("run").unwrap();
    let mut program =
        DeclarationProgramBuilder::new(nocter_model::CompilationTarget::Arm64Darwin, symbols);
    let package = program
        .add_package(PackageIdentity::new("workspace:app"), app_name)
        .unwrap();
    let module = program.add_module(package, ModulePath::root()).unwrap();
    program
        .define_module_namespace(module, ModuleNamespace::default())
        .unwrap();
    program
        .add_package_target(PackageTarget::new(
            package,
            module,
            run_name,
            PackageTargetKind::Executable,
            0,
        ))
        .unwrap();
    program
        .add_package_target(PackageTarget::new(
            package,
            module,
            run_name,
            PackageTargetKind::Executable,
            1,
        ))
        .unwrap();

    assert_eq!(
        program.finish().unwrap_err(),
        ProgramBuildError::InvalidProgram(ProgramValidationError::Integrity(
            ProgramIntegrityError::DuplicateReference(DeclarationDomain::PackageTarget)
        ))
    );
}

#[test]
fn two_pass_definitions_support_recursive_header_identity() {
    let symbols = SymbolTable::from_spellings(["app", "Box", "T", "value"]);
    let app_name = symbols.get("app").unwrap();
    let box_name = symbols.get("Box").unwrap();
    let parameter_name = symbols.get("T").unwrap();
    let field_name = symbols.get("value").unwrap();
    let mut program =
        DeclarationProgramBuilder::new(nocter_model::CompilationTarget::Arm64Darwin, symbols);
    let package = program
        .add_package(PackageIdentity::new("workspace:app"), app_name)
        .unwrap();
    let module = program.add_module(package, ModulePath::root()).unwrap();
    program
        .define_module_namespace(module, ModuleNamespace::default())
        .unwrap();
    let type_site = program
        .add_declaration_site(module, Visibility::Public)
        .unwrap();
    let field_site = program
        .add_declaration_site(module, Visibility::Public)
        .unwrap();

    let nominal = program.declarations_mut().reserve_nominal_type();
    let generic = program
        .declarations_mut()
        .add_generic_parameter(GenericParameter::new(
            GenericOwner::NominalType(nominal),
            parameter_name,
            0,
        ));
    let generic_type = program
        .types_mut()
        .intern(TypeKind::GenericParameter(generic))
        .unwrap();
    let field = program.declarations_mut().add_field(FieldDeclaration::new(
        field_site,
        nominal,
        field_name,
        generic_type,
    ));
    program
        .declarations_mut()
        .define_nominal_type(
            nominal,
            NominalTypeDeclaration::new(
                type_site,
                box_name,
                [generic],
                [],
                NominalShape::Struct {
                    copy_declared: false,
                    fields: Box::new([field]),
                },
                None,
            ),
        )
        .unwrap();

    let program = program.finish().unwrap();
    assert_eq!(program.declarations().nominal_types().len(), 1);
    assert_eq!(program.declarations().generic_parameters().len(), 1);
    assert_eq!(program.declarations().fields().len(), 1);
}

#[test]
fn orphaned_members_cannot_enter_the_immutable_program() {
    let symbols = SymbolTable::from_spellings(["app", "Box", "value"]);
    let app_name = symbols.get("app").unwrap();
    let box_name = symbols.get("Box").unwrap();
    let field_name = symbols.get("value").unwrap();
    let mut program =
        DeclarationProgramBuilder::new(nocter_model::CompilationTarget::Arm64Darwin, symbols);
    let package = program
        .add_package(PackageIdentity::new("workspace:app"), app_name)
        .unwrap();
    let module = program.add_module(package, ModulePath::root()).unwrap();
    program
        .define_module_namespace(module, ModuleNamespace::default())
        .unwrap();
    let site = program
        .add_declaration_site(module, Visibility::Private)
        .unwrap();
    let field_type = program.types_mut().builtin(BuiltinType::I32);
    let nominal = program.declarations_mut().reserve_nominal_type();
    program
        .declarations_mut()
        .add_field(FieldDeclaration::new(site, nominal, field_name, field_type));
    program
        .declarations_mut()
        .define_nominal_type(
            nominal,
            NominalTypeDeclaration::new(
                site,
                box_name,
                [],
                [],
                NominalShape::Struct {
                    copy_declared: false,
                    fields: Box::new([]),
                },
                None,
            ),
        )
        .unwrap();

    assert_eq!(
        program.finish().unwrap_err(),
        ProgramBuildError::InvalidProgram(ProgramValidationError::Integrity(
            ProgramIntegrityError::OwnerMismatch(DeclarationDomain::Field)
        ))
    );
}

#[test]
fn empty_enums_cannot_enter_the_immutable_program() {
    let symbols = SymbolTable::from_spellings(["app", "Empty"]);
    let app_name = symbols.get("app").unwrap();
    let empty_name = symbols.get("Empty").unwrap();
    let mut program =
        DeclarationProgramBuilder::new(nocter_model::CompilationTarget::Arm64Darwin, symbols);
    let package = program
        .add_package(PackageIdentity::new("workspace:app"), app_name)
        .unwrap();
    let module = program.add_module(package, ModulePath::root()).unwrap();
    program
        .define_module_namespace(module, ModuleNamespace::default())
        .unwrap();
    let site = program
        .add_declaration_site(module, Visibility::Private)
        .unwrap();
    let nominal = program.declarations_mut().reserve_nominal_type();
    program
        .declarations_mut()
        .define_nominal_type(
            nominal,
            NominalTypeDeclaration::new(
                site,
                empty_name,
                [],
                [],
                NominalShape::Enum {
                    variants: Box::new([]),
                },
                None,
            ),
        )
        .unwrap();

    assert_eq!(
        program.finish().unwrap_err(),
        ProgramBuildError::InvalidProgram(ProgramValidationError::Declaration(
            DeclarationViolation::new(DeclarationRule::EmptyEnum, site)
        ))
    );
}

#[test]
fn method_provenance_can_name_the_receiver_without_forging_a_parameter_position() {
    let symbols = SymbolTable::from_spellings(["app", "Buffer", "self", "view"]);
    let app_name = symbols.get("app").unwrap();
    let buffer_name = symbols.get("Buffer").unwrap();
    let self_name = symbols.get("self").unwrap();
    let method_name = symbols.get("view").unwrap();
    let mut program =
        DeclarationProgramBuilder::new(nocter_model::CompilationTarget::Arm64Darwin, symbols);
    let package = program
        .add_package(PackageIdentity::new("workspace:app"), app_name)
        .unwrap();
    let module = program.add_module(package, ModulePath::root()).unwrap();
    program
        .define_module_namespace(module, ModuleNamespace::default())
        .unwrap();
    let site = program
        .add_declaration_site(module, Visibility::Private)
        .unwrap();
    let nominal = program.declarations_mut().reserve_nominal_type();
    let nominal_type = program
        .types_mut()
        .intern(TypeKind::Nominal {
            definition: nominal,
            arguments: Box::new([]),
        })
        .unwrap();
    let result = program.types_mut().builtin(BuiltinType::Usize);
    let instance = program.declarations_mut().reserve_instance();
    let callable = program.declarations_mut().reserve_callable();
    let receiver = program.declarations_mut().add_parameter(Parameter::new(
        ParameterOwner::Callable(callable),
        self_name,
        nominal_type,
        ParameterRole::Receiver(CallableCapability::Readonly),
    ));
    let body = program
        .declarations_mut()
        .add_body(Body::new(BodyOwner::Callable(callable)));
    program
        .declarations_mut()
        .define_callable(
            callable,
            CallableDeclaration::new(
                site,
                CallableOwner::Instance(instance),
                CallableKind::Method,
                Some(method_name),
                Some(receiver),
                [],
                [],
                result,
                CallableProvenanceContract::declared(
                    CallableProvenance::from_origins([ProvenanceOrigin::Receiver]).unwrap(),
                ),
                [],
                Some(body),
                None,
            ),
        )
        .unwrap();
    program
        .declarations_mut()
        .define_instance(
            instance,
            InstanceDeclaration::new(site, nominal_type, [], [], [callable]),
        )
        .unwrap();
    program
        .declarations_mut()
        .define_nominal_type(
            nominal,
            NominalTypeDeclaration::new(
                site,
                buffer_name,
                [],
                [],
                NominalShape::Struct {
                    copy_declared: false,
                    fields: Box::new([]),
                },
                None,
            ),
        )
        .unwrap();

    assert!(program.finish().is_ok());
}

#[test]
fn builtin_attachment_authority_uses_exact_selected_module_identity() {
    let build = |attach_from_standard_module: bool| {
        let symbols = SymbolTable::from_spellings(["app", "std", "str"]);
        let app_name = symbols.get("app").unwrap();
        let standard_name = symbols.get("std").unwrap();
        let str_name = symbols.get("str").unwrap();
        let mut program =
            DeclarationProgramBuilder::new(nocter_model::CompilationTarget::Arm64Darwin, symbols);
        let app = program
            .add_package(PackageIdentity::new("workspace:app"), app_name)
            .unwrap();
        let standard = program
            .add_package(PackageIdentity::new("toolchain:std"), standard_name)
            .unwrap();
        let app_str = program
            .add_module(app, ModulePath::from_segments([str_name]))
            .unwrap();
        let standard_str = program
            .add_module(standard, ModulePath::from_segments([str_name]))
            .unwrap();
        program
            .define_module_namespace(app_str, ModuleNamespace::default())
            .unwrap();
        program
            .define_module_namespace(standard_str, ModuleNamespace::default())
            .unwrap();
        program.set_standard_package(standard).unwrap();
        program
            .set_builtin_attachment_module(BuiltinAttachment::Str, standard_str)
            .unwrap();
        let owner = if attach_from_standard_module {
            standard_str
        } else {
            app_str
        };
        let site = program
            .add_declaration_site(owner, Visibility::Private)
            .unwrap();
        let target = program.types().builtin(BuiltinType::Str);
        let instance = program.declarations_mut().reserve_instance();
        program
            .declarations_mut()
            .define_instance(instance, InstanceDeclaration::new(site, target, [], [], []))
            .unwrap();
        program.finish()
    };

    assert!(build(true).is_ok());
    assert!(matches!(
        build(false).unwrap_err(),
        ProgramBuildError::InvalidProgram(ProgramValidationError::Declaration(error))
            if error.rule() == DeclarationRule::InvalidInherentAttachment
    ));
}

#[test]
fn construction_uniqueness_uses_the_target_family_not_local_binder_identity() {
    let symbols = SymbolTable::from_spellings(["app", "Box", "T", "U"]);
    let app_name = symbols.get("app").unwrap();
    let box_name = symbols.get("Box").unwrap();
    let t_name = symbols.get("T").unwrap();
    let u_name = symbols.get("U").unwrap();
    let mut program =
        DeclarationProgramBuilder::new(nocter_model::CompilationTarget::Arm64Darwin, symbols);
    let package = program
        .add_package(PackageIdentity::new("workspace:app"), app_name)
        .unwrap();
    let module = program.add_module(package, ModulePath::root()).unwrap();
    program
        .define_module_namespace(module, ModuleNamespace::default())
        .unwrap();
    let site = program
        .add_declaration_site(module, Visibility::Private)
        .unwrap();
    let nominal = program.declarations_mut().reserve_nominal_type();
    let nominal_parameter =
        program
            .declarations_mut()
            .add_generic_parameter(GenericParameter::new(
                GenericOwner::NominalType(nominal),
                t_name,
                0,
            ));
    program
        .declarations_mut()
        .define_nominal_type(
            nominal,
            NominalTypeDeclaration::new(
                site,
                box_name,
                [nominal_parameter],
                [],
                NominalShape::Struct {
                    copy_declared: false,
                    fields: Box::new([]),
                },
                None,
            ),
        )
        .unwrap();

    for name in [t_name, u_name] {
        let construction = program.declarations_mut().reserve_construction();
        let parameter = program
            .declarations_mut()
            .add_generic_parameter(GenericParameter::new(
                GenericOwner::Construction(construction),
                name,
                0,
            ));
        let argument = program
            .types_mut()
            .intern(TypeKind::GenericParameter(parameter))
            .unwrap();
        let target = program
            .types_mut()
            .intern(TypeKind::Nominal {
                definition: nominal,
                arguments: Box::new([argument]),
            })
            .unwrap();
        program
            .declarations_mut()
            .define_construction(
                construction,
                ConstructionDeclaration::new(site, target, [parameter], [], None),
            )
            .unwrap();
    }

    assert_eq!(
        program.finish().unwrap_err(),
        ProgramBuildError::InvalidProgram(ProgramValidationError::Declaration(
            DeclarationViolation::with_related(DeclarationRule::DuplicateConstruction, site, site,)
        ))
    );
}

#[test]
fn copy_structs_and_payloadless_enums_cannot_own_drop_bodies() {
    for nominal_shape in [0_u8, 1_u8] {
        let symbols = SymbolTable::from_spellings(["app", "Value", "empty", "self"]);
        let app_name = symbols.get("app").unwrap();
        let value_name = symbols.get("Value").unwrap();
        let empty_name = symbols.get("empty").unwrap();
        let self_name = symbols.get("self").unwrap();
        let mut program =
            DeclarationProgramBuilder::new(nocter_model::CompilationTarget::Arm64Darwin, symbols);
        let package = program
            .add_package(PackageIdentity::new("workspace:app"), app_name)
            .unwrap();
        let module = program.add_module(package, ModulePath::root()).unwrap();
        program
            .define_module_namespace(module, ModuleNamespace::default())
            .unwrap();
        let site = program
            .add_declaration_site(module, Visibility::Private)
            .unwrap();
        let nominal = program.declarations_mut().reserve_nominal_type();
        let target = program
            .types_mut()
            .intern(TypeKind::Nominal {
                definition: nominal,
                arguments: Box::new([]),
            })
            .unwrap();
        let shape = if nominal_shape == 0 {
            NominalShape::Struct {
                copy_declared: true,
                fields: Box::new([]),
            }
        } else {
            let variant = program.declarations_mut().reserve_variant();
            program
                .declarations_mut()
                .define_variant(
                    variant,
                    VariantDeclaration::new(site, nominal, empty_name, []),
                )
                .unwrap();
            NominalShape::Enum {
                variants: Box::new([variant]),
            }
        };
        program
            .declarations_mut()
            .define_nominal_type(
                nominal,
                NominalTypeDeclaration::new(site, value_name, [], [], shape, None),
            )
            .unwrap();
        let drop = program.declarations_mut().reserve_drop();
        let receiver = program.declarations_mut().add_parameter(Parameter::new(
            ParameterOwner::Drop(drop),
            self_name,
            target,
            ParameterRole::Receiver(CallableCapability::ReadWrite),
        ));
        let body = program
            .declarations_mut()
            .add_body(Body::new(BodyOwner::Drop(drop)));
        program
            .declarations_mut()
            .define_drop(drop, DropDeclaration::new(site, target, [], receiver, body))
            .unwrap();

        assert_eq!(
            program.finish().unwrap_err(),
            ProgramBuildError::InvalidProgram(ProgramValidationError::Declaration(
                DeclarationViolation::with_related(
                    if nominal_shape == 0 {
                        DeclarationRule::CopyDrop
                    } else {
                        DeclarationRule::PayloadlessEnumDrop
                    },
                    site,
                    site,
                )
            ))
        );
    }
}
