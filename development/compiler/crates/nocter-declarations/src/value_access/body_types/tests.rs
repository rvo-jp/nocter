//! Header ancestry and body-owned suffixes preserve one canonical type authority.
use super::super::*;
use nocter_model::ValueInvocation;

#[test]
fn earlier_body_types_replay_past_later_header_keys_at_the_same_raw_position() {
    let symbols = SymbolTable::from_spellings(["alpha", "beta", "action"]);
    let mut reservations = Reservations::new(symbols.clone());
    let alpha = reservations.reserve(symbols.get("alpha").unwrap(), 0);
    let beta = reservations.reserve(symbols.get("beta").unwrap(), 1);
    let mut builder = reservations.finish().unwrap();
    let kind = CallableKind::Function(CompileTimeGuarantee::RuntimeOnly);
    builder.define(alpha, kind, [], BuiltinType::I32).unwrap();
    builder
        .define(
            beta,
            kind,
            [ParameterDefinition {
                name: symbols.get("action").unwrap(),
                operation: ValueInputOperation::Read,
                data: ValueDefinition::Function(FunctionDefinition {
                    invocation: ValueInvocation::Read,
                    inputs: Box::new([]),
                    result: BuiltinType::Void.into(),
                }),
            }],
            BuiltinType::Void,
        )
        .unwrap();
    let mut publication = builder.into_publication().unwrap();
    let empty = publication.snapshot();
    assert!(empty.callables().is_empty());
    assert_eq!(empty.resolve(symbols.get("alpha").unwrap()), None);
    assert_eq!(
        empty.function_data(FunctionWitness {
            origin: nocter_model::ValueFunctionOrigin::Declaration(alpha),
            invocation: ValueInvocation::Read
        }),
        None
    );
    publication.publish(alpha).unwrap();
    let early = publication.snapshot();
    assert!(!early.headers_complete());
    assert!(early.body_types(beta).is_err());
    let mut body = early.body_types(alpha).unwrap();
    let tuple = body
        .tuple(&[
            early.data_type(BuiltinType::I32).unwrap(),
            early.data_type(BuiltinType::Bool).unwrap(),
        ])
        .unwrap();
    let body = body.finish();
    publication.publish(beta).unwrap();
    let later = publication.finish();
    assert!(later.headers_complete());
    assert!(later.preserves_headers(&early));
    assert!(!early.preserves_headers(&later));
    let empty_body = later.body_types(alpha).unwrap().finish();
    assert!(empty_body.body_types(beta).is_err());
    assert!(empty_body.canonical_body_types(BTreeMap::new()).is_err());
    let empty_canonical = later
        .canonical_body_types(BTreeMap::new())
        .unwrap()
        .finish();
    assert!(empty_canonical.body_types(alpha).is_err());
    let parameter = later.callables().get(beta).unwrap().parameters()[0];
    assert!(matches!(
        later.canonical_body_types(BTreeMap::from([(beta, &body)])),
        Err(BodyTypeError::ForeignDeclarations)
    ));
    assert_eq!(
        later.parameters().get(parameter).unwrap().data(),
        tuple,
        "two branches can use the same raw TypeId for different kinds"
    );
    assert!(matches!(
        later.data_schema(tuple),
        Some(PayloadSchema::Function(_))
    ));
    let canonical = later
        .canonical_body_types(BTreeMap::from([(alpha, &body)]))
        .unwrap();
    let projected = canonical.project(alpha, tuple).unwrap();
    assert_ne!(projected, tuple);
    let canonical = canonical.finish();
    let Some(PayloadSchema::Aggregate(schema)) = canonical.data_schema(projected) else {
        panic!("replayed tuple");
    };
    assert_eq!(
        schema
            .components()
            .map(|c| canonical.data_kind(c.data).unwrap())
            .collect::<Vec<_>>(),
        [BuiltinType::I32, BuiltinType::Bool]
    );
}

