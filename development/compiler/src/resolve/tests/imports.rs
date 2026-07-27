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

#[test]
fn block_scoped_imported_calls_resolve_inside_block() {
    let output = resolve_text(
        r#"func main(): i32 {
    use std/io.print
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
fn block_scoped_imports_do_not_escape_block() {
    let output = resolve_text(
        r#"func main(debug: bool): i32 {
    if debug {
        use std/io.print
        print("debug") catch error {
            return 1
        }
    }

    print("done") catch error {
        return 1
    }

    return 0
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert_eq!(output.diagnostics[0].code, "E0416");
    assert!(output.diagnostics[0].message.contains("print"));
    assert_eq!(output.call_targets.len(), 1);
}

#[test]
fn block_scoped_imports_cannot_shadow_outer_visible_names() {
    let output = resolve_text(
        r#"use std/io.print

func debug(): void {
    use debug/console.print
    return
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert_eq!(output.diagnostics[0].code, "E0400");
    assert!(output.diagnostics[0].message.contains("print"));
}

#[test]
fn block_scoped_imports_cannot_shadow_parameters() {
    let output = resolve_text(
        r#"func debug(print: i32): i32 {
    use std/io.print
    return print
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert_eq!(output.diagnostics[0].code, "E0400");
    assert!(output.diagnostics[0].message.contains("print"));
}

#[test]
fn locals_cannot_shadow_block_scoped_imports() {
    let output = resolve_text(
        r#"func debug(): i32 {
    use std/io.print
    let print = 0
    return print
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert_eq!(output.diagnostics[0].code, "E0400");
    assert!(output.diagnostics[0].message.contains("print"));
}

#[test]
fn block_scoped_import_aliases_can_avoid_shadowing_outer_imports() {
    let output = resolve_text(
        r#"use std/io.print

func debug(): i32 {
    use debug/console.print as debug_print
    debug_print("details") catch error {
        return 1
    }
    print("done") catch error {
        return 1
    }

    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let resolved_names = output
        .call_targets
        .values()
        .filter_map(|id| output.symbols.get(*id))
        .map(|symbol| symbol.name.as_str())
        .collect::<Vec<_>>();

    assert!(resolved_names.contains(&"debug_print"));
    assert!(resolved_names.contains(&"print"));
}
