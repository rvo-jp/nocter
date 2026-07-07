use super::support::parse_text;
use crate::ast::{ImplMember, Item, TypeExpr, Visibility};

#[test]
fn parses_hello_program() {
    let output = parse_text(
        r#"use std/prelude

from std/io import print

program(): i32 {
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
    assert!(matches!(ast.items[2], Item::Program(_)));
}

#[test]
fn parses_import_aliases() {
    let output = parse_text(
        r#"import std/io as io
from std/io import File as StdFile, stdout
pub from std/string import String as StdString

program(): i32 {
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
fn parses_impl_trait_methods_and_generic_bounds() {
    let output = parse_text(
        r#"pub struct Counter {
    value: i32
}

impl Counter {
    pub func zero(): i32 {
        return 0
    }

    pub method (counter: &+Self).add(value: i32): void {
        return
    }
}

pub trait Writer {
    method (writer: &+Self).write(text: str): void!
}

impl Writer for Counter {
    method (counter: &+Self).write(text: str): void! {
        return
    }
}

func print<W: Writer>(writer: &+W): void! {
    return
}

program(): i32 {
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();

    let Item::Impl(inherent_impl) = &ast.items[1] else {
        panic!("expected inherent impl");
    };
    assert!(inherent_impl.trait_ty.is_none());
    assert!(matches!(
        &inherent_impl.target_ty,
        TypeExpr::Reference(reference) if reference.name == "Counter"
    ));
    assert!(matches!(
        &inherent_impl.members[0],
        ImplMember::Function(function) if function.name == "zero"
    ));
    let ImplMember::Method(method) = &inherent_impl.members[1] else {
        panic!("expected method");
    };
    assert_eq!(method.name, "add");
    assert!(method.body.is_some());
    assert!(matches!(&method.receiver.ty, TypeExpr::Borrow(_)));

    let Item::Trait(trait_) = &ast.items[2] else {
        panic!("expected trait");
    };
    assert_eq!(trait_.visibility, Visibility::Public);
    assert_eq!(trait_.name, "Writer");
    assert_eq!(trait_.methods.len(), 1);
    assert_eq!(trait_.methods[0].name, "write");
    assert!(trait_.methods[0].body.is_none());

    let Item::Impl(trait_impl) = &ast.items[3] else {
        panic!("expected trait impl");
    };
    assert!(trait_impl.trait_ty.is_some());
    assert!(matches!(
        &trait_impl.target_ty,
        TypeExpr::Reference(reference) if reference.name == "Counter"
    ));

    let Item::Function(function) = &ast.items[4] else {
        panic!("expected generic function");
    };
    assert_eq!(function.generics.parameters.len(), 1);
    assert_eq!(function.generics.parameters[0].name, "W");
    assert!(matches!(
        &function.generics.parameters[0].bound,
        Some(TypeExpr::Reference(reference)) if reference.name == "Writer"
    ));
}

#[test]
fn parses_relative_import_paths() {
    let output = parse_text(
        r#"from ./config import Config
from ../shared/path import Path

program(): i32 {
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

program(): i32 {
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
    not_found(path: str)
    denied
}

pub(nocter) primitive addr<T>(pointer: *T): usize

pub func write(file: &+File, text: str): void! {
    return
}

program(): i32 {
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
fn diagnoses_unknown_top_level_item() {
    let output = parse_text(
        r#"module app/main

program(): i32 {
    return 0
}
"#,
    );

    assert!(output.ast.is_none());
    assert_eq!(output.diagnostics.len(), 1);
    assert!(output.diagnostics[0].message.contains("top-level item"));
}
