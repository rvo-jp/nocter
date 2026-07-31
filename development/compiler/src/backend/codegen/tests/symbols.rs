use super::*;

#[test]
fn maps_imported_call_target_to_imported_function_symbol() {
    let source = SourceId::new(9);
    let symbol = FunctionSymbol::from_call_target(&CallTarget::imported(source, "answer"));

    assert_eq!(
        symbol,
        FunctionSymbol::Imported {
            source,
            name: "answer".to_string(),
        }
    );
    assert_eq!(symbol.description(), "answer from source 9");
}

#[test]
fn maps_function_definition_to_same_file_function_symbol() {
    let function = Function {
        name: "answer".to_string(),
        target: crate::ir::CallTarget::same_file("answer".to_string()),
        return_type: Type::I32,
        instructions: vec![Instruction::Return],
    };

    assert_eq!(
        FunctionSymbol::from_function(&function),
        FunctionSymbol::SameFile("answer".to_string())
    );
}

#[test]
fn maps_imported_function_definition_to_imported_function_symbol() {
    let source = SourceId::new(11);
    let function = Function {
        name: "answer".to_string(),
        target: CallTarget::imported(source, "answer"),
        return_type: Type::I32,
        instructions: vec![Instruction::Return],
    };

    assert_eq!(
        FunctionSymbol::from_function(&function),
        FunctionSymbol::Imported {
            source,
            name: "answer".to_string(),
        }
    );
}
