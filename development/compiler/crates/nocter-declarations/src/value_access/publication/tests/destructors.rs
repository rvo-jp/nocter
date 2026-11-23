//! Lifecycle headers own their implicit receiver and depend on the published nominal policy.
use super::*;
use crate::{
    DefinitionError,
    value_access::{BehaviorOwner, HeaderPosition, ParameterBinding, ScopeId, VariantDefinition},
};
use nocter_model::{ValueField, ValueInputOperation, ValueInvocation};

#[test]
fn destructor_requires_one_move_only_owner_and_has_no_callable_binding() {
    let symbols = SymbolTable::from_spellings(["Handle", "self"]);
    let mut reservations = Reservations::new(symbols.clone());
    let owner = reservations.reserve_record(symbols.get("Handle").unwrap(), 0);
    let scope = reservations
        .new_behavior_scope(ScopeId::ROOT, BehaviorOwner::Nominal(owner))
        .unwrap();
    assert_eq!(
        reservations.reserve_destructor(ScopeId::ROOT, owner),
        Err(DefinitionError::UnknownId)
    );
    let destructor = reservations.reserve_destructor(scope, owner).unwrap();
    let other_scope = reservations
        .new_behavior_scope(ScopeId::ROOT, BehaviorOwner::Nominal(owner))
        .unwrap();
    assert_eq!(
        reservations.reserve_destructor(other_scope, owner),
        Err(DefinitionError::AlreadyDefined)
    );
    let mut builder = reservations.finish().unwrap();
    builder.define_record(owner, false, []).unwrap();
    let mut publication = builder.into_publication();
    assert_eq!(
        publication.advance(destructor).unwrap(),
        HeaderStep::Require(Box::new([owner.into()]))
    );
    assert_eq!(publication.snapshot().destructor(owner), None);
    assert_eq!(
        publication.snapshot().reserved_destructor(owner),
        Some(destructor)
    );
    publication.publish(destructor).unwrap();
    let program = publication.snapshot();
    assert_eq!(program.destructor(owner), Some(destructor));
    let code = program.codes().get(destructor).unwrap();
    assert_eq!(code.kind(), CodeKind::Destructor(owner));
    assert_eq!(code.name(), None);
    assert_eq!(Some(code.result()), program.data_type(BuiltinType::Void));
    assert_eq!(program.function_data(destructor.into()), None);
    let &[parameter] = code.parameters() else {
        panic!("one implicit receiver")
    };
    let parameter = program.parameters().get(parameter).unwrap();
    assert_eq!(
        parameter.binding(),
        &ParameterBinding::Name(symbols.get("self").unwrap())
    );
    assert_eq!(parameter.operation(), ValueInputOperation::Edit);
    assert_eq!(program.nominal_identity(parameter.data()), Some(owner));
    let receiver = program.instantiate_closed_data(parameter.data()).unwrap();
    assert_eq!(
        program.bind_destructor(&receiver),
        program.bind_code(destructor.into(), [])
    );
}

