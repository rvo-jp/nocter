use super::*;
use crate::value_access::{ScopeId, ValueDefinition};
use nocter_model::{ModuleAudience, ModuleIdentity, PackageIdentity, ValueInputOperation};
use std::sync::Arc;

fn publication(
    parameters: [usize; 2],
    fields: [usize; 2],
    audience: ModuleAudience,
) -> HeaderPublication {
    let symbols = SymbolTable::from_spellings([
        "alpha", "beta", "One", "Two", "arg0", "arg1", "field0", "field1", "space", "visible",
    ]);
    let name = |text| symbols.get(text).unwrap();
    let mut slots = Reservations::new(symbols.clone());
    slots
        .set_scope_module(
            ScopeId::ROOT,
            ModuleIdentity::new(PackageIdentity::new("app"), ["client"]),
        )
        .unwrap();
    let alpha = slots.reserve(name("alpha"), parameters[0]);
    slots.reserve(name("beta"), parameters[1]);
    for (name, count) in [name("One"), name("Two")].into_iter().zip(fields) {
        slots.reserve_record(name, count);
    }
    let exports = slots.new_scope();
    slots
        .bind_export(exports, name("visible"), alpha, Arc::new(audience))
        .unwrap();
    slots
        .bind_namespace(ScopeId::ROOT, name("space"), exports, None)
        .unwrap();
    slots.finish().unwrap().into_publication()
}

fn complete(mut headers: HeaderPublication) -> Program {
    let name = |text| headers.symbols().get(text).unwrap();
    let data_names = [name("One"), name("Two")];
    for name in data_names {
        let record = headers.snapshot().resolve_nominal(name).unwrap();
        let fields = headers
            .field_ids(record)
            .unwrap()
            .iter()
            .enumerate()
            .map(|(position, _)| FieldDefinition {
                name: headers.symbols().get(&format!("field{position}")).unwrap(),
                data: BuiltinType::I32.into(),
            })
            .collect::<Vec<_>>();
        headers.define_record(record, true, fields).unwrap();
    }
    let ids = headers.code_ids().collect::<Vec<_>>();
    for id in ids {
        let parameters = headers
            .parameter_ids(id)
            .unwrap()
            .iter()
            .enumerate()
            .map(|(position, _)| ParameterDefinition {
                binding: crate::value_access::ParameterBinding::Name(
                    headers.symbols().get(&format!("arg{position}")).unwrap(),
                ),
                data: BuiltinType::I32.into(),
                operation: ValueInputOperation::Read,
            })
            .collect::<Vec<_>>();
        headers
            .define(
                id,
                CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
                parameters,
                BuiltinType::I32,
            )
            .unwrap();
    }
    headers.finish().unwrap()
}

#[test]
fn equal_reserved_layout_never_grants_typed_readiness_or_construction_lineage() {
    let retained = complete(publication([1, 1], [1, 1], ModuleAudience::Public));
    let mut current = publication([1, 1], [1, 1], ModuleAudience::Public);
    assert!(current.has_same_reserved_layout(&retained));
    let empty = current.snapshot();
    assert!(!empty.headers_complete());
    assert!(!empty.preserves_headers(&retained));
    assert!(!retained.preserves_headers(&empty));
    assert!(!current.has_same_reserved_layout(&empty));
    let alpha = retained
        .resolve(retained.symbols().get("alpha").unwrap())
        .unwrap();
    current
        .define(
            alpha,
            CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [ParameterDefinition {
                binding: crate::value_access::ParameterBinding::Name(
                    current.symbols().get("arg0").unwrap(),
                ),
                data: BuiltinType::I32.into(),
                operation: ValueInputOperation::Read,
            }],
            ValueDefinition::Data(BuiltinType::Bool.into()),
        )
        .unwrap();
    assert!(
        current.has_same_reserved_layout(&retained),
        "signature agreement remains a separate obligation"
    );
}

#[test]
fn equal_total_counts_do_not_hide_changed_parameter_or_representation_assignments() {
    let retained = complete(publication([1, 1], [1, 1], ModuleAudience::Public));
    for (parameters, fields) in [([0, 2], [1, 1]), ([1, 1], [0, 2]), ([2, 0], [2, 0])] {
        let current = publication(parameters, fields, ModuleAudience::Public);
        assert!(!current.has_same_reserved_layout(&retained));
    }
    let restricted = publication(
        [1, 1],
        [1, 1],
        ModuleAudience::Package(PackageIdentity::new("app")),
    );
    assert!(!restricted.has_same_reserved_layout(&retained));
}
