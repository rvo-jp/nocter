use super::*;

fn table(cases: &[RuntimeDataId]) -> Result<RuntimeTypeTable, RuntimeTypeTableBuildError> {
    let mut builder = RuntimeTypeTableBuilder::new();
    for (id, shape) in [
        (
            RuntimeDataId::Instance(0),
            RuntimeType::CallableAlternatives(cases.into()),
        ),
        (
            RuntimeDataId::Instance(1),
            RuntimeType::StaticCallable { environment: None },
        ),
        (
            RuntimeDataId::Instance(2),
            RuntimeType::StaticCallable { environment: None },
        ),
        (RuntimeDataId::Instance(3), RuntimeType::Unit),
    ] {
        builder.insert(id, shape).unwrap();
    }
    builder.finish()
}

#[test]
fn preserves_canonical_case_order_independently_of_runtime_ids() {
    let cases = [RuntimeDataId::Instance(2), RuntimeDataId::Instance(1)];
    assert_eq!(
        table(&cases).unwrap().get(RuntimeDataId::Instance(0)),
        Some(&RuntimeType::CallableAlternatives(cases.into()))
    );
}

#[test]
fn rejects_empty_singleton_duplicate_noncallable_and_nested_cases() {
    let a = RuntimeDataId::Instance(1);
    let b = RuntimeDataId::Instance(2);
    for cases in [
        vec![],
        vec![a],
        vec![a, a],
        vec![a, RuntimeDataId::Instance(3)],
        vec![a, RuntimeDataId::Instance(0)],
        vec![a, b, a],
    ] {
        assert_eq!(
            table(&cases).unwrap_err(),
            RuntimeTypeTableBuildError::InvalidCallableAlternatives(RuntimeDataId::Instance(0))
        );
    }
}

#[test]
fn rejects_missing_case_before_layout() {
    let missing = RuntimeDataId::Instance(9);
    assert_eq!(
        table(&[RuntimeDataId::Instance(1), missing]).unwrap_err(),
        RuntimeTypeTableBuildError::UnknownReference {
            owner: RuntimeDataId::Instance(0),
            referenced: missing
        }
    );
}
