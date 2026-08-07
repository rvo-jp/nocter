use super::support::{
    assert_rejects_discard_name, assert_rejects_self_name, find_json_node, parse_text,
    parse_text_with_sources,
};
use crate::ast::{ImplMember, Item, MethodReceiverMode, TypeExpr, Visibility};

#[test]
fn parses_result_allocation_modifiers_on_callable_declarations() {
    let output = parse_text(
        r#"pub alloc func copy(): Text { return make() }
pub(nocter) alloc primitive make_text(): Text
interface Factory {
    pub alloc method &self.make(): Text
}
impl Factory for Builder {
    alloc method &self.make(): Text { return make() }
}
func alloc(): void { return }
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function");
    };
    assert!(function.result_allocation.is_some());
    let Item::Primitive(primitive) = &ast.items[1] else {
        panic!("expected primitive");
    };
    assert!(primitive.result_allocation.is_some());
    let Item::Interface(interface) = &ast.items[2] else {
        panic!("expected interface");
    };
    assert!(interface.methods[0].result_allocation.is_some());
    let Item::Impl(implementation) = &ast.items[3] else {
        panic!("expected implementation");
    };
    assert!(matches!(
        &implementation.members[0],
        ImplMember::Method(method) if method.result_allocation.is_some()
    ));
    let Item::Function(named_alloc) = &ast.items[4] else {
        panic!("expected function named alloc");
    };
    assert_eq!(named_alloc.name, "alloc");
    assert!(named_alloc.result_allocation.is_none());
}

#[test]
fn rejects_result_allocation_modifier_on_non_callable_declarations() {
    for source in ["alloc struct Text {}\n", "pub alloc use std/io\n"] {
        let output = parse_text(source);
        assert!(output.ast.is_none(), "accepted `{source}`");
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("`alloc` applies only to callable declarations")
        }));
    }

    let output = parse_text("copy alloc func make(): Text { return build() }\n");
    assert!(output.ast.is_none());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("`copy` applies only to `struct` declarations")
    }));
}

#[test]
fn parses_native_test_declarations_as_non_callable_items() {
    let output = parse_text(
        r#"test pushes_in_order {
    let count: i32 = 1
    return
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Test(test) = &ast.items[0] else {
        panic!("expected native test declaration");
    };
    assert_eq!(test.name, "pushes_in_order");
    assert_eq!(test.body.statements.len(), 2);
}

#[test]
fn rejects_test_signatures_and_modifiers() {
    for source in [
        "pub test hidden {}\n",
        "test generic<T> {}\n",
        "test parameter(value: i32) {}\n",
        "test typed: void {}\n",
    ] {
        let output = parse_text(source);
        assert!(output.ast.is_none(), "accepted `{source}`");
        assert!(
            !output.diagnostics.is_empty(),
            "rejected without diagnostic"
        );
    }
}

#[test]
fn native_test_json_has_no_synthetic_function_signature() {
    let (sources, output) = parse_text_with_sources("test works { return }\n");
    let ast = output.ast.unwrap();
    let json = ast.to_json(&sources);
    let node = find_json_node(&json, "test_decl").expect("test JSON node");
    assert_eq!(node.value.as_deref(), Some("works"));
    assert!(find_json_node(node, "parameter_list").is_none());
    assert!(find_json_node(node, "fallible_type").is_none());
}

#[test]
fn rejects_package_directives_in_an_ordinary_module() {
    let output = parse_text("#name: \"json-tool\"\n");
    assert!(output.ast.is_none());
    assert!(!output.diagnostics.is_empty());
}

#[test]
fn rejects_removed_parenthesized_target_directive() {
    let output = parse_text(
        r#"#target("arm64-darwin")
func main(): i32 { 0 }
"#,
    );
    assert!(output.ast.is_none());
    assert!(
        output.diagnostics[0]
            .message
            .contains("expected `:` after `#target`")
    );
}

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
fn rejects_top_level_use_after_declaration() {
    let output = parse_text(
        r#"func main(): i32 {
    return 0
}

use std/io.print
"#,
    );

    assert!(output.ast.is_none());
    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert!(
        output.diagnostics[0]
            .message
            .contains("top-level `use` declarations must appear before other declarations")
    );
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
fn rejects_discard_name_for_item_declarations_and_imports() {
    for source in [
        "use std/io as _\n",
        "use std/io._\n",
        "use std/io.print as _\n",
        r#"func _(): i32 {
    return 0
}
"#,
        r#"func Owner._(): i32 {
    return 0
}
"#,
        "primitive _(): i32\n",
        "type _ = i32\n",
        r#"struct _ {
    value: i32
}
"#,
        r#"struct Pair {
    _: i32
}
"#,
        r#"enum _ {
    ready
}
"#,
        r#"enum Status {
    _
}
"#,
        r#"interface _ {
    pub method &self.ready(): bool
}
"#,
        r#"struct Counter {
    value: i32
}