#[test]
fn application_suffixes_replay_by_body_owner_without_changing_the_header() {
    use nocter_model::{AccessExpressionId, ArenaBuilder, ValueFunctionOrigin as Origin};
    let symbols = SymbolTable::from_spellings(["first", "second", "make"]);
    let mut reservations = Reservations::new(symbols.clone());
    let first = reservations.reserve(symbols.get("first").unwrap(), 0);
    let second = reservations.reserve(symbols.get("second").unwrap(), 0);
    let make = reservations.reserve(symbols.get("make").unwrap(), 0);
    let mut builder = reservations.finish().unwrap();
    builder
        .define(
            first,
            crate::value_access::CallableKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [],
            BuiltinType::Void,
        )
        .unwrap();
    builder
        .define(
            second,
            crate::value_access::CallableKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [],
            ValueDefinition::Function(FunctionDefinition {
                invocation: ValueInvocation::Read,
                inputs: Box::new([]),
                result: BuiltinType::Void.into(),
            }),
        )
        .unwrap();
    builder
        .define(
            make,
            crate::value_access::CallableKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [],
            ValueDefinition::Function(FunctionDefinition {
                invocation: ValueInvocation::Read,
                inputs: Box::new([]),
                result: BuiltinType::Void.into(),
            }),
        )
        .unwrap();
    let header = builder.finish().unwrap();
    let call = ArenaBuilder::<AccessExpressionId, ()>::new().insert(());
    let mut a = header.body_types(first).unwrap();
    let da = a.application(call, make).unwrap();
    assert_eq!(a.application(call, make).unwrap(), da);
    assert!(a.application(call, second).is_err());
    let a = a.finish();
    let mut b = header.body_types(second).unwrap();
    let db = b.application(call, make).unwrap();
    let b = b.finish();
    assert_eq!(da, db, "branch-local suffix positions may coincide");
    let Some(PayloadSchema::Function(wa)) = a.data_schema(da) else {
        panic!("application")
    };
    assert!(matches!(wa.origin, Origin::Application(_)));
    assert!(header.function_data(wa).is_none());
    assert_eq!(a.data_copyable(da), Some(false));
    assert_eq!(a.types.store().is_concrete(da), Some(false));
    assert!(a.types.store().may_carry_storage(da));
    let canonical = header
        .canonical_body_types(std::collections::BTreeMap::from([
            (second, &b),
            (first, &a),
        ]))
        .unwrap();
    let ca = canonical.project(first, da).unwrap();
    let cb = canonical.project(second, db).unwrap();
    assert_ne!(ca, cb);
    let canonical = canonical.finish();
    assert_eq!(
        canonical.data_schema(ca).unwrap(),
        a.data_schema(da).unwrap()
    );
    assert_eq!(
        canonical.data_schema(cb).unwrap(),
        b.data_schema(db).unwrap()
    );
    assert!(canonical.body_types(first).is_err());
    assert!(
        header
            .body_types(first)
            .unwrap()
            .application(call, first)
            .is_err()
    );
    assert!(
        header
            .canonical_body_types(std::collections::BTreeMap::from([(first, &b)]))
            .is_err()
    );
}

#[test]
fn owned_closure_schemas_replay_with_their_own_body_and_preserve_capture_copy_evidence() {
    use nocter_model::{ValueClosure, ValueFunctionOrigin as Origin};
    let symbols = SymbolTable::from_spellings(["first", "second"]);
    let mut reservations = Reservations::new(symbols.clone());
    let first = reservations.reserve(symbols.get("first").unwrap(), 0);
    let second = reservations.reserve(symbols.get("second").unwrap(), 0);
    let mut builder = reservations.finish().unwrap();
    builder
        .define(
            first,
            crate::value_access::CallableKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [],
            BuiltinType::Void,
        )
        .unwrap();
    builder
        .define(
            second,
            crate::value_access::CallableKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [],
            BuiltinType::Void,
        )
        .unwrap();
    let header = builder.finish().unwrap();
    let make = |owner, environment| {
        let mut branch = header.body_types(owner).unwrap();
        let key = ValueClosure { owner, slot: 0 };
        let contract = FunctionConstraint {
            invocation: ValueInvocation::Read,
            inputs: Box::new([]),
            result: header.data_type(BuiltinType::Void).unwrap(),
        };
        let data = branch
            .closure(
                key,
                header.data_type(environment).unwrap(),
                contract.clone(),
            )
            .unwrap();
        assert!(
            branch
                .closure(key, header.data_type(environment).unwrap(), contract)
                .is_err()
        );
        (branch.finish(), data, key)
    };
    let (a, da, ka) = make(first, BuiltinType::Error);
    let (b, db, kb) = make(second, BuiltinType::I32);
    assert_eq!(da, db);
    assert!(header.closure(ka).is_none());
    let canonical = header
        .canonical_body_types(std::collections::BTreeMap::from([
            (first, &a),
            (second, &b),
        ]))
        .unwrap();
    let ca = canonical.project(first, da).unwrap();
    let cb = canonical.project(second, db).unwrap();
    let program = canonical.finish();
    assert_ne!(ca, cb);
    assert_eq!(program.data_copyable(ca), Some(false));
    assert_eq!(program.data_copyable(cb), Some(true));
    assert_eq!(program.closure(ka), a.closure(ka));
    assert_eq!(program.closure(kb), b.closure(kb));
    assert_eq!(
        program.function_data(FunctionWitness {
            origin: Origin::Closure(ka),
            invocation: ValueInvocation::Read
        }),
        Some(ca)
    );
    assert!(
        header
            .canonical_body_types(std::collections::BTreeMap::from([(first, &b)]))
            .is_err()
    );
}
