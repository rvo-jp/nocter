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

#[test]
fn destructor_entry_requires_restored_components_on_every_path() {
    let prepare = |run: &str| {
        let body = format!(
            "see ./index.nct\n\
             struct Item {{ value: i32 }}\n\
             struct Handle {{ items: [Item; 1]\nitem: Item\ncount: i32 }}\n\
             impl Handle {{ drop {{ self.count += 1 }} }}\n\
             enum Packet {{ empty\ndata(item: Item)\nnumber(value: i32) }}\n\
             impl Packet {{ drop {{}} }}\n\
             func item(): Item {{ Item {{ value: 42 }} }}\n\
             func make(): Handle {{ Handle {{ items: [item(); 1], item: item(), count: 0 }} }}\n\
             {run}"
        );
        let fixture = Fixture::new(
            &[
                ("/app/index.nct", Root, "see ./body.nct\nstruct Handle"),
                ("/app/body.nct", Implementation, &body),
            ],
            &[(0, 0, 1), (1, 0, 0)],
        );
        ModuleSourceUnit::prepare(&fixture.input()).map(drop)
    };
    for incomplete in [
        "func run(): i32 { let owner = make()\nlet moved = owner.item^\nmoved.value }",
        "func run(): i32 { let owner = make()\nowner.count^ }",
        "func run(index: usize): i32 { let owner = make()\nlet moved = owner.items[index]^\nmoved.value }",
        "func run(leave: bool): i32 { var owner = make()\nlet moved = owner.item^\nif leave { return moved.value }\nowner.item = item()\nmoved.value }",
        "func run(): i32 { var owner = make()\nlet moved = owner.item^\nowner = make()\nmoved.value }",
        "func run(): i32 { let outer = (make(), 0)\nlet moved = outer.0.item^\ndrop outer\nmoved.value }",
    ] {
        let result = prepare(incomplete);
        let Err(SourceFailure::Checking(accepted::Failure::Flow { diagnostics, .. })) = result
        else {
            panic!("required restoration must be diagnosed at cleanup: {incomplete}: {result:?}");
        };
        assert!(
            diagnostics
                .iter()
                .any(|error| error.message().contains("restore every moved component"))
        );
    }
    for immediate in [
        "func run(): i32 { let value = make().item\nvalue.value }",
        "func run(take packet: Packet): i32 { if take packet^ is Packet.data(part) { part.value } else { 0 } }",
        "func run(take packet: Packet): i32 { match take packet^ { Packet.data(part) { part.value }\n_ { 0 } } }",
    ] {
        let result = prepare(immediate);
        let Err(SourceFailure::Checking(accepted::Failure::Body {
            failure: Failure::Rejected(error),
            ..
        })) = result
        else {
            panic!("partial extraction must be rejected at its source: {result:?}");
        };
        assert_eq!(error.rule(), Rule::IncompleteDestructor, "{immediate}");
    }
    for valid in [
        "func run(): i32 { var owner = make()\nowner.item = item()\nlet moved = owner^\nmoved.item.value }",
        "func run(): i32 { var owner = make()\nlet moved = owner.item^\nowner.item = moved^\nowner.item.value }",
        "func run(reuse: bool): i32 { var owner = make()\nlet moved = owner.item^\nif reuse { owner.item = moved^ } else { owner.item = item() }\nowner.item.value }",
        "func run(): i32 { let pair = (make(), 0)\nlet moved = pair.0^\nmoved.count }",
        "func run(): i32 { make().count }",
        "func run(take packet: Packet): i32 { if take packet^ is Packet.number(value) { value } else { 0 } }",
        "func run(take packet: Packet): i32 { match take packet^ { Packet.number(value) { value }\n_ { 0 } } }",
    ] {
        let result = prepare(valid);
        assert!(
            result.is_ok(),
            "complete owners must pass lifecycle qualification: {valid}: {result:?}"
        );
    }
}