impl Counter {
    method &self._(): i32 {
        return 0
    }
}
"#,
        r#"func generic<_>(): i32 {
    return 0
}
"#,
        r#"func consume(_: i32): i32 {
    return 0
}
"#,
    ] {
        assert_rejects_discard_name(source);
    }
}

#[test]
fn rejects_self_name_for_item_declarations_and_imports() {
    for source in [
        "use std/io as Self\n",
        "use std/io.Self\n",
        "use std/io.print as Self\n",
        r#"func Self(): i32 {
    return 0
}
"#,
        r#"func Owner.Self(): i32 {
    return 0
}
"#,
        "primitive Self(): i32\n",
        "type Self = i32\n",
        r#"struct Self {
    value: i32
}
"#,
        r#"struct Pair {
    Self: i32
}
"#,
        r#"enum Self {
    ready
}
"#,
        r#"enum Status {
    Self
}
"#,
        r#"interface Self {
    pub method &self.ready(): bool
}
"#,
        r#"struct Counter {
    value: i32
}

impl Counter {
    method &self.Self(): i32 {
        return 0
    }
}
"#,
        r#"func generic<Self>(): i32 {
    return 0
}
"#,
        r#"func consume(Self: i32): i32 {
    return 0
}
"#,
    ] {
        assert_rejects_self_name(source);
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
fn diagnoses_wildcard_imports_as_deferred() {
    for source in [
        "use std/io.*\n",
        "pub use std/io.*\n",
        "use std/io.{print, *}\n",
        "pub use std/io.{*}\n",
    ] {
        let output = parse_text(source);
        assert!(output.ast.is_none(), "{source}");
        assert_eq!(output.diagnostics.len(), 1, "{source}");
        assert!(
            output.diagnostics[0]
                .message
                .contains("wildcard imports are not supported"),
            "{source}: {:?}",
            output.diagnostics
        );
    }
}

#[test]
fn diagnoses_textual_include_as_deferred() {
    for source in ["include std/prelude\n", "pub include std/prelude\n"] {
        let output = parse_text(source);
        assert!(output.ast.is_none(), "{source}");
        assert_eq!(output.diagnostics.len(), 1, "{source}");
        assert!(
            output.diagnostics[0]
                .message
                .contains("textual include is not supported"),
            "{source}: {:?}",
            output.diagnostics
        );
    }
}

#[test]
fn diagnoses_dotted_import_paths_as_removed() {
    for source in [
        "use std.io.print\n",
        "pub use std.io.print\n",
        "use ./config.nct.Config\n",
    ] {
        let output = parse_text(source);
        assert!(output.ast.is_none(), "{source}");
        assert_eq!(output.diagnostics.len(), 1, "{source}");
        assert!(
            output.diagnostics[0]
                .message
                .contains("module paths use `/`"),
            "{source}: {:?}",
            output.diagnostics
        );
    }
}

#[test]
fn parses_qualified_associated_functions_inherent_methods_and_generic_params() {
    let source = r#"pub struct Counter {
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
"#;
    let output = parse_text(source);

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
    assert_eq!(method.receiver.mode, MethodReceiverMode::ReadwriteBorrow);
    let ImplMember::Drop(drop_) = &inherent_impl.members[1] else {
        panic!("expected drop member");
    };
    assert_eq!(&source[drop_.name_span.start..drop_.name_span.end], "drop");
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
    assert!(function.generics.parameters[0].bounds.is_empty());
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
            .contains("parameters cannot use `var`")
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
            .contains("parameters cannot declare default values")
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
            .contains("ordinary parameters cannot use variadic syntax")
    );
}

