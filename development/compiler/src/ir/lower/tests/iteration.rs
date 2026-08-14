use super::*;

fn cached_main_body(fixture: &LoweringFixture) -> crate::mir::Body {
    let root = fixture.analysis.root_file().unwrap();
    let body_id = root
        .ast
        .items
        .iter()
        .find_map(|item| match item {
            crate::ast::Item::Function(function) if function.name == "main" => {
                function.body.as_ref()
            }
            _ => None,
        })
        .and_then(|body| fixture.analysis.semantic_db.body_at(body.span))
        .unwrap();
    fixture
        .analysis
        .mir_bodies
        .cached_specialized(body_id, &HashMap::new())
        .expect("lowering should retain a MIR cache entry")
        .expect("collection iteration should construct valid MIR")
}

#[test]
fn collection_iteration_uses_optional_inspection_in_retained_mir() {
    let fixture = analyze_text_fixture_with_development_home(
        r#"use std/vec.Vec

func main(): i32 {
    let values = Vec [1, 2, 3]
    var total: i32 = 0
    for value in move values {
        total = total + value
    }
    return total
}
"#,
    );
    lower_executable(&fixture.analysis, &fixture.sources).unwrap();
    let body = cached_main_body(&fixture);

    assert_eq!(body.loop_regions.len(), 1);
    let loop_ = body.loop_regions[0];
    assert!(matches!(
        body.blocks[loop_.condition.index()].terminator,
        crate::mir::Terminator::InspectOutcome {
            source: crate::mir::Operand::Copy(_) | crate::mir::Operand::Move(_),
            layer: crate::outcomes::OutcomeLayer::Optional,
            ..
        }
    ));
}

#[test]
fn nested_collection_iteration_remains_on_the_mir_route() {
    let fixture = analyze_text_fixture_with_development_home(
        r#"use std/vec.Vec

func main(): i32 {
    let outer = Vec [1, 2]
    var total: i32 = 0
    for left in move outer {
        let inner = Vec [3, 4]
        for right in move inner {
            total = total + left * right
        }
    }
    return total
}
"#,
    );
    lower_executable(&fixture.analysis, &fixture.sources).unwrap();
    let body = cached_main_body(&fixture);

    assert_eq!(body.loop_regions.len(), 2);
}
