use super::support::resolve_text;
use crate::resolve::{ImportedSymbolKind, SymbolKind};

#[test]
fn imported_calls_are_not_function_signatures_yet() {
    let output = resolve_text(
        r#"use std/io.print

func main(): i32 {
    print("hello") catch error {
        return 1
    }

    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let symbol = output
        .call_targets
        .values()
        .next()
        .and_then(|id| output.symbols.get(*id))
        .unwrap();
    assert_eq!(symbol.name, "print");
    assert!(
        matches!(&symbol.kind, SymbolKind::Imported(imported) if imported.kind == ImportedSymbolKind::UnloadedName)
    );
}

#[test]
fn imports_from_alias_under_local_name() {
    let output = resolve_text(
        r#"use std/io.print as write

func main(): i32 {
    write("hello") catch error {
        return 1
    }

    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert!(output.symbols.symbol_by_name("print").is_none());
    let symbol = output.symbols.symbol_by_name("write").unwrap();
    assert_eq!(symbol.name, "write");
    assert!(
        matches!(&symbol.kind, SymbolKind::Imported(imported) if imported.kind == ImportedSymbolKind::UnloadedName)
    );
}

#[test]
fn imports_namespace_alias_as_visible_name() {
    let output = resolve_text(
        r#"use std/io as io

func main(): i32 {
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let symbol = output.symbols.symbol_by_name("io").unwrap();
    assert_eq!(symbol.name, "io");
    assert!(
        matches!(&symbol.kind, SymbolKind::Imported(imported) if imported.kind == ImportedSymbolKind::Namespace)
    );
}

#[test]
fn imports_default_namespace_alias_as_visible_name() {
    let output = resolve_text(
        r#"use std/io

func main(): i32 {
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let symbol = output.symbols.symbol_by_name("io").unwrap();
    assert_eq!(symbol.name, "io");
    assert!(
        matches!(&symbol.kind, SymbolKind::Imported(imported) if imported.kind == ImportedSymbolKind::Namespace)
    );
}
