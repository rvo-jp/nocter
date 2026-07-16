use super::support::{find_json_node, parse_text, parse_text_with_sources};
use crate::ast::{ImplMember, Item, TypeExpr, Visibility};

#[test]
fn parses_hello_entry_function() {
    let output = parse_text(
        r#"use std/prelude

from std/io import print

func main(): i32 {
    print("Hello") catch error {
        return 1
    }

    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    assert_eq!(ast.items.len(), 3);
    assert!(matches!(ast.items[0], Item::Use(_)));
    assert!(matches!(ast.items[1], Item::FromImport(_)));
    assert!(matches!(ast.items[2], Item::Function(_)));
}

#[test]
fn parses_import_aliases() {
    let output = parse_text(
        r#"import std/io as io
from std/io import File as StdFile, stdout
pub from std/string import String as StdString

func main(): i32 {
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Import(import) = &ast.items[0] else {
        panic!("expected namespace import");
    };
    let Item::FromImport(from_import) = &ast.items[1] else {
        panic!("expected from import");
    };
    let Item::FromImport(reexport) = &ast.items[2] else {
        panic!("expected public re-export");
    };

    assert_eq!(import.path.value, "std/io");
    assert_eq!(import.alias.name, "io");
    assert_eq!(from_import.names[0].name, "File");
    assert_eq!(from_import.names[0].local_name(), "StdFile");
    assert_eq!(from_import.names[1].name, "stdout");
    assert_eq!(from_import.names[1].local_name(), "stdout");
    assert_eq!(reexport.visibility, Visibility::Public);
    assert_eq!(reexport.names[0].local_name(), "StdString");
}

#[test]
fn parses_qualified_associated_functions_inherent_methods_and_generic_params() {
    let output = parse_text(
        r#"pub struct Counter {
    value: i32
}

pub func Counter.zero(): i32 {
    return 0
}

impl Counter {
    pub method (counter: &+Self).add(value: i32): void {
        return
    }

    drop counter: &+Self {
        return
    }
}

func print<W>(writer: &+W): void! {
    return
}

func main(): i32 {
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();

    let Item::Function(associated_function) = &ast.items[1] else {
        panic!("expected associated function");
    };
    assert_eq!(associated_function.name, "Counter.zero");
    assert_eq!(associated_function.member_name, "zero");
    let owner = associated_function.owner.as_ref().unwrap();
    assert_eq!(owner.name, "Counter");

    let Item::Impl(inherent_impl) = &ast.items[2] else {
        panic!("expected inherent impl");
    };
    assert!(inherent_impl.trait_ty.is_none());
    assert!(matches!(
        &inherent_impl.target_ty,
        TypeExpr::Reference(reference) if reference.name == "Counter"
    ));
    let ImplMember::Method(method) = &inherent_impl.members[0] else {
        panic!("expected method");
    };
    assert_eq!(method.name, "add");
    assert!(method.body.is_some());
    assert!(matches!(&method.receiver.ty, TypeExpr::Borrow(_)));
    let ImplMember::Drop(drop_) = &inherent_impl.members[1] else {
        panic!("expected drop member");
    };
    assert_eq!(drop_.binding.name, "counter");
    assert!(matches!(
        &drop_.binding.ty,
        TypeExpr::Borrow(borrow) if borrow.is_readwrite
    ));

    let Item::Function(function) = &ast.items[3] else {
        panic!("expected generic function");
    };
    assert_eq!(function.generics.parameters.len(), 1);
    assert_eq!(function.generics.parameters[0].name, "W");
    assert!(function.generics.parameters[0].bound.is_none());
}

#[test]
fn rejects_trait_declarations_in_v0() {
    let output = parse_text(
        r#"pub trait Writer {
    method (writer: &+Self).write(text: &str): void!
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert!(output.diagnostics[0].message.contains("deferred after v0"));
}

#[test]
fn rejects_trait_impls_in_v0() {
    let output = parse_text(
        r#"struct Counter {
    value: i32
}

impl Writer for Counter {
    method (counter: &+Self).write(text: &str): void! {
        return
    }
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert!(
        output.diagnostics[0]
            .message
            .contains("trait implementations")
    );
}

#[test]
fn rejects_generic_bounds_in_v0() {
    let output = parse_text(
        r#"func print<W: Writer>(writer: &+W): void! {
    return
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert!(output.diagnostics[0].message.contains("generic bounds"));
}

#[test]
fn rejects_function_members_in_impl_blocks() {
    let output = parse_text(
        r#"struct Counter {
    value: i32
}

impl Counter {
    func zero(): i32 {
        return 0
    }
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert!(output.diagnostics[0].message.contains("func Type.name"));
}

#[test]
fn parses_drop_as_ordinary_function_name() {
    let output = parse_text(
        r#"func drop(): void {
    return
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function");
    };
    assert_eq!(function.name, "drop");
}

#[test]
fn parses_trait_as_ordinary_function_name() {
    let output = parse_text(
        r#"func trait(): void {
    return
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function");
    };
    assert_eq!(function.name, "trait");
}

#[test]
fn parses_relative_import_paths() {
    let output = parse_text(
        r#"from ./config import Config
from ../shared/path import Path

func main(): i32 {
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::FromImport(config) = &ast.items[0] else {
        panic!("expected first item to be a relative import");
    };
    let Item::FromImport(path) = &ast.items[1] else {
        panic!("expected second item to be a relative import");
    };

    assert_eq!(config.path.value, "./config");
    assert_eq!(config.visibility, Visibility::Private);
    assert_eq!(path.path.value, "../shared/path");
    assert_eq!(path.visibility, Visibility::Private);
}

#[test]
fn parses_public_reexports() {
    let output = parse_text(
        r#"pub from std/string import String

func main(): i32 {
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::FromImport(import) = &ast.items[0] else {
        panic!("expected first item to be a public re-export");
    };

    assert_eq!(import.visibility, Visibility::Public);
    assert_eq!(import.names.len(), 1);
}

#[test]
fn parses_top_level_type_and_primitive_declarations() {
    let output = parse_text(
        r#"pub type Bytes = [u8]

pub copy struct Layout {
    size: usize
    align: usize
}

pub enum IOError {
    not_found(path: &str)
    denied
}

pub(nocter) primitive addr<T>(pointer: *T): usize

pub func write(file: &+File, text: &str): void! {
    return
}

func main(): i32 {
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    assert_eq!(ast.items.len(), 6);

    let Item::TypeAlias(alias) = &ast.items[0] else {
        panic!("expected type alias");
    };
    assert_eq!(alias.visibility, Visibility::Public);
    assert!(matches!(&alias.target, TypeExpr::View(_)));

    let Item::Struct(struct_) = &ast.items[1] else {
        panic!("expected struct declaration");
    };
    assert!(struct_.is_copy);
    assert_eq!(struct_.fields.len(), 2);

    let Item::Enum(enum_) = &ast.items[2] else {
        panic!("expected enum declaration");
    };
    assert_eq!(enum_.variants.len(), 2);
    assert_eq!(enum_.variants[0].payload.len(), 1);

    let Item::Primitive(primitive) = &ast.items[3] else {
        panic!("expected primitive declaration");
    };
    assert_eq!(primitive.visibility, Visibility::Nocter);
    assert_eq!(primitive.generics.parameters.len(), 1);
    assert!(matches!(
        &primitive.parameters.parameters[0].ty,
        TypeExpr::Pointer(_)
    ));

    let Item::Function(function) = &ast.items[4] else {
        panic!("expected function declaration");
    };
    assert!(matches!(
        &function.parameters.parameters[0].ty,
        TypeExpr::Borrow(borrow) if borrow.is_readwrite
    ));
    assert!(matches!(&function.return_type, TypeExpr::Fallible(_)));
}

#[test]
fn ast_json_includes_attached_documentation() {
    let (sources, output) = parse_text_with_sources(
        r#"//! File docs.

/// Stores a path.
pub struct File {
    /// Raw path view.
    pub path: &str
}

/// Runs the program.
func main(): i32 {
    /// Exit code.
    let code = 0
    return code
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap().to_json(&sources);

    assert_eq!(ast.documentation.as_deref(), Some("File docs."));

    let struct_ = find_json_node(&ast, "struct_decl").expect("expected struct node");
    assert_eq!(struct_.documentation.as_deref(), Some("Stores a path."));

    let field = find_json_node(&ast, "struct_field").expect("expected struct field node");
    assert_eq!(field.documentation.as_deref(), Some("Raw path view."));

    let function = find_json_node(&ast, "function_decl").expect("expected function node");
    assert_eq!(function.documentation.as_deref(), Some("Runs the program."));

    let binding = find_json_node(&ast, "let_statement").expect("expected let statement node");
    assert_eq!(binding.documentation.as_deref(), Some("Exit code."));
}

#[test]
fn ast_json_does_not_attach_documentation_across_empty_lines() {
    let (sources, output) = parse_text_with_sources(
        r#"/// Detached.

func main(): i32 {
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap().to_json(&sources);
    let function = find_json_node(&ast, "function_decl").expect("expected function node");

    assert_eq!(function.documentation, None);
}

#[test]
fn diagnoses_unknown_top_level_item() {
    let output = parse_text(
        r#"module app/main

func main(): i32 {
    return 0
}
"#,
    );

    assert!(output.ast.is_none());
    assert_eq!(output.diagnostics.len(), 1);
    assert!(output.diagnostics[0].message.contains("top-level item"));
}
