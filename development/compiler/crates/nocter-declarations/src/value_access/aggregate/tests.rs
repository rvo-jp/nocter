use super::*;
use crate::value_access::{
    CallableKind, DataReference, DeclarationRule, FieldDefinition, Reservations, TupleData,
};
use nocter_model::{BuiltinType, CompileTimeGuarantee, SymbolTable};

fn pair(first: DataReference, second: DataReference) -> DataReference {
    DataReference::Tuple(TupleData::new(first, second, []))
}
#[test]
fn nominal_fields_and_tuple_positions_cannot_substitute_for_each_other() {
    let symbols = SymbolTable::from_spellings(["Box", "value"]);
    let mut reservations = Reservations::new(symbols.clone());
    let record = reservations.reserve_record(symbols.get("Box").unwrap());
    let mut builder = reservations.finish().unwrap();
    builder
        .define_record(
            record,
            true,
            [FieldDefinition {
                name: symbols.get("value").unwrap(),
                data: pair(BuiltinType::I32.into(), BuiltinType::Bool.into()),
            }],
        )
        .unwrap();
    let program = builder.finish().unwrap();
    let record = program.records().get(record).unwrap();
    let key = ValueField::Record(record.fields()[0]);
    let tuple = program.component(record.data(), key).unwrap().data;
    assert!(
        program
            .component(record.data(), ValueField::Tuple(0))
            .is_none()
    );
    assert!(program.component(tuple, key).is_none());
    assert!(program.component(tuple, ValueField::Tuple(2)).is_none());
    assert_eq!(
        program.component(tuple, ValueField::Tuple(0)).unwrap().data,
        program.data_type(BuiltinType::I32).unwrap()
    );
    assert_eq!(program.data_copyable(record.data()), Some(true));
}
#[test]
fn nominal_containment_and_copy_validation_follow_leaves_inside_tuples() {
    for (recursive, copy) in [(true, false), (false, true)] {
        let symbols = SymbolTable::from_spellings(["Box", "Item", "value"]);
        let mut reservations = Reservations::new(symbols.clone());
        let owner = reservations.reserve_record(symbols.get("Box").unwrap());
        let item = reservations.reserve_record(symbols.get("Item").unwrap());
        let mut builder = reservations.finish().unwrap();
        builder
            .define_record(
                owner,
                copy,
                [FieldDefinition {
                    name: symbols.get("value").unwrap(),
                    data: pair(DataReference::Record(item), BuiltinType::I32.into()),
                }],
            )
            .unwrap();
        let fields = if recursive {
            vec![FieldDefinition {
                name: symbols.get("value").unwrap(),
                data: pair(BuiltinType::Bool.into(), DataReference::Record(owner)),
            }]
        } else {
            vec![]
        };
        builder.define_record(item, false, fields).unwrap();
        let failures = builder.finish().unwrap_err();
        assert!(failures.iter().any(|failure| if recursive {
            matches!(failure.rule, DeclarationRule::RecursivePayload { .. })
        } else {
            failure.rule == DeclarationRule::NonCopyField
        }));
    }
}
#[test]
fn shared_structural_copy_proof_visits_shapes_without_expanding_the_payload() {
    let symbols = SymbolTable::from_spellings(["run"]);
    let mut reservations = Reservations::new(symbols.clone());
    let run = reservations.reserve(symbols.get("run").unwrap());
    let mut builder = reservations.finish().unwrap();
    builder
        .define(
            run,
            CallableKind::Function(CompileTimeGuarantee::RuntimeOnly),
            [],
            BuiltinType::Void,
        )
        .unwrap();
    let program = builder.finish().unwrap();
    let mut body = program.body_types(run).unwrap();
    let mut data = program.data_type(BuiltinType::I32).unwrap();
    for _ in 0..64 {
        data = body.tuple(&[data, data]).unwrap();
    }
    assert_eq!(body.program().data_copyable(data), Some(true));
    assert!(body.tuple(&[data]).is_err());
}
