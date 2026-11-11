//! Header ancestry and body-owned suffixes preserve one canonical type authority.
use super::super::*;
use nocter_model::ValueInvocation;

mod arrays;
mod expression_roots;

#[test]
fn a_late_record_field_type_cannot_reinterpret_an_early_body_tuple() {
    let symbols = SymbolTable::from_spellings(["Late", "value", "run"]);
    let mut reservations = Reservations::new(symbols.clone());
    let record = reservations.reserve_record(symbols.get("Late").unwrap(), 1);
    let run = reservations.reserve(symbols.get("run").unwrap(), 0);
    let mut builder = reservations.finish().unwrap();
    builder
        .define_record(
            record,
            false,
            [FieldDefinition {
                name: symbols.get("value").unwrap(),
                data: DataReference::Tuple(TupleData::new(
                    BuiltinType::Bool.into(),
                    BuiltinType::Bool.into(),
                    [],
                )),
            }],
        )
        .unwrap();
    builder
        .define(
            run,
            CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [],
            BuiltinType::Void,
        )
        .unwrap();
    let mut headers = builder.into_publication();
    headers.publish(run).unwrap();
    let early = headers.snapshot();
    let integer = early.data_type(BuiltinType::I32).unwrap();
    let boolean = early.data_type(BuiltinType::Bool).unwrap();
    let mut body = early.body_types(run).unwrap();
    let tuple = body.tuple(&[integer, boolean]).unwrap();
    let body = body.finish();
    let later = headers.finish().unwrap();
    let field = later.records().get(record).unwrap().fields()[0];
    assert_eq!(
        tuple,
        later.fields().get(field).unwrap().data(),
        "the collision must be exercised"
    );
    assert_ne!(body.data_schema(tuple), later.data_schema(tuple));
    let canonical = later
        .canonical_body_types(BTreeMap::from([(run.into(), &body)]))
        .unwrap();
    let projected = canonical.project(run, tuple).unwrap();
    assert_ne!(projected, tuple);
    let canonical = canonical.finish();
    assert_eq!(canonical.data_schema(projected), body.data_schema(tuple));
    assert_eq!(canonical.fields().get(field).unwrap().data(), tuple);
}

