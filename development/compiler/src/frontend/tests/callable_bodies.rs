use super::super::{FrontendOptions, load_compile_unit};
use super::support::{make_nocter_home, make_temp_project};
use crate::analysis::analyze_executable_compile_unit;
use crate::source::SourceMap;
use crate::target::DEFAULT_TARGET;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

static PROJECT_ID: AtomicU64 = AtomicU64::new(0);

fn analyze_sources(
    root_text: &str,
    implementations: &[(&str, &str)],
) -> Vec<crate::diagnostics::Diagnostic> {
    let root = make_temp_project(&format!(
        "callable-bodies-{}",
        PROJECT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let home = make_nocter_home(&root);
    crate::test_files::write(root.join("index.nct"), root_text).unwrap();
    for (name, text) in implementations {
        crate::test_files::write(root.join(name), text).unwrap();
    }
    let mut sources = SourceMap::new();
    let source = sources.load_file(root.join("index.nct")).unwrap();
    let result = load_compile_unit(
        &mut sources,
        source,
        &FrontendOptions {
            nocter_home: Some(home),
            package_graph: None,
            target: DEFAULT_TARGET.to_string(),
        },
    )
    .map(|unit| analyze_executable_compile_unit(&sources, &unit).diagnostics())
    .unwrap_or_else(|diagnostics| diagnostics);
    fs::remove_dir_all(root).unwrap();
    result
}

#[test]
fn source_backed_function_contract_typechecks_as_one_callable() {
    let diagnostics = analyze_sources(
        r#"use ./answer

pub func answer(value: i32): i32

func main(): i32 {
    return answer(40)
}
"#,
        &[(
            "answer.nct",
            r#"func answer(value: i32): i32 {
    return value + 2
}
"#,
        )],
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn missing_source_backed_body_is_rejected() {
    let diagnostics = analyze_sources(
        r#"pub func answer(): i32

func main(): i32 {
    return 0
}
"#,
        &[],
    );
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0250");
}

#[test]
fn source_backed_body_signature_must_match_exactly() {
    let diagnostics = analyze_sources(
        r#"use ./answer

pub func answer(value: i32): i32

func main(): i32 {
    return 0
}
"#,
        &[(
            "answer.nct",
            r#"func answer(value: usize): i32 {
    return 0
}
"#,
        )],
    );
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0251", "{diagnostics:?}");
    assert_eq!(diagnostics[0].notes.len(), 1);
}

#[test]
fn source_backed_body_is_unique_across_composed_sources() {
    let diagnostics = analyze_sources(
        r#"use ./first
use ./second

pub func answer(): i32

func main(): i32 {
    return 0
}
"#,
        &[
            ("first.nct", "func answer(): i32 { return 1 }\n"),
            ("second.nct", "func answer(): i32 { return 2 }\n"),
        ],
    );
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0252", "{diagnostics:?}");
    assert_eq!(diagnostics[0].notes.len(), 2);
}

#[test]
fn private_bodyless_callable_is_not_a_contract() {
    let diagnostics = analyze_sources(
        r#"func answer(): i32

func main(): i32 {
    return 0
}
"#,
        &[],
    );
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0253");
}

#[test]
fn associated_function_and_method_contracts_share_their_private_bodies() {
    let diagnostics = analyze_sources(
        r#"use ./box_impl

pub struct Box {
    value: i32,
}

construct Box {
    pub default func new(value: i32): Self
}

impl Box {
    pub method &self.get(): i32
}

func main(): i32 {
    let value = Box.new(42)
    return value.get()
}
"#,
        &[(
            "box_impl.nct",
            r#"construct Box {
    func new(value: i32): Self {
        return Box { value: value }
    }
}

impl Box {
    method &self.get(): i32 {
        return self.value
    }
}
"#,
        )],
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn associated_function_contracts_are_distinguished_by_owner() {
    let diagnostics = analyze_sources(
        r#"use ./constructors

pub struct Left { value: i32 }
pub struct Right { value: i32 }

construct Left {
    pub default func new(value: i32): Self
}

construct Right {
    pub default func new(value: i32): Self
}

func main(): i32 {
    let left = Left.new(20)
    let right = Right.new(22)
    return left.value + right.value
}
"#,
        &[(
            "constructors.nct",
            r#"construct Left {
    func new(value: i32): Self {
        return Left { value: value }
    }
}

construct Right {
    func new(value: i32): Self {
        return Right { value: value }
    }
}
"#,
        )],
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn literal_and_coercion_contracts_share_their_private_bodies() {
    let diagnostics = analyze_sources(
        r#"use ./text_impl

pub struct Text {
    value: &str,
}

construct Text {
    pub default literal ""(text: &str): Self
}

coerce Text {
    pub &self as &str from self
}

func main(): i32 {
    let text = Text "hello"
    let view: &str = &text as &str
    return 0
}
"#,
        &[(
            "text_impl.nct",
            r#"construct Text {
    literal ""(text: &str): Self {
        return Text { value: text }
    }
}

coerce Text {
    &self as &str from self {
        return self.value
    }
}
"#,
        )],
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}
