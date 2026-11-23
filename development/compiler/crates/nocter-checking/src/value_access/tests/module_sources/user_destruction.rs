use super::*;
use crate::value_access::evaluation;
use nocter_constant_evaluation::state::{Limits, Value};

#[test]
fn authored_destruction_blocks_required_array_lengths() {
    let fixture = Fixture::new(
        &[
            (
                "/app/index.nct",
                Root,
                "see ./body.nct\nstruct Handle\nstruct Sized",
            ),
            (
                "/app/body.nct",
                Implementation,
                concat!(
                    "see ./index.nct\n",
                    "struct Handle { value: i32 }\n",
                    "struct Sized { items: [i32; if true {\n",
                    "    let handle = Handle { value: 1 }\n",
                    "    drop handle\n",
                    "    42\n",
                    "} else { 0 }] }\n",
                    "impl Handle { drop { self.value += 1 } }",
                ),
            ),
        ],
        &[(0, 0, 1), (1, 0, 0)],
    );
    let Err(SourceFailure::Checking(accepted::Failure::Const { diagnostic, .. })) =
        ModuleSourceUnit::prepare(&fixture.input())
    else {
        panic!("array length must reject the required destructor before executable admission");
    };
    assert!(matches!(
        diagnostic.rule(),
        crate::value_access::constant::Rule::UnsupportedCleanup(_)
    ));
}

#[test]
fn absent_destructor_bearing_components_do_not_block_required_values() {
    let fixture = Fixture::new(
        &[
            (
                "/app/index.nct",
                Root,
                "see ./body.nct\nstruct Handle\nconst ANSWER: usize",
            ),
            (
                "/app/body.nct",
                Implementation,
                concat!(
                    "see ./index.nct\n",
                    "struct Handle { value: i32 }\n",
                    "impl Handle { drop { self.value += 1 } }\n",
                    "enum Packet { empty\nresource(value: Handle) }\n",
                    "struct EmptyBuffer { items: [Handle; 0] }\n",
                    "const func compute(): usize {\n",
                    "    let packet = Packet.empty\n",
                    "    drop packet\n",
                    "    42\n",
                    "}\n",
                    "const ANSWER: usize = compute()",
                ),
            ),
        ],
        &[(0, 0, 1), (1, 0, 0)],
    );
    let input = fixture.input();
    let headers = declarations::ModuleSources::prepare(&input)
        .unwrap()
        .pair()
        .unwrap()
        .reserve_names()
        .unwrap();
    let (declarations, values) =
        evaluation::prepare_module_source(headers, Limits::default()).unwrap();
    let current = declarations.current().unwrap();
    let program = current.program();
    let answer = program
        .resolve_constant(program.symbols().get("ANSWER").unwrap())
        .unwrap();
    assert_eq!(
        values.values.get(&answer.into()).unwrap().as_ref(),
        &Value::integer(42).freeze(),
    );
    let buffer = program.type_named("EmptyBuffer").unwrap();
    let mut types = evaluation::Types::default();
    assert!(types.supports(program, buffer).unwrap());
    assert!(types.supports_final(program, buffer).unwrap());
}