#[test]
fn earlier_body_types_replay_past_later_header_keys_at_the_same_raw_position() {
    let symbols = SymbolTable::from_spellings(["alpha", "beta", "action"]);
    let mut reservations = Reservations::new(symbols.clone());
    let alpha = reservations.reserve(symbols.get("alpha").unwrap(), 0);
    let beta = reservations.reserve(symbols.get("beta").unwrap(), 1);
    let mut builder = reservations.finish().unwrap();
    let kind = CodeKind::Function(CompileTimeGuarantee::RuntimeOnly);
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
    let mut publication = builder.into_publication();
    let empty = publication.snapshot();
    assert!(empty.codes().is_empty());
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
    let later = publication.finish().unwrap();
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
    assert!(matches!(
        later.canonical_body_types(BTreeMap::from([(beta.into(), &body)])),
        Err(BodyTypeError::ForeignDeclarations)
    ));
    assert_eq!(
        later.function_data(FunctionWitness {
            origin: nocter_model::ValueFunctionOrigin::Declaration(beta),
            invocation: ValueInvocation::Read,
        }),
        Some(tuple),
        "two branches can use the same raw TypeId for different kinds"
    );
    assert!(matches!(
        later.data_schema(tuple),
        Some(PayloadSchema::Function(_))
    ));
    let canonical = later
        .canonical_body_types(BTreeMap::from([(alpha.into(), &body)]))
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
    let (header, [first, second, make]) = application_headers();
    let call = ArenaBuilder::<AccessExpressionId, ()>::new().insert(());
    let mut a = header.body_types(first).unwrap();
    let da = a
        .application(nocter_model::ValueCode::Declaration(first), call, make)
        .unwrap();
    assert_eq!(
        a.application(nocter_model::ValueCode::Declaration(first), call, make)
            .unwrap(),
        da
    );
    assert!(
        a.application(nocter_model::ValueCode::Declaration(first), call, second)
            .is_err()
    );
    let a = a.finish();
    let mut b = header.body_types(second).unwrap();
    let db = b
        .application(nocter_model::ValueCode::Declaration(second), call, make)
        .unwrap();
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
            (second.into(), &b),
            (first.into(), &a),
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
            .application(nocter_model::ValueCode::Declaration(first), call, first)
            .is_err()
    );
    assert!(
        header
            .canonical_body_types(std::collections::BTreeMap::from([(first.into(), &b)]))
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
            crate::value_access::CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [],
            BuiltinType::Void,
        )
        .unwrap();
    builder
        .define(
            second,
            crate::value_access::CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
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
    let (a, da, ka) = make(first.into(), BuiltinType::Error);
    let (b, db, kb) = make(second.into(), BuiltinType::I32);
    assert_eq!(da, db);
    assert!(header.closure(ka).is_none());
    let canonical = header
        .canonical_body_types(std::collections::BTreeMap::from([
            (first.into(), &a),
            (second.into(), &b),
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
            .canonical_body_types(std::collections::BTreeMap::from([(first.into(), &b)]))
            .is_err()
    );
}

fn application_headers() -> (Program, [nocter_model::AccessCodeId; 3]) {
    let symbols = SymbolTable::from_spellings(["first", "second", "make"]);
    let mut reservations = Reservations::new(symbols.clone());
    let first = reservations.reserve(symbols.get("first").unwrap(), 0);
    let second = reservations.reserve(symbols.get("second").unwrap(), 0);
    let make = reservations.reserve(symbols.get("make").unwrap(), 0);
    let mut builder = reservations.finish().unwrap();
    builder
        .define(
            first,
            crate::value_access::CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [],
            BuiltinType::Void,
        )
        .unwrap();
    builder
        .define(
            second,
            crate::value_access::CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
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
            crate::value_access::CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [],
            ValueDefinition::Function(FunctionDefinition {
                invocation: ValueInvocation::Read,
                inputs: Box::new([]),
                result: BuiltinType::Void.into(),
            }),
        )
        .unwrap();
    (builder.finish().unwrap(), [first, second, make])
}

#[test]
fn lexical_code_owns_application_reservations_within_one_type_root() {
    let (header, [first, second, make]) = application_headers();
    let call = nocter_model::ArenaBuilder::<nocter_model::AccessExpressionId, ()>::new().insert(());
    let mut a = header.body_types(first).unwrap();
    let da = a
        .application(nocter_model::ValueCode::Declaration(first), call, make)
        .unwrap();
    let void = header.data_type(BuiltinType::Void).unwrap();
    let mut nested = Vec::new();
    for (slot, callee) in [(0, make), (1, second)] {
        let id = nocter_model::ValueClosure {
            owner: first.into(),
            slot,
        };
        a.closure(
            id,
            void,
            FunctionConstraint {
                invocation: ValueInvocation::Read,
                inputs: Box::new([]),
                result: void,
            },
        )
        .unwrap();
        nested.push(
            a.application(nocter_model::ValueCode::Closure(id), call, callee)
                .unwrap(),
        );
    }
    assert_ne!(
        nested[0], da,
        "a closure's operation is separate from its root's operation"
    );
    assert_ne!(
        nested[0], nested[1],
        "sibling code has independent call/callee reservations"
    );
    assert!(
        a.application(nocter_model::ValueCode::Declaration(second), call, make)
            .is_err()
    );
    let a = a.finish();
    header
        .canonical_body_types(std::collections::BTreeMap::from([(first.into(), &a)]))
        .unwrap();
}

#[test]
fn pending_closure_reservations_survive_child_publication_and_key_exhaustion() {
    let (header, [root, _, _]) = application_headers();
    let mut parent = header.body_types(root).unwrap();
    let outer = parent.reserve_closure().unwrap();
    let mut child = parent.clone();
    let inner = child.reserve_closure().unwrap();
    assert_ne!(outer, inner);
    let void = header.data_type(BuiltinType::Void).unwrap();
    let contract = FunctionConstraint {
        invocation: ValueInvocation::Read,
        inputs: Box::new([]),
        result: void,
    };
    child.closure(inner, void, contract.clone()).unwrap();
    parent = child;
    let last = nocter_model::ValueClosure {
        owner: root.into(),
        slot: u32::MAX,
    };
    parent.closure(last, void, contract.clone()).unwrap();
    assert_eq!(
        parent.reserve_closure(),
        Err(super::BodyTypeError::InvalidExtension)
    );
    parent.closure(outer, void, contract).unwrap();
    let body = parent.finish();
    assert_eq!(body.closures.len(), 3);
    header
        .canonical_body_types(std::collections::BTreeMap::from([(root.into(), &body)]))
        .unwrap();
}

#[test]
fn callable_capture_payloads_require_exact_witnesses_and_failed_publication_is_atomic() {
    let (header, [root, factory, _]) = application_headers();
    let named = header
        .function_data(FunctionWitness {
            origin: nocter_model::ValueFunctionOrigin::Declaration(root),
            invocation: ValueInvocation::Read,
        })
        .unwrap();
    assert!(header.capture_environment_supported(named));
    assert!(!header.capture_environment_supported(header.codes().get(factory).unwrap().result()));
    let mut body = header.body_types(root).unwrap();
    let call = nocter_model::ArenaBuilder::<nocter_model::AccessExpressionId, ()>::new().insert(());
    let pending = body
        .application(nocter_model::ValueCode::Declaration(root), call, factory)
        .unwrap();
    assert!(!body.program().capture_environment_supported(pending));
    let id = body.reserve_closure().unwrap();
    let contract = FunctionConstraint {
        invocation: ValueInvocation::Read,
        inputs: Box::new([]),
        result: header.data_type(BuiltinType::Void).unwrap(),
    };
    assert_eq!(
        body.closure(id, pending, contract.clone()),
        Err(super::BodyTypeError::InvalidExtension)
    );
    assert!(body.program().closure(id).is_none());
    body.closure(id, named, contract).unwrap();
}