#[test]
fn destructor_frame_retains_the_context_of_its_exact_receiver() {
    use crate::value_access::{FieldDefinition, FunctionDefinition, FunctionWitness};
    use nocter_model::{CompileTimeGuarantee, ValueFunctionOrigin};
    let symbols = SymbolTable::from_spellings(["Handler", "first", "second", "callback", "self"]);
    let mut reservations = Reservations::new(symbols.clone());
    let owner = reservations.reserve_record(symbols.get("Handler").unwrap(), 1);
    let scope = reservations
        .new_behavior_scope(ScopeId::ROOT, BehaviorOwner::Nominal(owner))
        .unwrap();
    let destructor = reservations.reserve_destructor(scope, owner).unwrap();
    let functions =
        ["first", "second"].map(|name| reservations.reserve(symbols.get(name).unwrap(), 0));
    let mut declarations = reservations.finish().unwrap();
    for function in functions {
        declarations
            .define(
                function,
                CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
                [],
                BuiltinType::I32,
            )
            .unwrap();
    }
    declarations
        .define_record(
            owner,
            false,
            [FieldDefinition {
                name: symbols.get("callback").unwrap(),
                data: DataReference::Function(Box::new(FunctionDefinition {
                    invocation: ValueInvocation::Read,
                    inputs: Box::new([]),
                    result: BuiltinType::I32.into(),
                })),
            }],
        )
        .unwrap();
    let program = declarations.finish().unwrap();
    let nominal = program.record(owner).unwrap();
    let field = nominal.fields()[0];
    let field_data = program.fields().get(field).unwrap().data();
    let frames = functions.map(|function| {
        let witness = program
            .concrete_witness(FunctionWitness {
                origin: ValueFunctionOrigin::Declaration(function),
                invocation: ValueInvocation::Read,
            })
            .unwrap();
        let component = program.instantiate_function(field_data, &witness).unwrap();
        let receiver = program
            .instantiate_components(nominal.data(), [(ValueField::Record(field), component)])
            .unwrap();
        let frame = program.bind_destructor(&receiver).unwrap();
        assert_eq!(frame.code(), destructor.into());
        assert_eq!(frame.input_data(0), Some(&receiver));
        frame
    });
    assert_ne!(frames[0], frames[1]);
    let uninhabited = program
        .instantiate_uninhabited_data(nominal.data())
        .unwrap();
    assert!(program.bind_destructor(&uninhabited).is_err());
}

#[test]
fn destructor_publication_rejects_the_effective_copy_policy() {
    let symbols = SymbolTable::from_spellings(["Entry", "empty", "self"]);
    for scalar in [false, true] {
        let mut reservations = Reservations::new(symbols.clone());
        let name = symbols.get("Entry").unwrap();
        let owner = if scalar {
            reservations
                .reserve_enum_in(ScopeId::ROOT, name, [0])
                .unwrap()
        } else {
            reservations.reserve_record(name, 0)
        };
        let scope = reservations
            .new_behavior_scope(ScopeId::ROOT, BehaviorOwner::Nominal(owner))
            .unwrap();
        let destructor = reservations.reserve_destructor(scope, owner).unwrap();
        let mut builder = reservations.finish().unwrap();
        if scalar {
            builder
                .define_enum(
                    owner,
                    true,
                    [VariantDefinition::scalar(symbols.get("empty").unwrap())],
                )
                .unwrap();
        } else {
            builder.define_record(owner, true, []).unwrap();
        }
        let mut publication = builder.into_publication();
        assert_eq!(
            publication.publish(destructor),
            Err(HeaderFailure::Rejected(vec![DeclarationViolation {
                declaration: destructor.into(),
                position: HeaderPosition::Name,
                rule: DeclarationRule::DestructorOnCopy,
            }]))
        );
        assert_eq!(publication.snapshot().destructor(owner), None);
        assert!(
            publication
                .snapshot()
                .nominals()
                .get(owner)
                .unwrap()
                .copyable()
        );
    }
}

#[test]
fn reserved_layout_tracks_the_nominal_owner_of_an_anonymous_destructor() {
    let symbols = SymbolTable::from_spellings(["First", "Second", "self"]);
    let prepare = |selected: usize| {
        let mut reservations = Reservations::new(symbols.clone());
        let owners = ["First", "Second"]
            .map(|name| reservations.reserve_record(symbols.get(name).unwrap(), 0));
        let scopes = owners.map(|owner| {
            reservations
                .new_behavior_scope(ScopeId::ROOT, BehaviorOwner::Nominal(owner))
                .unwrap()
        });
        reservations
            .reserve_destructor(scopes[selected], owners[selected])
            .unwrap();
        let mut builder = reservations.finish().unwrap();
        for owner in owners {
            builder.define_record(owner, false, []).unwrap();
        }
        builder.into_publication()
    };
    let previous = prepare(0).finish().unwrap();
    assert!(prepare(0).has_same_reserved_layout(&previous));
    assert!(!prepare(1).has_same_reserved_layout(&previous));
}
