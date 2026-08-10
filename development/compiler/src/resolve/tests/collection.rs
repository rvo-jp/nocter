use super::support::resolve_text;
use crate::ast::Visibility;
use crate::resolve::{SymbolKind, TypeSymbol, TypeSymbolKind};

#[test]
fn collects_source_declared_builtin_method_surfaces_outside_the_symbol_table() {
    let output = resolve_text(
        r#"instance str {
    pub method &self.count(): usize {
        return 1
    }
}

instance [T] {
    pub method &self.count(): usize {
        return 2
    }
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert!(output.symbols.symbol_by_name("str").is_none());
    let text = output
        .builtin_type_surface(crate::builtin_types::BuiltinTypeOwner::Str)
        .expect("str method surface");
    let slice = output
        .builtin_type_surface(crate::builtin_types::BuiltinTypeOwner::Slice)
        .expect("slice method surface");
    assert_eq!(text.symbol.methods[0].name, "count");
    assert_eq!(slice.symbol.generic_parameters, ["T"]);
    assert_eq!(slice.symbol.methods[0].name, "count");
}

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

construct Point {
    pub default func origin(): Self {
        return Point { x: 0 }
    }
}

instance Point {
    method self.x_value(): i32 {
        return self.x
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
    assert_eq!(methods[0].receiver.name, "self");
    assert_eq!(methods[0].signature.parameters.len(), 0);
}

#[test]
fn collects_conformanceementation_member_identities() {
    let output = resolve_text(
        r#"interface Measure {
    pub method &self.measure(): i32
}

struct Count {
    value: i32
}

conform Measure for Count {
    method &self.measure(): i32 {
        return self.value
    }
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let count = output.type_symbol_by_name("Count").unwrap();
    let [conformance] = count.interface_conformances.as_slice() else {
        panic!("expected one conformance: {count:?}");
    };
    let [method] = conformance.methods.as_slice() else {
        panic!("expected one conformance member: {conformance:?}");
    };
    assert_eq!(method.name, "measure");
    assert!(!method.has_default_body);
    assert_eq!(
        method.owner_target_ty.as_ref(),
        Some(&conformance.target_ty)
    );
    assert_eq!(
        output
            .method_signature_by_name_span(method.name_span)
            .map(|resolved| resolved.name.as_str()),
        Some("measure")
    );
}

#[test]
fn collects_destructor_signature() {
    let output = resolve_text(
        r#"struct File {
    fd: i32
}

destruct File(&+self) {
    return
}

func main(): i32 {
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let file_symbol = output.symbols.symbol_by_name("File").unwrap();
    let SymbolKind::Type(TypeSymbol { destructor, .. }) = &file_symbol.kind else {
        panic!("expected type symbol");
    };
    let destructor = destructor.as_ref().expect("expected destructor");
    assert_eq!(destructor.target_name, "File.drop");
    assert_eq!(destructor.binding.name, "self");
}
