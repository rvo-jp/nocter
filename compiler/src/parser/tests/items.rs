use super::support::{find_json_node, parse_text, parse_text_with_sources};
use crate::ast::{ImplMember, Item, TypeExpr, Visibility};

#[test]
fn parses_hello_entry_function() {
    let output = parse_text(
        r#"use std/io.print

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
    assert_eq!(ast.items.len(), 2);
    assert!(matches!(ast.items[0], Item::FromImport(_)));
    assert!(matches!(ast.items[1], Item::Function(_)));
}

#[test]
fn parses_bare_use_as_default_namespace_import_for_any_module_path() {
    let source = r#"use std/io
use ./config
use /shared/config
use ./path/to/dir
use ../shared/path
use ../../shared/path

func main(): i32 {
    return 0
}
"#;
    let output = parse_text(source);

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Import(std_use) = &ast.items[0] else {
        panic!("expected std namespace import");
    };
    let Item::Import(relative_use) = &ast.items[1] else {
        panic!("expected relative namespace import");
    };
    let Item::Import(absolute_use) = &ast.items[2] else {
        panic!("expected absolute namespace import");
    };
    let Item::Import(nested_relative_use) = &ast.items[3] else {
        panic!("expected nested relative namespace import");
    };
    let Item::Import(parent_relative_use) = &ast.items[4] else {
        panic!("expected parent relative namespace import");
    };
    let Item::Import(nested_parent_relative_use) = &ast.items[5] else {
        panic!("expected nested parent relative namespace import");
    };
    assert_eq!(std_use.path.value, "std/io");
    assert_eq!(std_use.alias.name, "io");
    assert!(std_use.alias_is_default);
    assert_eq!(relative_use.path.value, "./config");
    assert_eq!(relative_use.alias.name, "config");
    assert!(relative_use.alias_is_default);
    assert_eq!(absolute_use.path.value, "/shared/config");
    assert_eq!(absolute_use.alias.name, "config");
    assert!(absolute_use.alias_is_default);
    assert_eq!(nested_relative_use.path.value, "./path/to/dir");
    assert_eq!(nested_relative_use.alias.name, "dir");
    assert!(nested_relative_use.alias_is_default);
    assert_eq!(parent_relative_use.path.value, "../shared/path");
    assert_eq!(parent_relative_use.alias.name, "path");
    assert!(parent_relative_use.alias_is_default);
    assert_eq!(nested_parent_relative_use.path.value, "../../shared/path");
    assert_eq!(nested_parent_relative_use.alias.name, "path");
    assert!(nested_parent_relative_use.alias_is_default);
    assert_eq!(
        nested_parent_relative_use.path.span.start,
        source.find("../../shared/path").unwrap()
    );
}

#[test]
fn rejects_invalid_module_path_segments() {
    for source in [
        "use std/IO\n",
        "use std/Self\n",
        "use std/_\n",
        "use ./Path/config\n",
    ] {
        let output = parse_text(source);
        assert!(output.ast.is_none(), "{source}");
        assert_eq!(output.diagnostics.len(), 1, "{source}");
        assert!(
            output.diagnostics[0]
                .message
                .contains("module path segments must be snake_case identifiers"),
            "{source}: {:?}",
            output.diagnostics
        );
    }
}

#[test]
fn parses_import_aliases() {
    let output = parse_text(
        r#"use std/io as io
use std/io.File as StdFile
use std/io.stdout
pub use std/string.String as StdString

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
    let Item::FromImport(stdout_import) = &ast.items[2] else {
        panic!("expected from import");
    };
    let Item::FromImport(reexport) = &ast.items[3] else {
        panic!("expected public re-export");
    };

    assert_eq!(import.path.value, "std/io");
    assert_eq!(import.alias.name, "io");
    assert!(!import.alias_is_default);
    assert_eq!(from_import.names[0].name, "File");
    assert_eq!(from_import.names[0].local_name(), "StdFile");
    assert_eq!(stdout_import.names[0].name, "stdout");
    assert_eq!(stdout_import.names[0].local_name(), "stdout");
    assert_eq!(reexport.visibility, Visibility::Public);
    assert_eq!(reexport.names[0].local_name(), "StdString");
}

#[test]
fn diagnoses_removed_import_syntax() {
    let output = parse_text(
        r#"from std/io import print
"#,
    );

    assert!(output.ast.is_none());
    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert!(
        output.diagnostics[0]
            .message
            .contains("`import` syntax has been removed")
    );
}

#[test]
fn rejects_source_level_prelude_import_forms() {
    for source in [
        "use std/prelude\n",
        "use std/prelude.Error\n",
        "use std/prelude.{Error, String}\n",
        "use std/prelude as prelude\n",
    ] {
        let output = parse_text(source);
        assert!(output.ast.is_none(), "{source}");
        assert_eq!(output.diagnostics.len(), 1, "{source}");
        assert!(
            output.diagnostics[0]
                .message
                .contains("`std/prelude` is compiler-managed"),
            "{source}: {:?}",
            output.diagnostics
        );
    }
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
    pub method &+self.add(value: i32): void {
        return
    }

    drop &+self {
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
    assert!(inherent_impl.interface_ty.is_none());
    assert!(matches!(
        &inherent_impl.target_ty,
        TypeExpr::Reference(reference) if reference.name == "Counter"
    ));
    let ImplMember::Method(method) = &inherent_impl.members[0] else {
        panic!("expected method");
    };
    assert_eq!(method.name, "add");
    assert!(method.body.is_some());
    assert_eq!(method.receiver.name, "self");
    assert!(matches!(&method.receiver.ty, TypeExpr::Borrow(_)));
    let ImplMember::Drop(drop_) = &inherent_impl.members[1] else {
        panic!("expected drop member");
    };
    assert_eq!(drop_.binding.name, "self");
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
fn diagnoses_var_parameters_as_deferred() {
    let output = parse_text(
        r#"func bump(var count: i32): i32 {
    return count
}
"#,
    );

    assert!(output.ast.is_none());
    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert!(
        output.diagnostics[0]
            .message
            .contains("`var` parameters are not part of v0")
    );
}

#[test]
fn diagnoses_default_parameters_as_deferred() {
    let output = parse_text(
        r#"func open(path: &str = "input.txt"): i32 {
    return 0
}
"#,
    );

    assert!(output.ast.is_none());
    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert!(
        output.diagnostics[0]
            .message
            .contains("default parameters are not part of v0")
    );
}

#[test]
fn diagnoses_variadic_parameters_as_deferred() {
    let output = parse_text(
        r#"func print_all(parts: &str...): void {
}
"#,
    );

    assert!(output.ast.is_none());
    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert!(
        output.diagnostics[0]
            .message
            .contains("variadic parameters are not part of v0")
    );
}

#[test]
fn accepts_multiline_parameter_trailing_comma() {
    let output = parse_text(
        r#"func add(
    value: i32,
): i32 {
    return value
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
}

#[test]
fn diagnoses_single_line_parameter_trailing_comma() {
    let output = parse_text(
        r#"func add(value: i32,): i32 {
    return value
}
"#,
    );

    assert!(output.ast.is_none());
    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert!(
        output.diagnostics[0]
            .message
            .contains("single-line parameter lists")
    );
}

#[test]
fn parses_generic_impl_parameters() {
    let output = parse_text(
        r#"struct Box<T> {
    value: T
}

impl<U> Box<U> {
    method self.value(): U {
        return self.value
    }
}

func main(): i32 {
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Impl(impl_) = &ast.items[1] else {
        panic!("expected impl");
    };
    assert_eq!(impl_.generics.parameters.len(), 1);
    assert_eq!(impl_.generics.parameters[0].name, "U");
    let TypeExpr::Generic(target) = &impl_.target_ty else {
        panic!("expected generic impl target");
    };
    assert_eq!(target.name, "Box");
    assert_eq!(target.arguments.len(), 1);
}

#[test]
fn parses_interface_declarations() {
    let output = parse_text(
        r#"pub interface Writer {
    pub method &+self.write(text: &str): void!
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Interface(interface) = &ast.items[0] else {
        panic!("expected interface declaration");
    };
    assert_eq!(interface.visibility, Visibility::Public);
    assert_eq!(interface.name, "Writer");
    assert_eq!(interface.methods.len(), 1);
    assert_eq!(interface.methods[0].visibility, Visibility::Public);
    assert_eq!(interface.methods[0].name, "write");
    assert!(interface.methods[0].body.is_none());
}

#[test]
fn rejects_non_self_method_receiver_name() {
    let output = parse_text(
        r#"struct File {
    fd: i32
}

impl File {
    method file.bad(): i32 {
        return 0
    }
}
"#,
    );

    assert!(output.ast.is_none());
    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert!(
        output.diagnostics[0]
            .message
            .contains("receiver name must be `self`")
    );
}

#[test]
fn rejects_legacy_drop_member_binding_syntax() {
    let output = parse_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop file: Self {
        return
    }
}
"#,
    );

    assert!(output.ast.is_none());
    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert!(output.diagnostics[0].message.contains("expected `&+self`"));
}

