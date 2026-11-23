use super::*;

#[test]
fn recursive_type_projection_keeps_callable_grouping_outcomes_operations_and_counts_distinct() {
    for (annotation, optional) in [("func(): i32?", false), ("(func(): i32)?", true)] {
        let source = format!("static ACTION: {annotation}");
        let (sources, tree) = input(&source, ParseGoal::V2CoreUnit);
        let syntax = bound(&sources, &tree);
        let declaration = headers::declarations(syntax).unwrap()[0];
        let ty = nocter_syntax::direct_node(&tree, declaration, NodeKind::Type).unwrap();
        let projected = project_type_syntax(syntax, ty).unwrap();
        if optional {
            let TypeSyntax::Optional { payload, .. } = projected else {
                panic!("optional callable");
            };
            assert!(matches!(*payload, TypeSyntax::Callable(_)));
        } else {
            let TypeSyntax::Callable(contract) = projected else {
                panic!("callable returning optional");
            };
            assert!(matches!(*contract.result, TypeSyntax::Optional { .. }));
        }
    }
    let source =
        "func nested(input: func(take func([i32; width()]): i32): func(): [i32; 2]): void {}";
    let (sources, tree) = input(source, ParseGoal::V2CoreUnit);
    let syntax = bound(&sources, &tree);
    let declaration = headers::declarations(syntax).unwrap()[0];
    let header = headers::function(syntax, declaration).unwrap();
    let ty = &header.parameters[0].data;
    let TypeSyntax::Callable(contract) = ty else {
        panic!("outer callable");
    };
    assert_eq!(
        contract.inputs[0].operation,
        nocter_model::ValueInputOperation::Take
    );
    assert!(matches!(contract.inputs[0].data, TypeSyntax::Callable(_)));
    let mut lengths = Vec::new();
    ty.length_expressions(&mut lengths);
    assert_eq!(
        lengths
            .iter()
            .map(|node| syntax.text_at(tree.node(*node).unwrap().range()).unwrap())
            .collect::<Vec<_>>(),
        ["width()", "2"]
    );
}
