use super::support::resolve_text;
use crate::ast::Visibility;
use crate::resolve::{SymbolKind, TypeSymbol, TypeSymbolKind};

#[test]
fn collects_function_symbols() {
    let output = resolve_text(
        r#"func main(): i32 {
    return answer()
}

func answer(): i32 {
    return 1
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let symbol = output.symbols.symbol_by_name("answer").unwrap();
    assert_eq!(symbol.name, "answer");
    assert!(matches!(symbol.kind, SymbolKind::Function(_)));
}

#[test]
fn collects_primitive_and_type_symbols() {
    let output = resolve_text(
        r#"pub primitive addr<T>(pointer: *T): usize

	pub type Bytes = [u8]

pub copy struct File {
    pub fd: i32
    name: &str
}

pub enum IOError {
    denied
}

func main(): i32 {
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert!(matches!(
        &output.symbols.symbol_by_name("addr").unwrap().kind,
        SymbolKind::Primitive(_)
    ));
    assert!(matches!(
        &output.symbols.symbol_by_name("Bytes").unwrap().kind,
        SymbolKind::Type(TypeSymbol {
            kind: TypeSymbolKind::Alias,
            ..
        })
    ));
    let file_symbol = output.symbols.symbol_by_name("File").unwrap();
    let SymbolKind::Type(TypeSymbol {
        kind: TypeSymbolKind::Struct,
        is_copy,
        fields,
        ..
    }) = &file_symbol.kind
    else {
        panic!("expected struct symbol");
    };
    assert!(*is_copy);
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].name, "fd");
    assert_eq!(fields[0].visibility, Visibility::Public);
    assert_eq!(fields[1].name, "name");
    assert_eq!(fields[1].visibility, Visibility::Private);
    assert!(matches!(
        &output.symbols.symbol_by_name("IOError").unwrap().kind,
        SymbolKind::Type(TypeSymbol {
            kind: TypeSymbolKind::Enum,
            ..
        })
    ));
}

#[test]
fn collects_associated_function_symbols() {
    let output = resolve_text(
        r#"struct Point {
    x: i32
}

pub func Point.origin(): Point {
    return Point{ x: 0 }
}

impl Point {
    method (point: Self).x_value(): i32 {
        return point.x
    }
}

func main(): i32 {
    return Point.origin().x
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let point_symbol = output.symbols.symbol_by_name("Point").unwrap();
    let SymbolKind::Type(TypeSymbol {
        associated_functions,
        methods,
        ..
    }) = &point_symbol.kind
    else {
        panic!("expected type symbol");
    };

    assert_eq!(associated_functions.len(), 1);
    assert_eq!(associated_functions[0].name, "origin");
    assert_eq!(associated_functions[0].visibility, Visibility::Public);
    assert_eq!(associated_functions[0].signature.parameters.len(), 0);
    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].name, "x_value");
    assert_eq!(methods[0].receiver.name, "point");
    assert_eq!(methods[0].signature.parameters.len(), 0);
}

#[test]
fn collects_drop_member_signature() {
    let output = resolve_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop file: &+Self {
        return
    }
}

func main(): i32 {
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let file_symbol = output.symbols.symbol_by_name("File").unwrap();
    let SymbolKind::Type(TypeSymbol { drop_member, .. }) = &file_symbol.kind else {
        panic!("expected type symbol");
    };
    let drop_member = drop_member.as_ref().expect("expected drop member");
    assert_eq!(drop_member.target_name, "File.drop");
    assert_eq!(drop_member.binding.name, "file");
}

#[test]
fn collects_trait_symbols() {
    let output = resolve_text(
        r#"pub trait Writer {
    method (writer: &+Self).write(text: &str): void!
}

func main(): i32 {
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let symbol = output.symbols.symbol_by_name("Writer").unwrap();
    assert!(matches!(
        &symbol.kind,
        SymbolKind::Type(TypeSymbol {
            kind: TypeSymbolKind::Trait,
            ..
        })
    ));
}