#[test]
fn rejects_readonly_drop_member_receiver() {
    let output = parse_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &self {
        return
    }
}
"#,
    );

    assert!(output.ast.is_none());
    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert!(output.diagnostics[0].message.contains("expected `&+self`"));
}

#[test]
fn rejects_trait_declarations() {
    let output = parse_text(
        r#"pub trait Writer {
    pub method &+self.write(text: &str): void!
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert!(output.diagnostics[0].message.contains("has been removed"));
}

#[test]
fn rejects_private_interface_methods() {
    let output = parse_text(
        r#"interface Writer {
    method &+self.write(text: &str): void!
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert!(
        output.diagnostics[0]
            .message
            .contains("must be marked `pub`")
    );
}

#[test]
fn rejects_interface_method_bodies() {
    let output = parse_text(
        r#"interface Writer {
    pub method &+self.write(text: &str): void! {
        return
    }
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert!(output.diagnostics[0].message.contains("cannot have bodies"));
}

#[test]
fn parses_interface_conformance_impls() {
    let output = parse_text(
        r#"interface Writer {
    pub method &+self.write(text: &str): void!
}

struct Counter {
    value: i32
}

impl Writer for Counter
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Impl(interface_impl) = &ast.items[2] else {
        panic!("expected interface impl");
    };
    assert!(
        matches!(&interface_impl.interface_ty, Some(TypeExpr::Reference(reference)) if reference.name == "Writer")
    );
    assert!(matches!(
        &interface_impl.target_ty,
        TypeExpr::Reference(reference) if reference.name == "Counter"
    ));
    assert!(interface_impl.members.is_empty());
}

#[test]
fn rejects_members_in_interface_conformance_impls() {
    let output = parse_text(
        r#"interface Writer {
    pub method &+self.write(text: &str): void!
}

struct Counter {
    value: i32
}

impl Writer for Counter {
    pub method &+self.write(text: &str): void! {
        return
    }
}
"#,
    );

    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert!(
        output.diagnostics[0]
            .message
            .contains("cannot contain members")
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
        r#"use ./config.Config
use ../shared/path.Path

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
        r#"pub use std/string.String

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
fn parses_target_directive_on_primitive_declaration() {
    let (sources, output) = parse_text_with_sources(
        r#"#target("arm64-darwin")
pub(nocter) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Primitive(primitive) = &ast.items[0] else {
        panic!("expected primitive declaration");
    };
    let target = primitive
        .target
        .as_ref()
        .expect("expected target directive");
    assert_eq!(target.target, "arm64-darwin");
    assert_eq!(target.span.start, 0);
    assert_eq!(primitive.span.start, 0);

    let json = ast.to_json(&sources);
    let directive = find_json_node(&json, "target_directive").expect("expected target directive");
    assert_eq!(directive.value.as_deref(), Some("arm64-darwin"));
}

#[test]
fn parses_target_directive_on_function_declaration() {
    let (sources, output) = parse_text_with_sources(
        r#"#target("arm64-darwin")
func main(): i32 {
    return 0
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function declaration");
    };
    let target = function.target.as_ref().expect("expected target directive");
    assert_eq!(target.target, "arm64-darwin");
    assert_eq!(function.span.start, 0);

    let json = ast.to_json(&sources);
    let directive = find_json_node(&json, "target_directive").expect("expected target directive");
    assert_eq!(directive.value.as_deref(), Some("arm64-darwin"));
}

#[test]
fn parses_target_directive_on_type_declarations() {
    let (sources, output) = parse_text_with_sources(
        r#"#target("arm64-darwin")
pub(nocter) type RawWord = usize

#target("arm64-darwin")
pub(nocter) copy struct SyscallResult {
    pub value: usize
    pub errno: i32
}

#target("arm64-darwin")
pub(nocter) enum PlatformError {
    interrupted
}

#target("arm64-darwin")
pub(nocter) interface PlatformContract {
    pub method &self.code(): i32
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::TypeAlias(alias) = &ast.items[0] else {
        panic!("expected type alias declaration");
    };
    let Item::Struct(struct_) = &ast.items[1] else {
        panic!("expected struct declaration");
    };
    let Item::Enum(enum_) = &ast.items[2] else {
        panic!("expected enum declaration");
    };
    let Item::Interface(interface) = &ast.items[3] else {
        panic!("expected interface declaration");
    };
    assert_eq!(
        alias
            .target_directive
            .as_ref()
            .expect("expected target directive")
            .target,
        "arm64-darwin"
    );
    assert_eq!(
        struct_
            .target
            .as_ref()
            .expect("expected target directive")
            .target,
        "arm64-darwin"
    );
    assert_eq!(
        enum_
            .target
            .as_ref()
            .expect("expected target directive")
            .target,
        "arm64-darwin"
    );
    assert_eq!(
        interface
            .target
            .as_ref()
            .expect("expected target directive")
            .target,
        "arm64-darwin"
    );

    let json = ast.to_json(&sources);
    let directive = find_json_node(&json, "target_directive").expect("expected target directive");
    assert_eq!(directive.value.as_deref(), Some("arm64-darwin"));
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
