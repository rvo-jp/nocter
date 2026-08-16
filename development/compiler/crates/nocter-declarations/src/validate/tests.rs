use nocter_model::{BuiltinType, CallableCapability, SymbolTable, TypeKind};

use crate::{
    Body, BodyOwner, CallableDeclaration, CallableKind, CallableOwner, CallableProvenance,
    DeclarationDomain, DeclarationProgramBuilder, FieldDeclaration, GenericOwner, GenericParameter,
    InstanceDeclaration, ModulePath, NominalShape, NominalTypeDeclaration, Parameter,
    ParameterOwner, ParameterRole, ProgramBuildError, ProgramIntegrityError, ProvenanceOrigin,
    Visibility,
};

#[test]
fn two_pass_definitions_support_recursive_header_identity() {
    let symbols = SymbolTable::from_spellings(["app", "Box", "T", "value"]);
    let app_name = symbols.get("app").unwrap();
    let box_name = symbols.get("Box").unwrap();
    let parameter_name = symbols.get("T").unwrap();
    let field_name = symbols.get("value").unwrap();
    let mut program = DeclarationProgramBuilder::new(symbols);
    let package = program.add_package(app_name).unwrap();
    let module = program.add_module(package, ModulePath::root()).unwrap();
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
    let mut program = DeclarationProgramBuilder::new(symbols);
    let package = program.add_package(app_name).unwrap();
    let module = program.add_module(package, ModulePath::root()).unwrap();
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
        ProgramBuildError::InvalidProgram(ProgramIntegrityError::OwnerMismatch(
            DeclarationDomain::Field,
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
    let mut program = DeclarationProgramBuilder::new(symbols);
    let package = program.add_package(app_name).unwrap();
    let module = program.add_module(package, ModulePath::root()).unwrap();
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
                CallableProvenance::from_origins([ProvenanceOrigin::Receiver]).unwrap(),
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
