use nocter_declaration_lowering::lower_compile_unit_declarations;

use super::check_prepared_program;
use crate::test_support::Fixture;
use crate::{CheckedOperation, prepare_program_checking};

#[test]
fn readonly_borrow_uses_the_same_resolved_parameter_place() {
    let fixture =
        Fixture::new("func observe(value: i32): void {\n    let view = &value\n    return\n}\n");
    let (input, prelude) = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    let output = check_prepared_program(&input, prepared).unwrap();
    let (_, body) = output.program().bodies().iter().next().unwrap();

    assert!(
        body.nodes()
            .iter()
            .any(|(_, node)| matches!(node.operation(), CheckedOperation::Borrow { .. }))
    );
    assert_eq!(body.places().len(), 1);
}
