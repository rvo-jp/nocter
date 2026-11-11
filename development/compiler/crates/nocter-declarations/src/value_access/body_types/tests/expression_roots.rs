use super::*;
use nocter_model::{
    AccessExpressionId, ArenaBuilder, ValueClosure, ValueCodeRoot, ValueExpression,
};
use std::collections::BTreeSet;

fn headers() -> (Program, AccessCodeId, AccessCodeId, AccessCodeId) {
    let symbols = SymbolTable::from_spellings(["owner", "other", "factory"]);
    let mut reservations = Reservations::new(symbols.clone());
    let owner = reservations.reserve(symbols.get("owner").unwrap(), 0);
    let other = reservations.reserve(symbols.get("other").unwrap(), 0);
    let factory = reservations.reserve(symbols.get("factory").unwrap(), 0);
    let mut builder = reservations.finish().unwrap();
    let kind = CodeKind::Function(CompileTimeGuarantee::RuntimeOnly);
    for id in [owner, other] {
        builder.define(id, kind, [], BuiltinType::Void).unwrap();
    }
    builder
        .define(
            factory,
            kind,
            [],
            ValueDefinition::Function(FunctionDefinition {
                invocation: ValueInvocation::Read,
                inputs: Box::new([]),
                result: BuiltinType::Void.into(),
            }),
        )
        .unwrap();
    (builder.finish().unwrap(), owner, other, factory)
}

#[test]
fn expressions_share_source_ownership_without_sharing_type_or_application_branches() {
    let (header, owner, other, factory) = headers();
    let roots = [
        ValueCodeRoot::Declaration(owner),
        ValueCodeRoot::Expression(ValueExpression { owner, slot: 0 }),
        ValueCodeRoot::Expression(ValueExpression { owner, slot: 1 }),
        ValueCodeRoot::Expression(ValueExpression {
            owner: other,
            slot: 0,
        }),
    ];
    let call = ArenaBuilder::<AccessExpressionId, ()>::new().insert(());
    let integer = header.data_type(BuiltinType::I32).unwrap();
    let unit = header.data_type(BuiltinType::Void).unwrap();
    let mut branches = BTreeMap::new();
    for root in roots {
        assert_eq!(root.code().root(), root);
        let mut body = header.body_types(root).unwrap();
        assert_eq!(body.owner(), root);
        let array = body.array(integer, 3).unwrap();
        let application = body.application(root.code(), call, factory).unwrap();
        let closure = ValueClosure {
            owner: root,
            slot: 0,
        };
        let capture = body
            .closure(
                closure,
                array,
                FunctionConstraint {
                    invocation: ValueInvocation::Read,
                    inputs: Box::new([]),
                    result: unit,
                },
            )
            .unwrap();
        branches.insert(root, (body.finish(), array, application, capture, closure));
    }
    assert_eq!(roots[0].source_owner(), roots[1].source_owner());
    assert_eq!(roots[1].source_owner(), roots[2].source_owner());
    assert_eq!(roots[1].code().origin(), None);
    assert_ne!(roots[1].code(), roots[2].code());
    assert!(matches!(
        header.canonical_body_types(BTreeMap::from([(roots[1], &branches[&roots[2]].0),])),
        Err(BodyTypeError::ForeignDeclarations)
    ));
    let canonical = header
        .canonical_body_types(
            branches
                .iter()
                .map(|(&root, (body, ..))| (root, body))
                .collect(),
        )
        .unwrap();
    let mut arrays = BTreeSet::new();
    let mut applications = BTreeSet::new();
    let mut captures = BTreeSet::new();
    for (&root, (_, array, application, capture, _)) in &branches {
        arrays.insert(canonical.project(root, *array).unwrap());
        applications.insert(canonical.project(root, *application).unwrap());
        captures.insert(canonical.project(root, *capture).unwrap());
    }
    assert_eq!(
        arrays.len(),
        1,
        "structural data is independent of its code root"
    );
    assert_eq!(
        applications.len(),
        roots.len(),
        "application identities include the complete root"
    );
    assert_eq!(
        captures.len(),
        roots.len(),
        "closure identities include the complete root"
    );
    let canonical = canonical.finish();
    assert_eq!(canonical.codes().len(), header.codes().len());
    for (_, _, _, _, closure) in branches.values() {
        assert!(canonical.closure(*closure).is_some());
    }
}

#[test]
fn an_expression_branch_does_not_inherit_its_unready_parent_contract() {
    let symbols = SymbolTable::from_spellings(["owner", "factory"]);
    let mut reservations = Reservations::new(symbols.clone());
    let owner = reservations.reserve(symbols.get("owner").unwrap(), 0);
    let factory = reservations.reserve(symbols.get("factory").unwrap(), 0);
    let mut headers = reservations.finish().unwrap().into_publication();
    headers
        .define(
            factory,
            CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [],
            ValueDefinition::Function(FunctionDefinition {
                invocation: ValueInvocation::Read,
                inputs: Box::new([]),
                result: BuiltinType::Void.into(),
            }),
        )
        .unwrap();
    headers.publish(factory).unwrap();
    let early = headers.snapshot();
    let root = ValueCodeRoot::Expression(ValueExpression { owner, slot: 0 });
    assert!(early.codes().get(owner).is_none());
    assert!(early.body_types(owner).is_err());
    assert_eq!(
        early.root_label(root).as_deref(),
        Some("expression 0 in owner")
    );
    let mut branch = early.body_types(root).unwrap();
    let boolean = early.data_type(BuiltinType::Bool).unwrap();
    let array = branch.array(boolean, 3).unwrap();
    let call = ArenaBuilder::<AccessExpressionId, ()>::new().insert(());
    let application = branch.application(root.code(), call, factory).unwrap();
    let branch = branch.finish();
    assert!(matches!(
        branch.data_schema(application),
        Some(PayloadSchema::Function(_))
    ));
    let mut unknown = ArenaBuilder::<AccessCodeId, ()>::new();
    for _ in 0..2 {
        unknown.insert(());
    }
    let unknown = unknown.insert(());
    assert!(
        early
            .body_types(ValueCodeRoot::Expression(ValueExpression {
                owner: unknown,
                slot: 0
            }))
            .is_err()
    );
    headers
        .define(
            owner,
            CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [],
            BuiltinType::Void,
        )
        .unwrap();
    let late = headers.finish().unwrap();
    let canonical = late
        .canonical_body_types(BTreeMap::from([(root, &branch)]))
        .unwrap();
    let array = canonical.project(root, array).unwrap();
    let canonical = canonical.finish();
    let Some(PayloadSchema::Array(array)) = canonical.data_schema(array) else {
        panic!("replayed independent expression array");
    };
    assert_eq!(array.element(), boolean);
    assert_eq!(array.length(), 3);
}