#[test]
fn diagnoses_prefix_variadic_parameters_as_deferred() {
    let output = parse_text(
        r#"func print_all(...values: [String]): void {
}
"#,
    );

    assert!(output.ast.is_none());
    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert!(
        output.diagnostics[0]
            .message
            .contains("ordinary parameters cannot use variadic syntax")
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
fn parses_method_generic_parameters_after_the_method_name() {
    let source = r#"struct Factory {}

impl Factory {
    method &self.identity<T: Copyable>(value: T): T {
        return value
    }
}
"#;
    let output = parse_text(source);

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Impl(impl_) = &ast.items[1] else {
        panic!("expected impl");
    };
    let ImplMember::Method(method) = &impl_.members[0] else {
        panic!("expected method");
    };
    assert_eq!(method.generics.parameters.len(), 1);
    assert_eq!(method.generics.parameters[0].name, "T");
    assert_eq!(
        &source[method.generics.parameters[0].name_span.start
            ..method.generics.parameters[0].name_span.end],
        "T"
    );
    assert_eq!(method.generics.parameters[0].bounds.len(), 1);
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
fn parses_result_provenance_contracts_on_callable_signatures() {
    let (sources, output) = parse_text_with_sources(
        r#"interface Lookup<T> {
    pub method &self.get(fallback: &T): &T from self | fallback
}

func greeting(): &str from static {
    return "hello"
}

alloc primitive allocated_text(): &str
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.expect("expected AST");
    let Item::Interface(interface) = &ast.items[0] else {
        panic!("expected interface");
    };
    let method = &interface.methods[0];
    let method_contract = method
        .result_provenance
        .as_ref()
        .expect("expected method contract");
    assert!(matches!(
        method_contract.origins[0].kind,
        crate::ast::ResultProvenanceOriginKind::Receiver
    ));
    assert!(matches!(
        &method_contract.origins[1].kind,
        crate::ast::ResultProvenanceOriginKind::Parameter(name) if name == "fallback"
    ));

    let Item::Function(function) = &ast.items[1] else {
        panic!("expected function");
    };
    assert!(matches!(
        function.result_provenance.as_ref().unwrap().origins[0].kind,
        crate::ast::ResultProvenanceOriginKind::Static
    ));
    let Item::Primitive(primitive) = &ast.items[2] else {
        panic!("expected primitive");
    };
    assert!(primitive.result_allocation.is_some());
    assert!(primitive.result_provenance.is_none());

    let json = ast.to_json(&sources);
    let provenance =
        find_json_node(&json, "result_provenance").expect("expected result provenance JSON node");
    assert_eq!(provenance.items.len(), 2);
    assert_eq!(provenance.items[0].value.as_deref(), Some("self"));
    assert_eq!(provenance.items[1].value.as_deref(), Some("fallback"));
}

#[test]
fn ast_json_preserves_method_receiver_mode_without_a_synthetic_type() {
    let (sources, output) = parse_text_with_sources(
        r#"interface Writer {
    pub method &+self.write(text: &str): void!
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap().to_json(&sources);
    let receiver = find_json_node(&ast, "method_receiver").expect("expected method receiver");

    assert_eq!(receiver.value.as_deref(), Some("readwrite_borrow"));
    assert_eq!(receiver.items.len(), 1);
    assert_eq!(receiver.items[0].kind, "parameter");
    assert_eq!(receiver.items[0].value.as_deref(), Some("self"));
    assert!(receiver.items[0].items.is_empty());
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
fn parses_interface_default_method_bodies() {
    let output = parse_text(
        r#"interface Writer {
    pub method &+self.write(text: &str): void! {
        return
    }
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.expect("expected AST");
    let Item::Interface(interface) = &ast.items[0] else {
        panic!("expected interface");
    };
    assert!(interface.methods[0].body.is_some());
}

#[test]
fn parses_interface_conformance_impls() {
    let (sources, output) = parse_text_with_sources(
        r#"interface Writer {
    pub method &+self.write(text: &str): void!
}

struct Counter {
    value: i32
}

impl Writer for Counter {
    method &+self.write(text: &str): void! {
        return
    }
}
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
    assert!(matches!(
        interface_impl.members.as_slice(),
        [ImplMember::Method(method)] if method.name == "write" && method.body.is_some()
    ));
    let json = ast.to_json(&sources);
    let impl_node = find_json_node(&json, "impl_decl").expect("expected impl JSON node");
    assert!(
        impl_node
            .items
            .iter()
            .any(|node| node.kind == "interface_type")
    );
    assert!(
        impl_node
            .items
            .iter()
            .any(|node| { node.kind == "method_decl" && node.value.as_deref() == Some("write") })
    );
}

#[test]
fn rejects_braceless_interface_implementations() {
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

    assert_eq!(output.diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert!(
        output.diagnostics[0]
            .message
            .contains("expected `{` after interface implementation target")
    );
}

#[test]
fn rejects_visibility_on_interface_implementation_members() {
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
    assert!(output.diagnostics[0].message.contains("inherit visibility"));
}

#[test]
fn parses_single_generic_interface_bounds() {
    let output = parse_text(
        r#"func print<W: Writer>(writer: &+W): void! {
    return
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.expect("ast");
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function");
    };
    let parameter = &function.generics.parameters[0];
    assert_eq!(parameter.name, "W");
    assert!(matches!(
        parameter.bounds.first(),
        Some(TypeExpr::Reference(reference)) if reference.name == "Writer"
    ));
}

#[test]
fn parses_multiple_generic_interface_bounds() {
    let output = parse_text(
        r#"func inspect<I: Iterator<T> + ExactSizeIterator<T>, T>(iterator: &I): usize {
    return iterator.remaining_len()
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.expect("ast");
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function");
    };
    let parameter = &function.generics.parameters[0];
    assert_eq!(parameter.name, "I");
    assert_eq!(parameter.bounds.len(), 2);
    assert!(matches!(
        &parameter.bounds[0],
        TypeExpr::Generic(generic) if generic.name == "Iterator"
    ));
    assert!(matches!(
        &parameter.bounds[1],
        TypeExpr::Generic(generic) if generic.name == "ExactSizeIterator"
    ));
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
fn parses_include_as_ordinary_function_name() {
    let output = parse_text(
        r#"func include(): void {
    return
}
"#,
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ast = output.ast.unwrap();
    let Item::Function(function) = &ast.items[0] else {
        panic!("expected function");
    };
    assert_eq!(function.name, "include");
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
fn reserves_literal_for_literal_definitions() {
    let output = parse_text(
        r#"func literal(): void {
    return
}
"#,
    );

    assert!(output.ast.is_none());
    assert!(
        output.diagnostics[0]
            .message
            .contains("expected function name")
    );
}

#[test]
fn rejects_the_obsolete_capture_inside_shape_marker() {
    let output = parse_text(
        r#"construct Vec<T> {
    pub default literal [...items: [T]]: Self {
    return Self.empty()
    }
}
"#,
    );

    assert!(output.ast.is_none());
    assert!(!output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert!(
        output.diagnostics[0]
            .message
            .contains("sequence shape marker")
    );
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
fn diagnoses_embedding_declarations_as_deferred() {
    for source in [
        r#"struct Profile {
    ...User
}
"#,
        r#"struct Profile {
    pub ...User
}
"#,
    ] {
        let output = parse_text(source);

        assert!(output.ast.is_none(), "{source}");
        assert_eq!(
            output.diagnostics.len(),
            1,
            "{source}: {:?}",
            output.diagnostics
        );
        assert!(
            output.diagnostics[0]
                .message
                .contains("embedding declarations are not supported"),
            "{source}: {:?}",
            output.diagnostics
        );
    }
}

#[test]
fn parses_target_directive_on_primitive_declaration() {
    let (sources, output) = parse_text_with_sources(
        r#"#target: "arm64-darwin"
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
        r#"#target: "arm64-darwin"
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
        r#"#target: "arm64-darwin"
pub(nocter) type RawWord = usize

#target: "arm64-darwin"
pub(nocter) copy struct SyscallResult {
    pub value: usize
    pub errno: i32
}

#target: "arm64-darwin"
pub(nocter) enum PlatformError {
    interrupted
}

#target: "arm64-darwin"
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
