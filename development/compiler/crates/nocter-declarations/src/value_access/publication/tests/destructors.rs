//! Lifecycle headers own their implicit receiver and depend on the published nominal policy.
use super::*;
use crate::{
    DefinitionError,
    value_access::{BehaviorOwner, HeaderPosition, ParameterBinding, ScopeId, VariantDefinition},
};
use nocter_model::ValueInputOperation;

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
