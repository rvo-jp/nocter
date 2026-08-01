use super::*;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

#[test]
fn decodes_file_uri_percent_encoding() {
    assert_eq!(
        file_uri_to_path("file:///tmp/nocter%20test/app.nct"),
        Some(PathBuf::from("/tmp/nocter test/app.nct"))
    );
}

#[test]
fn converts_byte_offsets_to_utf16_positions() {
    let text = "a\néx\n";
    assert_eq!(
        byte_offset_to_lsp_position(text, 0),
        LspPosition {
            line: 0,
            character: 0
        }
    );
    assert_eq!(
        byte_offset_to_lsp_position(text, 4),
        LspPosition {
            line: 1,
            character: 1
        }
    );
}

#[test]
fn handles_initialize_request() {
    let mut output = Vec::new();

    run_lsp_stream(
        Cursor::new(frame(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {}
        }))),
        &mut output,
    )
    .unwrap();

    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("\"id\":1"));
    assert!(text.contains("\"textDocumentSync\""));
    assert!(text.contains("\"semanticTokensProvider\""));
    assert!(text.contains("\"hoverProvider\""));
    assert!(text.contains("\"definitionProvider\""));
    assert!(text.contains("\"referencesProvider\""));
    assert!(text.contains("\"documentSymbolProvider\""));
    assert!(text.contains("\"completionProvider\""));
    assert!(text.contains("\"signatureHelpProvider\""));
}

#[test]
fn returns_specialized_signature_help_for_imported_generic_call() {
    let project = TempProject::new("lsp-signature-help-generic-import");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    let app_text = r#"use ./math.identity

func main(): i32 {
    return identity(42)
}
"#;
    let math_text = r#"/// Returns the provided value.
pub func identity<T>(value: T): T {
    return value
}
"#;
    let app = project.write_source("app.nct", app_text);
    project.write_source("math.nct", math_text);
    let uri = file_uri(&app);
    let document = open_document(uri.clone(), Some(1), app_text.to_string());
    let server = LspServer {
        documents: HashMap::from([(uri.clone(), document)]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };
    let position = byte_offset_to_lsp_position(
        app_text,
        app_text.find("42").expect("expected call argument"),
    );

    let response = server.signature_help_response(
        json!(2),
        Some(&json!({
            "textDocument": { "uri": uri },
            "position": position
        })),
    );

    assert_eq!(
        response["result"]["signatures"][0]["label"],
        json!("func identity<i32>(value: i32): i32")
    );
    assert_eq!(response["result"]["activeParameter"], json!(0));
    assert_eq!(
        response["result"]["signatures"][0]["documentation"]["value"],
        json!("Returns the provided value.")
    );
}

#[test]
fn initializes_with_semantic_token_legend() {
    let response = initialize_response(json!(1));
    let legend = response["result"]["capabilities"]["semanticTokensProvider"]["legend"]
        .as_object()
        .expect("expected semantic token legend");

    assert_eq!(
        legend["tokenTypes"],
        json!([
            "function",
            "method",
            "variable",
            "parameter",
            "type",
            "property"
        ])
    );
    assert_eq!(legend["tokenModifiers"], json!(["declaration", "readonly"]));
}

#[test]
fn converts_utf16_positions_to_byte_offsets() {
    let text = "a\néx\n";
    assert_eq!(lsp_position_to_byte_offset(text, 0, 0), 0);
    assert_eq!(lsp_position_to_byte_offset(text, 1, 0), 2);
    assert_eq!(lsp_position_to_byte_offset(text, 1, 1), 4);
    assert_eq!(lsp_position_to_byte_offset(text, 1, 2), 5);
}

#[test]
fn returns_semantic_tokens_for_open_document() {
    let uri = "file:///tmp/nocter-semantic.nct".to_string();
    let document = open_document(
        uri.clone(),
        Some(1),
        "func main(path: AppError): i32 {\n    let code = AppError.open_failed(path)\n    return code\n}\n"
            .to_string(),
    );
    let server = LspServer {
        documents: HashMap::from([(uri.clone(), document)]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let response = server.semantic_tokens_response(
        json!(2),
        Some(&json!({
            "textDocument": {
                "uri": uri
            }
        })),
    );
    let data = response["result"]["data"]
        .as_array()
        .expect("expected semantic token data");

    assert!(!data.is_empty());
    assert_eq!(data.len() % 5, 0);
    assert_eq!(
        data[3],
        json!(semantic_token_kind_index(SemanticTokenKind::Function))
    );
    assert_eq!(data[4], json!(SEMANTIC_DECLARATION_MODIFIER));
}

#[test]
fn single_file_semantic_tokens_classify_builtin_types() {
    let text = "func main(path: &str): void! {\n    let byte: u8 = 0 as u8\n    let count: usize = 0 as usize\n    return\n}\n\nfunc fail(error_value: error): never {\n    return\n}\n";
    let document = open_document(
        "file:///tmp/nocter-semantic-single-file-types.nct".to_string(),
        Some(1),
        text.to_string(),
    );
    let identifiers = classified_identifiers(&document);

    for name in ["str", "void", "u8", "usize", "error", "never"] {
        assert!(
            classified_identifier_with_lexeme(text, &identifiers, name)
                .iter()
                .any(|identifier| identifier.kind == SemanticTokenKind::Type),
            "expected semantic tokens to classify `{name}` as a type"
        );
    }
}

#[test]
fn semantic_tokens_are_empty_when_document_cannot_be_analyzed() {
    let uri = "file:///tmp/nocter-bad-semantic.nct".to_string();
    let document = open_document(
        uri.clone(),
        Some(1),
        "func main(: i32 {\n    value\n".to_string(),
    );
    let server = LspServer {
        documents: HashMap::from([(uri.clone(), document)]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let response = server.semantic_tokens_response(
        json!(2),
        Some(&json!({
            "textDocument": {
                "uri": uri
            }
        })),
    );

    assert_eq!(response["result"]["data"], json!([]));
}

#[test]
fn semantic_tokens_do_not_classify_unresolved_identifiers_or_module_paths() {
    let text = "use ./missing.nope\n\nfunc main(): i32 {\n    return value\n}\n";
    let document = open_document(
        "file:///tmp/nocter-semantic-unresolved.nct".to_string(),
        Some(1),
        text.to_string(),
    );
    let identifiers = classified_identifiers(&document);

    for lexeme in ["missing", "nope", "value"] {
        assert!(
            classified_identifier_with_lexeme(text, &identifiers, lexeme).is_empty(),
            "expected `{lexeme}` to remain uncolored, got {identifiers:#?}"
        );
    }
}

#[test]
fn classifies_builtin_types_and_methods_for_semantic_tokens() {
    let project = TempProject::new("lsp-semantic-methods");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    let text = "struct File {\n    fd: i32\n    byte: u8\n}\n\nimpl File {\n    method self.read(): i32 {\n        return 0\n    }\n}\n\nfunc main(path: &str): i32 {\n    var file = File { fd: 0, byte: 0 as u8 }\n    return file.read()\n}\n";
    let app = project.write_source("app.nct", text);
    let uri = file_uri(&app);
    let document = open_document(uri.clone(), Some(1), text.to_string());
    let documents = HashMap::from([(uri.clone(), document)]);
    let workspace =
        workspace_analysis_for_uri(&uri, &documents).expect("expected workspace analysis");
    let file = workspace.root_file().expect("expected analyzed file");
    let identifiers = classified_identifiers_for_file_analysis(documents.get(&uri).unwrap(), file);

    for name in ["i32", "u8", "str"] {
        assert!(
            classified_identifier_with_lexeme(text, &identifiers, name)
                .iter()
                .any(|identifier| identifier.kind == SemanticTokenKind::Type),
            "expected `{name}` to be classified as a type"
        );
    }

    let read_identifiers = classified_identifier_with_lexeme(text, &identifiers, "read");
    assert!(
        read_identifiers.iter().any(|identifier| {
            identifier.kind == SemanticTokenKind::Method
                && identifier.modifiers == SEMANTIC_DECLARATION_MODIFIER
        }),
        "expected method declaration name to be classified as a method declaration"
    );
    assert!(
        read_identifiers.iter().any(|identifier| {
            identifier.kind == SemanticTokenKind::Method && identifier.modifiers == 0
        }),
        "expected method call name to be classified as a method"
    );
}

#[test]
fn classifies_block_imported_function_name_for_semantic_tokens() {
    let project = TempProject::new("lsp-semantic-block-import");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    let app_text = "func main(): i32 {\n    use ./config.answer\n\n    return answer()\n}\n";
    let config_text = "pub func answer(): i32 {\n    return 42\n}\n";
    let app = project.write_source("app.nct", app_text);
    let config = project.write_source("config.nct", config_text);
    let app_uri = file_uri(&app);
    let config_uri = file_uri(&config);
    let documents = HashMap::from([
        (
            app_uri.clone(),
            open_document(app_uri.clone(), Some(1), app_text.to_string()),
        ),
        (
            config_uri,
            open_document(file_uri(&config), Some(1), config_text.to_string()),
        ),
    ]);
    let workspace =
        workspace_analysis_for_uri(&app_uri, &documents).expect("expected workspace analysis");
    let file = workspace.root_file().expect("expected analyzed file");
    let identifiers =
        classified_identifiers_for_file_analysis(documents.get(&app_uri).unwrap(), file);

    let import_name = classified_identifier_starting_at(
        &identifiers,
        app_text.find("./config.answer").unwrap() + "./config.".len(),
    )
    .expect("expected semantic token for block import name");
    assert_eq!(import_name.kind, SemanticTokenKind::Function);
    assert!(
        import_name.modifiers & SEMANTIC_DECLARATION_MODIFIER != 0,
        "expected block import name to be classified as a declaration"
    );

    let call_name = classified_identifier_starting_at(
        &identifiers,
        app_text.rfind("answer()").expect("expected answer call"),
    )
    .expect("expected semantic token for imported function call");
    assert_eq!(call_name.kind, SemanticTokenKind::Function);
    assert_eq!(call_name.modifiers & SEMANTIC_DECLARATION_MODIFIER, 0);
}

#[test]
fn marks_readonly_bindings_for_semantic_tokens() {
    let project = TempProject::new("lsp-semantic-readonly-bindings");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    let text = "func main(value: i32, path: &str): i32 {\n    let alpha = value\n    var beta = 2\n    return alpha + beta\n}\n";
    let app = project.write_source("app.nct", text);
    let uri = file_uri(&app);
    let document = open_document(uri.clone(), Some(1), text.to_string());
    let documents = HashMap::from([(uri.clone(), document)]);
    let workspace =
        workspace_analysis_for_uri(&uri, &documents).expect("expected workspace analysis");
    let file = workspace.root_file().expect("expected analyzed file");
    let identifiers = classified_identifiers_for_file_analysis(documents.get(&uri).unwrap(), file);

    for name in ["value", "path", "alpha"] {
        let identifiers = classified_identifier_with_lexeme(text, &identifiers, name);
        assert!(
            !identifiers.is_empty(),
            "expected semantic tokens for `{name}`"
        );
        assert!(
            identifiers
                .iter()
                .all(|identifier| identifier.modifiers & SEMANTIC_READONLY_MODIFIER != 0),
            "expected `{name}` to be marked readonly"
        );
    }

    let beta_identifiers = classified_identifier_with_lexeme(text, &identifiers, "beta");
    assert!(
        !beta_identifiers.is_empty(),
        "expected semantic tokens for `beta`"
    );
    assert!(
        beta_identifiers
            .iter()
            .all(|identifier| identifier.modifiers & SEMANTIC_READONLY_MODIFIER == 0),
        "expected `beta` to remain mutable"
    );
}

#[test]
fn marks_readonly_field_accesses_for_semantic_tokens() {
    let project = TempProject::new("lsp-semantic-readonly-fields");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    let text = r#"struct Header {
code: i32
}

func inspect(value: Header, readonly: &Header, readwrite: &+Header): i32 {
let fixed = Header { code: 1 }
var mutable = Header { code: 2 }
let readwrite_alias = readwrite
var readonly_alias = readonly
return fixed.code + mutable.code + value.code + readonly.code + readwrite.code + readwrite_alias.code + readonly_alias.code
}
"#;
    let app = project.write_source("app.nct", text);
    let uri = file_uri(&app);
    let document = open_document(uri.clone(), Some(1), text.to_string());
    let documents = HashMap::from([(uri.clone(), document)]);
    let workspace =
        workspace_analysis_for_uri(&uri, &documents).expect("expected workspace analysis");
    let file = workspace.root_file().expect("expected analyzed file");
    let identifiers = classified_identifiers_for_file_analysis(documents.get(&uri).unwrap(), file);

    for access in [
        "fixed.code",
        "value.code",
        "readonly.code",
        "readonly_alias.code",
    ] {
        let identifier = classified_identifier_starting_at(
            &identifiers,
            field_name_offset_for_access(text, access),
        )
        .unwrap_or_else(|| panic!("expected semantic token for `{access}`"));
        assert_eq!(
            identifier.kind,
            SemanticTokenKind::Property,
            "expected `{access}` to be classified as a property"
        );
        assert!(
            identifier.modifiers & SEMANTIC_READONLY_MODIFIER != 0,
            "expected `{access}` to be marked readonly because `=` cannot target it"
        );
    }

    for access in ["mutable.code", "readwrite.code", "readwrite_alias.code"] {
        let identifier = classified_identifier_starting_at(
            &identifiers,
            field_name_offset_for_access(text, access),
        )
        .unwrap_or_else(|| panic!("expected semantic token for `{access}`"));
        assert_eq!(
            identifier.kind,
            SemanticTokenKind::Property,
            "expected `{access}` to be classified as a property"
        );
        assert!(
            identifier.modifiers & SEMANTIC_READONLY_MODIFIER == 0,
            "expected `{access}` to remain writable because `=` can target it"
        );
    }

    let first_literal_field = classified_identifier_starting_at(
        &identifiers,
        text.find("Header { code").unwrap() + "Header { ".len(),
    )
    .expect("expected semantic token for struct literal field label");
    assert_eq!(first_literal_field.kind, SemanticTokenKind::Property);
    assert_eq!(
        first_literal_field.modifiers & SEMANTIC_READONLY_MODIFIER,
        0,
        "struct literal field labels are not readonly declarations"
    );
}

#[test]
fn returns_null_hover_when_document_cannot_be_analyzed() {
    let uri = "file:///tmp/nocter-bad-hover.nct".to_string();
    let document = open_document(
        uri.clone(),
        Some(1),
        "func main(: i32 {\n    value\n".to_string(),
    );
    let server = LspServer {
        documents: HashMap::from([(uri.clone(), document)]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let response = server.hover_response(
        json!(3),
        Some(&json!({
            "textDocument": {
                "uri": uri
            },
            "position": {
                "line": 1,
                "character": 5
            }
        })),
    );

    assert_eq!(response["result"], Value::Null);
}

#[test]
fn returns_null_hover_for_unresolved_identifier() {
    let uri = "file:///tmp/nocter-unresolved-hover.nct".to_string();
    let document = open_document(
        uri.clone(),
        Some(1),
        "func main(): i32 {\n    return value\n}\n".to_string(),
    );
    let server = LspServer {
        documents: HashMap::from([(uri.clone(), document)]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let response = server.hover_response(
        json!(3),
        Some(&json!({
            "textDocument": {
                "uri": uri
            },
            "position": {
                "line": 1,
                "character": 12
            }
        })),
    );

    assert_eq!(response["result"], Value::Null);
}

#[test]
fn returns_hover_for_identifier() {
    let uri = "file:///tmp/nocter-hover.nct".to_string();
    let document = open_document(
        uri.clone(),
        Some(1),
        "func main(): i32 {\n    let answer = compute()\n    return answer\n}\n".to_string(),
    );
    let server = LspServer {
        documents: HashMap::from([(uri.clone(), document)]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let response = server.hover_response(
        json!(3),
        Some(&json!({
            "textDocument": {
                "uri": uri
            },
            "position": {
                "line": 1,
                "character": 9
            }
        })),
    );

    assert_eq!(
        response["result"]["contents"]["value"],
        json!("```nocter\nlet answer\n```")
    );
    assert_eq!(response["result"]["range"]["start"]["line"], json!(1));
    assert_eq!(response["result"]["range"]["start"]["character"], json!(8));
}

#[test]
fn returns_hover_for_local_reference() {
    let project = TempProject::new("lsp-hover-local-reference");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    let text = "func main(path: &str): i32 {\n    let code = 0\n    return code\n}\n";
    let app = project.write_source("app.nct", text);
    let uri = file_uri(&app);
    let document = open_document(uri.clone(), Some(1), text.to_string());
    let server = LspServer {
        documents: HashMap::from([(uri.clone(), document)]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let response = server.hover_response(
        json!(4),
        Some(&json!({
            "textDocument": {
                "uri": uri
            },
            "position": {
                "line": 2,
                "character": 12
            }
        })),
    );

    assert_eq!(
        response["result"]["contents"]["value"],
        json!("```nocter\nlet code: i32\n```")
    );
    assert_eq!(response["result"]["range"]["start"]["line"], json!(2));
    assert_eq!(response["result"]["range"]["start"]["character"], json!(11));
}

#[test]
fn returns_documented_hover_for_function_declaration() {
    let uri = "file:///tmp/nocter-hover-docs.nct".to_string();
    let document = open_document(
        uri.clone(),
        Some(1),
        "/// **Computes** the answer.\nfunc answer(path: &str): i32 {\n    return 0\n}\n"
            .to_string(),
    );
    let server = LspServer {
        documents: HashMap::from([(uri.clone(), document)]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let response = server.hover_response(
        json!(4),
        Some(&json!({
            "textDocument": {
                "uri": uri
            },
            "position": {
                "line": 1,
                "character": 6
            }
        })),
    );

    assert_eq!(
        response["result"]["contents"]["value"],
        json!("```nocter\nfunc answer(path: &str): i32\n```\n\n**Computes** the answer.")
    );
    assert_eq!(response["result"]["range"]["start"]["line"], json!(1));
    assert_eq!(response["result"]["range"]["start"]["character"], json!(5));
}

#[test]
fn returns_documented_hover_for_type_reference() {
    let uri = "file:///tmp/nocter-hover-type-reference.nct".to_string();
    let document = open_document(
        uri.clone(),
        Some(1),
        "/// Request header.\nstruct Header {\n    code: i32\n}\n\nfunc inspect(value: Header): i32 {\n    return value.code\n}\n"
            .to_string(),
    );
    let server = LspServer {
        documents: HashMap::from([(uri.clone(), document)]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let response = server.hover_response(
        json!(5),
        Some(&json!({
            "textDocument": {
                "uri": uri
            },
            "position": {
                "line": 5,
                "character": 21
            }
        })),
    );

    assert_eq!(
        response["result"]["contents"]["value"],
        json!("```nocter\nstruct Header\n```\n\nRequest header.")
    );
    assert_eq!(response["result"]["range"]["start"]["line"], json!(5));
    assert_eq!(response["result"]["range"]["start"]["character"], json!(20));
}

#[test]
fn returns_markdown_hover_for_import_module_path() {
    let project = TempProject::new("lsp-hover-import-module");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    std::fs::write(
        home.join("std/io.nct"),
        "//! **I/O** module.\n//!\n//! Provides file and text APIs.\n\npub func print(text: &str): void! {\n    return\n}\n",
    )
    .unwrap();
    let app = project.write_source(
        "app.nct",
        "use std/io.print\n\nfunc main(): i32 {\n    return 0\n}\n",
    );
    let app_uri = file_uri(&app);
    let server = LspServer {
        documents: HashMap::from([(
            app_uri.clone(),
            open_document(
                app_uri.clone(),
                Some(1),
                "use std/io.print\n\nfunc main(): i32 {\n    return 0\n}\n".to_string(),
            ),
        )]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let response = server.hover_response(
        json!(6),
        Some(&json!({
            "textDocument": {
                "uri": app_uri
            },
            "position": {
                "line": 0,
                "character": 7
            }
        })),
    );

    assert_eq!(
        response["result"]["contents"]["value"],
        json!("```nocter\nmodule std/io\n```\n\n**I/O** module.\nProvides file and text APIs.")
    );
    assert_eq!(response["result"]["range"]["start"]["line"], json!(0));
    assert_eq!(response["result"]["range"]["start"]["character"], json!(4));
    assert_eq!(response["result"]["range"]["end"]["character"], json!(10));
}

#[test]
fn returns_markdown_hover_for_block_import_module_path() {
    let project = TempProject::new("lsp-hover-block-import-module");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    std::fs::write(
        home.join("std/io.nct"),
        "//! **I/O** module.\n//!\n//! Provides file and text APIs.\n\npub func print(text: &str): void! {\n    return\n}\n",
    )
    .unwrap();
    let text = "func main(): i32 {\n    use std/io.print\n    return 0\n}\n";
    let app = project.write_source("app.nct", text);
    let app_uri = file_uri(&app);
    let server = LspServer {
        documents: HashMap::from([(
            app_uri.clone(),
            open_document(app_uri.clone(), Some(1), text.to_string()),
        )]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let response = server.hover_response(
        json!(6),
        Some(&json!({
            "textDocument": {
                "uri": app_uri
            },
            "position": {
                "line": 1,
                "character": 11
            }
        })),
    );

    assert_eq!(
        response["result"]["contents"]["value"],
        json!("```nocter\nmodule std/io\n```\n\n**I/O** module.\nProvides file and text APIs.")
    );
    assert_eq!(response["result"]["range"]["start"]["line"], json!(1));
    assert_eq!(response["result"]["range"]["start"]["character"], json!(8));
    assert_eq!(response["result"]["range"]["end"]["character"], json!(14));
}

#[test]
fn returns_documented_hover_for_local_binding_declaration() {
    let uri = "file:///tmp/nocter-hover-local-docs.nct".to_string();
    let document = open_document(
        uri.clone(),
        Some(1),
        "func main(): i32 {\n    /// Exit code.\n    let code: i32 = 0\n    return code\n}\n"
            .to_string(),
    );
    let server = LspServer {
        documents: HashMap::from([(uri.clone(), document)]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let response = server.hover_response(
        json!(5),
        Some(&json!({
            "textDocument": {
                "uri": uri
            },
            "position": {
                "line": 2,
                "character": 9
            }
        })),
    );

    assert_eq!(
        response["result"]["contents"]["value"],
        json!("```nocter\nlet code: i32\n```\n\nExit code.")
    );
}

#[test]
fn returns_documented_hover_for_local_binding_reference() {
    let uri = "file:///tmp/nocter-hover-local-reference-docs.nct".to_string();
    let document = open_document(
        uri.clone(),
        Some(1),
        "func main(): i32 {\n    /// Exit code.\n    let code: i32 = 0\n    return code\n}\n"
            .to_string(),
    );
    let server = LspServer {
        documents: HashMap::from([(uri.clone(), document)]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let response = server.hover_response(
        json!(6),
        Some(&json!({
            "textDocument": {
                "uri": uri
            },
            "position": {
                "line": 3,
                "character": 12
            }
        })),
    );

    assert_eq!(
        response["result"]["contents"]["value"],
        json!("```nocter\nlet code: i32\n```\n\nExit code.")
    );
}

#[test]
fn returns_inferred_hover_for_integer_binding() {
    let project = TempProject::new("lsp-hover-inferred-binding");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    let text = "func main(): i32 {\n    var count = 0\n    return count\n}\n";
    let app = project.write_source("app.nct", text);
    let uri = file_uri(&app);
    let document = open_document(uri.clone(), Some(1), text.to_string());
    let server = LspServer {
        documents: HashMap::from([(uri.clone(), document)]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let response = server.hover_response(
        json!(7),
        Some(&json!({
            "textDocument": {
                "uri": uri
            },
            "position": {
                "line": 2,
                "character": 12
            }
        })),
    );

    assert_eq!(
        response["result"]["contents"]["value"],
        json!("```nocter\nvar count: i32\n```")
    );
}

#[test]
fn returns_short_visible_type_names_for_hover() {
    let project = TempProject::new("lsp-hover-short-type-names");
    let home = project.write_nocter_home();
    std::fs::write(
        home.join("std/string.nct"),
        "pub copy struct String {\n    ptr: usize\n}\n",
    )
    .unwrap();
    let _home = NocterHomeEnv::set(&home);
    let text = "use std/string.String\n\nstruct TestStruct {\n    field3: String\n}\n\nfunc inspect(value: TestStruct): String {\n    let result = value.field3\n    return result\n}\n";
    let app = project.write_source("app.nct", text);
    let uri = file_uri(&app);
    let document = open_document(uri.clone(), Some(1), text.to_string());
    let server = LspServer {
        documents: HashMap::from([(uri.clone(), document)]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let field_declaration = server.hover_response(
        json!(7),
        Some(&json!({
            "textDocument": {
                "uri": uri
            },
            "position": {
                "line": 3,
                "character": 5
            }
        })),
    );
    let field_reference = server.hover_response(
        json!(8),
        Some(&json!({
            "textDocument": {
                "uri": uri
            },
            "position": {
                "line": 7,
                "character": 25
            }
        })),
    );
    let inferred_binding = server.hover_response(
        json!(9),
        Some(&json!({
            "textDocument": {
                "uri": uri
            },
            "position": {
                "line": 8,
                "character": 12
            }
        })),
    );

    assert_eq!(
        field_declaration["result"]["contents"]["value"],
        json!("```nocter\nfield field3: String\n```")
    );
    assert_eq!(
        field_reference["result"]["contents"]["value"],
        json!("```nocter\nfield TestStruct.field3: String\n```")
    );
    assert_eq!(
        inferred_binding["result"]["contents"]["value"],
        json!("```nocter\nlet result: String\n```")
    );
}

#[test]
fn shortens_hidden_canonical_type_names_for_hover() {
    let project = TempProject::new("lsp-hover-hidden-canonical-type-names");
    let home = project.write_nocter_home();
    std::fs::write(
        home.join("std/string.nct"),
        "pub copy struct String {\n    ptr: usize\n}\n",
    )
    .unwrap();
    let _home = NocterHomeEnv::set(&home);
    let app = project.write_source(
        "app.nct",
        "use ./config.make\n\nfunc main(): i32 {\n    let holder = make()\n    return 0\n}\n",
    );
    let config = project.write_source(
        "config.nct",
        "use std/string.String\n\npub copy struct Box {\n    value: String\n}\n\npub func make(): Box {\n    return Box { value: String { ptr: 0 } }\n}\n",
    );
    let app_uri = file_uri(&app);
    let config_uri = file_uri(&config);
    let server = LspServer {
        documents: HashMap::from([
            (
                app_uri.clone(),
                open_document(
                    app_uri.clone(),
                    Some(1),
                    "use ./config.make\n\nfunc main(): i32 {\n    let holder = make()\n    return 0\n}\n"
                        .to_string(),
                ),
            ),
            (
                config_uri,
                open_document(
                    file_uri(&config),
                    Some(1),
                    "use std/string.String\n\npub copy struct Box {\n    value: String\n}\n\npub func make(): Box {\n    return Box { value: String { ptr: 0 } }\n}\n"
                        .to_string(),
                ),
            ),
        ]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let box_binding = server.hover_response(
        json!(10),
        Some(&json!({
            "textDocument": {
                "uri": app_uri
            },
            "position": {
                "line": 3,
                "character": 12
            }
        })),
    );

    assert_eq!(
        box_binding["result"]["contents"]["value"],
        json!("```nocter\nlet holder: Box\n```")
    );
}

#[test]
fn returns_documented_workspace_hover_for_local_binding_reference() {
    let project = TempProject::new("lsp-hover-local-reference-docs");
    let app = project.write_source(
        "app.nct",
        "func main(): i32 {\n    /// Exit code.\n    let code: i32 = 0\n    return code\n}\n",
    );
    let app_uri = file_uri(&app);
    let server = LspServer {
        documents: HashMap::from([(
            app_uri.clone(),
            open_document(
                app_uri.clone(),
                Some(1),
                "func main(): i32 {\n    /// Exit code.\n    let code: i32 = 0\n    return code\n}\n"
                    .to_string(),
            ),
        )]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let response = server.hover_response(
        json!(7),
        Some(&json!({
            "textDocument": {
                "uri": app_uri
            },
            "position": {
                "line": 3,
                "character": 12
            }
        })),
    );

    assert_eq!(
        response["result"]["contents"]["value"],
        json!("```nocter\nlet code: i32\n```\n\nExit code.")
    );
}

#[test]
fn returns_documented_hover_for_resolved_function_reference() {
    let uri = "file:///tmp/nocter-hover-reference-docs.nct".to_string();
    let document = open_document(
        uri.clone(),
        Some(1),
        "func main(): i32 {\n    return answer()\n}\n\n/// Computes the answer.\nfunc answer(): i32 {\n    return 42\n}\n"
            .to_string(),
    );
    let server = LspServer {
        documents: HashMap::from([(uri.clone(), document)]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let response = server.hover_response(
        json!(6),
        Some(&json!({
            "textDocument": {
                "uri": uri
            },
            "position": {
                "line": 1,
                "character": 12
            }
        })),
    );

    assert_eq!(
        response["result"]["contents"]["value"],
        json!("```nocter\nfunc answer(): i32\n```\n\nComputes the answer.")
    );
    assert_eq!(response["result"]["range"]["start"]["line"], json!(1));
    assert_eq!(response["result"]["range"]["start"]["character"], json!(11));
}

#[test]
fn returns_documented_hover_for_imported_function_reference() {
    let project = TempProject::new("lsp-hover-import");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    let app = project.write_source(
        "app.nct",
        "use ./config.answer\n\nfunc main(): i32 {\n    return answer()\n}\n",
    );
    let config = project.write_source(
        "config.nct",
        "/// Returns the configured answer.\npub func answer(): i32 {\n    return 42\n}\n",
    );
    let app_uri = file_uri(&app);
    let config_uri = file_uri(&config);
    let server = LspServer {
        documents: HashMap::from([
            (
                app_uri.clone(),
                open_document(
                    app_uri.clone(),
                    Some(1),
                    "use ./config.answer\n\nfunc main(): i32 {\n    return answer()\n}\n"
                        .to_string(),
                ),
            ),
            (
                config_uri.clone(),
                open_document(
                    config_uri,
                    Some(1),
                    "/// Returns the configured answer.\npub func answer(): i32 {\n    return 42\n}\n"
                        .to_string(),
                ),
            ),
        ]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let response = server.hover_response(
        json!(7),
        Some(&json!({
            "textDocument": {
                "uri": app_uri
            },
            "position": {
                "line": 3,
                "character": 12
            }
        })),
    );

    assert_eq!(
        response["result"]["contents"]["value"],
        json!("```nocter\nfunc answer(): i32\n```\n\nReturns the configured answer.")
    );
    assert_eq!(response["result"]["range"]["start"]["line"], json!(3));
    assert_eq!(response["result"]["range"]["start"]["character"], json!(11));
}

#[test]
fn returns_documented_hover_for_namespace_imported_function_member_reference() {
    let project = TempProject::new("lsp-hover-namespace-import");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    let app_text = "use ./config\n\nfunc main(): i32 {\n    return config.answer()\n}\n";
    let config_text =
        "/// Returns the configured answer.\npub func answer(): i32 {\n    return 42\n}\n";
    let app = project.write_source("app.nct", app_text);
    let config = project.write_source("config.nct", config_text);
    let app_uri = file_uri(&app);
    let config_uri = file_uri(&config);
    let server = LspServer {
        documents: HashMap::from([
            (
                app_uri.clone(),
                open_document(app_uri.clone(), Some(1), app_text.to_string()),
            ),
            (
                config_uri,
                open_document(file_uri(&config), Some(1), config_text.to_string()),
            ),
        ]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let response = server.hover_response(
        json!(7),
        Some(&json!({
            "textDocument": {
                "uri": app_uri
            },
            "position": {
                "line": 3,
                "character": 20
            }
        })),
    );

    assert_eq!(
        response["result"]["contents"]["value"],
        json!("```nocter\nfunc answer(): i32\n```\n\nReturns the configured answer.")
    );
    assert_eq!(response["result"]["range"]["start"]["line"], json!(3));
    assert_eq!(response["result"]["range"]["start"]["character"], json!(18));
    assert_eq!(response["result"]["range"]["end"]["character"], json!(24));
}

#[test]
fn returns_documented_hover_for_imported_type_reference() {
    let project = TempProject::new("lsp-hover-imported-type");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    let app = project.write_source(
        "app.nct",
        "use ./config.Config\n\nfunc main(): i32 {\n    var config: Config = Config { value: 0 }\n    return 0\n}\n",
    );
    let config = project.write_source(
        "config.nct",
        "/// Runtime configuration.\npub struct Config {\n    value: i32\n}\n",
    );
    let app_uri = file_uri(&app);
    let config_uri = file_uri(&config);
    let server = LspServer {
        documents: HashMap::from([
            (
                app_uri.clone(),
                open_document(
                    app_uri.clone(),
                    Some(1),
                    "use ./config.Config\n\nfunc main(): i32 {\n    var config: Config = Config { value: 0 }\n    return 0\n}\n"
                        .to_string(),
                ),
            ),
            (
                config_uri,
                open_document(
                    file_uri(&config),
                    Some(1),
                    "/// Runtime configuration.\npub struct Config {\n    value: i32\n}\n"
                        .to_string(),
                ),
            ),
        ]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let response = server.hover_response(
        json!(8),
        Some(&json!({
            "textDocument": {
                "uri": app_uri
            },
            "position": {
                "line": 3,
                "character": 17
            }
        })),
    );

    assert_eq!(
        response["result"]["contents"]["value"],
        json!("```nocter\nstruct Config\n```\n\nRuntime configuration.")
    );
    assert_eq!(response["result"]["range"]["start"]["line"], json!(3));
    assert_eq!(response["result"]["range"]["start"]["character"], json!(16));
}

#[test]
fn returns_definition_for_resolved_function_reference() {
    let uri = "file:///tmp/nocter-definition-reference.nct".to_string();
    let document = open_document(
        uri.clone(),
        Some(1),
        "func main(): i32 {\n    return answer()\n}\n\nfunc answer(): i32 {\n    return 42\n}\n"
            .to_string(),
    );
    let server = LspServer {
        documents: HashMap::from([(uri.clone(), document)]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let response = server.definition_response(
        json!(8),
        Some(&json!({
            "textDocument": {
                "uri": uri
            },
            "position": {
                "line": 1,
                "character": 12
            }
        })),
    );

    assert_eq!(response["result"]["range"]["start"]["line"], json!(4));
    assert_eq!(response["result"]["range"]["start"]["character"], json!(5));
    assert_eq!(response["result"]["range"]["end"]["character"], json!(11));
}

#[test]
fn returns_definition_for_local_reference() {
    let uri = "file:///tmp/nocter-definition-local.nct".to_string();
    let document = open_document(
        uri.clone(),
        Some(1),
        "func main(path: &str): i32 {\n    let code = 0\n    return code\n}\n".to_string(),
    );
    let server = LspServer {
        documents: HashMap::from([(uri.clone(), document)]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let response = server.definition_response(
        json!(9),
        Some(&json!({
            "textDocument": {
                "uri": uri
            },
            "position": {
                "line": 2,
                "character": 12
            }
        })),
    );

    assert_eq!(response["result"]["range"]["start"]["line"], json!(1));
    assert_eq!(response["result"]["range"]["start"]["character"], json!(8));
    assert_eq!(response["result"]["range"]["end"]["character"], json!(12));
}

#[test]
fn returns_references_for_local_binding() {
    let uri = "file:///tmp/nocter-references-local.nct".to_string();
    let document = open_document(
        uri.clone(),
        Some(1),
        "func main(): i32 {\n    let code = 0\n    return code + code\n}\n".to_string(),
    );
    let server = LspServer {
        documents: HashMap::from([(uri.clone(), document)]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let response = server.references_response(
        json!(10),
        Some(&json!({
            "textDocument": {
                "uri": uri
            },
            "position": {
                "line": 1,
                "character": 9
            },
            "context": {
                "includeDeclaration": true
            }
        })),
    );
    let references = response["result"].as_array().expect("expected references");

    assert_eq!(references.len(), 3);
    assert_eq!(references[0]["range"]["start"]["line"], json!(1));
    assert_eq!(references[0]["range"]["start"]["character"], json!(8));
    assert_eq!(references[1]["range"]["start"]["line"], json!(2));
    assert_eq!(references[1]["range"]["start"]["character"], json!(11));
    assert_eq!(references[2]["range"]["start"]["line"], json!(2));
    assert_eq!(references[2]["range"]["start"]["character"], json!(18));
}

#[test]
fn returns_definition_for_imported_function_reference() {
    let project = TempProject::new("lsp-definition-import");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    let app = project.write_source(
        "app.nct",
        "use ./config.answer\n\nfunc main(): i32 {\n    return answer()\n}\n",
    );
    let config = project.write_source("config.nct", "pub func answer(): i32 {\n    return 42\n}\n");
    let app_uri = file_uri(&app);
    let config_uri = file_uri(&config);
    let server = LspServer {
        documents: HashMap::from([
            (
                app_uri.clone(),
                open_document(
                    app_uri.clone(),
                    Some(1),
                    "use ./config.answer\n\nfunc main(): i32 {\n    return answer()\n}\n"
                        .to_string(),
                ),
            ),
            (
                config_uri.clone(),
                open_document(
                    config_uri.clone(),
                    Some(1),
                    "pub func answer(): i32 {\n    return 42\n}\n".to_string(),
                ),
            ),
        ]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let response = server.definition_response(
        json!(9),
        Some(&json!({
            "textDocument": {
                "uri": app_uri
            },
            "position": {
                "line": 3,
                "character": 12
            }
        })),
    );

    assert_eq!(response["result"]["uri"], json!(config_uri));
    assert_eq!(response["result"]["range"]["start"]["line"], json!(0));
    assert_eq!(response["result"]["range"]["start"]["character"], json!(9));
    assert_eq!(response["result"]["range"]["end"]["character"], json!(15));
}

#[test]
fn returns_definition_for_namespace_imported_function_member_reference() {
    let project = TempProject::new("lsp-definition-namespace-import");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    let app_text = "use ./config\n\nfunc main(): i32 {\n    return config.answer()\n}\n";
    let config_text = "pub func answer(): i32 {\n    return 42\n}\n";
    let app = project.write_source("app.nct", app_text);
    let config = project.write_source("config.nct", config_text);
    let app_uri = file_uri(&app);
    let config_uri = file_uri(&config);
    let server = LspServer {
        documents: HashMap::from([
            (
                app_uri.clone(),
                open_document(app_uri.clone(), Some(1), app_text.to_string()),
            ),
            (
                config_uri.clone(),
                open_document(config_uri.clone(), Some(1), config_text.to_string()),
            ),
        ]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let response = server.definition_response(
        json!(9),
        Some(&json!({
            "textDocument": {
                "uri": app_uri
            },
            "position": {
                "line": 3,
                "character": 20
            }
        })),
    );

    assert_eq!(response["result"]["uri"], json!(config_uri));
    assert_eq!(response["result"]["range"]["start"]["line"], json!(0));
    assert_eq!(response["result"]["range"]["start"]["character"], json!(9));
    assert_eq!(response["result"]["range"]["end"]["character"], json!(15));
}

#[test]
fn returns_references_for_imported_function_reference() {
    let project = TempProject::new("lsp-references-import");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    let app = project.write_source(
        "app.nct",
        "use ./config.answer\n\nfunc main(): i32 {\n    return answer()\n}\n",
    );
    let config = project.write_source("config.nct", "pub func answer(): i32 {\n    return 42\n}\n");
    let app_uri = file_uri(&app);
    let config_uri = file_uri(&config);
    let server = LspServer {
        documents: HashMap::from([
            (
                app_uri.clone(),
                open_document(
                    app_uri.clone(),
                    Some(1),
                    "use ./config.answer\n\nfunc main(): i32 {\n    return answer()\n}\n"
                        .to_string(),
                ),
            ),
            (
                config_uri.clone(),
                open_document(
                    config_uri.clone(),
                    Some(1),
                    "pub func answer(): i32 {\n    return 42\n}\n".to_string(),
                ),
            ),
        ]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let response = server.references_response(
        json!(10),
        Some(&json!({
            "textDocument": {
                "uri": app_uri
            },
            "position": {
                "line": 3,
                "character": 12
            },
            "context": {
                "includeDeclaration": true
            }
        })),
    );
    let references = response["result"].as_array().expect("expected references");

    assert_eq!(references.len(), 3);
    assert_eq!(references[0]["uri"], json!(app_uri));
    assert_eq!(references[0]["range"]["start"]["line"], json!(0));
    assert_eq!(references[0]["range"]["start"]["character"], json!(13));
    assert_eq!(references[1]["uri"], json!(app_uri));
    assert_eq!(references[1]["range"]["start"]["line"], json!(3));
    assert_eq!(references[1]["range"]["start"]["character"], json!(11));
    assert_eq!(references[2]["uri"], json!(config_uri));
    assert_eq!(references[2]["range"]["start"]["line"], json!(0));
    assert_eq!(references[2]["range"]["start"]["character"], json!(9));
}

#[test]
fn returns_references_for_block_imported_function_reference() {
    let project = TempProject::new("lsp-references-block-import");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    let app_text =
        "func main(): i32 {\n    use ./config.answer\n\n    return answer() + answer()\n}\n";
    let config_text = "pub func answer(): i32 {\n    return 42\n}\n";
    let app = project.write_source("app.nct", app_text);
    let config = project.write_source("config.nct", config_text);
    let app_uri = file_uri(&app);
    let config_uri = file_uri(&config);
    let server = LspServer {
        documents: HashMap::from([
            (
                app_uri.clone(),
                open_document(app_uri.clone(), Some(1), app_text.to_string()),
            ),
            (
                config_uri.clone(),
                open_document(config_uri.clone(), Some(1), config_text.to_string()),
            ),
        ]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let response = server.references_response(
        json!(10),
        Some(&json!({
            "textDocument": {
                "uri": app_uri
            },
            "position": {
                "line": 3,
                "character": 12
            },
            "context": {
                "includeDeclaration": true
            }
        })),
    );
    let references = response["result"].as_array().expect("expected references");

    assert_eq!(references.len(), 4);
    assert_eq!(references[0]["uri"], json!(app_uri));
    assert_eq!(references[0]["range"]["start"]["line"], json!(1));
    assert_eq!(references[0]["range"]["start"]["character"], json!(17));
    assert_eq!(references[1]["uri"], json!(app_uri));
    assert_eq!(references[1]["range"]["start"]["line"], json!(3));
    assert_eq!(references[1]["range"]["start"]["character"], json!(11));
    assert_eq!(references[2]["uri"], json!(app_uri));
    assert_eq!(references[2]["range"]["start"]["line"], json!(3));
    assert_eq!(references[2]["range"]["start"]["character"], json!(22));
    assert_eq!(references[3]["uri"], json!(config_uri));
    assert_eq!(references[3]["range"]["start"]["line"], json!(0));
    assert_eq!(references[3]["range"]["start"]["character"], json!(9));
}

#[test]
fn returns_references_for_namespace_imported_function_member_reference() {
    let project = TempProject::new("lsp-references-namespace-import");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    let app_text =
        "use ./config\n\nfunc main(): i32 {\n    return config.answer() + config.answer()\n}\n";
    let config_text = "pub func answer(): i32 {\n    return 42\n}\n";
    let app = project.write_source("app.nct", app_text);
    let config = project.write_source("config.nct", config_text);
    let app_uri = file_uri(&app);
    let config_uri = file_uri(&config);
    let server = LspServer {
        documents: HashMap::from([
            (
                app_uri.clone(),
                open_document(app_uri.clone(), Some(1), app_text.to_string()),
            ),
            (
                config_uri.clone(),
                open_document(config_uri.clone(), Some(1), config_text.to_string()),
            ),
        ]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let response = server.references_response(
        json!(10),
        Some(&json!({
            "textDocument": {
                "uri": app_uri
            },
            "position": {
                "line": 3,
                "character": 20
            },
            "context": {
                "includeDeclaration": true
            }
        })),
    );
    let references = response["result"].as_array().expect("expected references");

    assert_eq!(references.len(), 3);
    assert_eq!(references[0]["uri"], json!(app_uri));
    assert_eq!(references[0]["range"]["start"]["line"], json!(3));
    assert_eq!(references[0]["range"]["start"]["character"], json!(18));
    assert_eq!(references[1]["uri"], json!(app_uri));
    assert_eq!(references[1]["range"]["start"]["line"], json!(3));
    assert_eq!(references[1]["range"]["start"]["character"], json!(36));
    assert_eq!(references[2]["uri"], json!(config_uri));
    assert_eq!(references[2]["range"]["start"]["line"], json!(0));
    assert_eq!(references[2]["range"]["start"]["character"], json!(9));
}

#[test]
fn returns_definition_for_import_module_path() {
    let project = TempProject::new("lsp-definition-import-module");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    std::fs::write(
        home.join("std/io.nct"),
        "//! I/O module.\n\npub func print(text: &str): void! {\n    return\n}\n",
    )
    .unwrap();
    let app = project.write_source(
        "app.nct",
        "use std/io.print\n\nfunc main(): i32 {\n    return 0\n}\n",
    );
    let app_uri = file_uri(&app);
    let io = home.join("std/io.nct").canonicalize().unwrap();
    let server = LspServer {
        documents: HashMap::from([(
            app_uri.clone(),
            open_document(
                app_uri.clone(),
                Some(1),
                "use std/io.print\n\nfunc main(): i32 {\n    return 0\n}\n".to_string(),
            ),
        )]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let response = server.definition_response(
        json!(9),
        Some(&json!({
            "textDocument": {
                "uri": app_uri
            },
            "position": {
                "line": 0,
                "character": 7
            }
        })),
    );

    assert_eq!(response["result"]["uri"], json!(file_uri(&io)));
    assert_eq!(response["result"]["range"]["start"]["line"], json!(0));
    assert_eq!(response["result"]["range"]["start"]["character"], json!(0));
    assert_eq!(response["result"]["range"]["end"]["line"], json!(0));
    assert_eq!(response["result"]["range"]["end"]["character"], json!(0));
}

#[test]
fn returns_definition_for_block_import_module_path() {
    let project = TempProject::new("lsp-definition-block-import-module");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    std::fs::write(
        home.join("std/io.nct"),
        "//! I/O module.\n\npub func print(text: &str): void! {\n    return\n}\n",
    )
    .unwrap();
    let text = "func main(): i32 {\n    use std/io.print\n    return 0\n}\n";
    let app = project.write_source("app.nct", text);
    let app_uri = file_uri(&app);
    let io = home.join("std/io.nct").canonicalize().unwrap();
    let server = LspServer {
        documents: HashMap::from([(
            app_uri.clone(),
            open_document(app_uri.clone(), Some(1), text.to_string()),
        )]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let response = server.definition_response(
        json!(9),
        Some(&json!({
            "textDocument": {
                "uri": app_uri
            },
            "position": {
                "line": 1,
                "character": 11
            }
        })),
    );

    assert_eq!(response["result"]["uri"], json!(file_uri(&io)));
    assert_eq!(response["result"]["range"]["start"]["line"], json!(0));
    assert_eq!(response["result"]["range"]["start"]["character"], json!(0));
    assert_eq!(response["result"]["range"]["end"]["line"], json!(0));
    assert_eq!(response["result"]["range"]["end"]["character"], json!(0));
}

#[test]
fn returns_definition_for_imported_type_reference() {
    let project = TempProject::new("lsp-definition-imported-type");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    let app = project.write_source(
        "app.nct",
        "use ./config.Config\n\nfunc main(): i32 {\n    var config: Config = Config { value: 0 }\n    return 0\n}\n",
    );
    let config = project.write_source("config.nct", "pub struct Config {\n    value: i32\n}\n");
    let app_uri = file_uri(&app);
    let config_uri = file_uri(&config);
    let server = LspServer {
        documents: HashMap::from([
            (
                app_uri.clone(),
                open_document(
                    app_uri.clone(),
                    Some(1),
                    "use ./config.Config\n\nfunc main(): i32 {\n    var config: Config = Config { value: 0 }\n    return 0\n}\n"
                        .to_string(),
                ),
            ),
            (
                config_uri,
                open_document(
                    file_uri(&config),
                    Some(1),
                    "pub struct Config {\n    value: i32\n}\n".to_string(),
                ),
            ),
        ]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let response = server.definition_response(
        json!(9),
        Some(&json!({
            "textDocument": {
                "uri": app_uri
            },
            "position": {
                "line": 3,
                "character": 17
            }
        })),
    );

    assert_eq!(response["result"]["uri"], json!(file_uri(&config)));
    assert_eq!(response["result"]["range"]["start"]["line"], json!(0));
    assert_eq!(response["result"]["range"]["start"]["character"], json!(11));
    assert_eq!(response["result"]["range"]["end"]["character"], json!(17));
}

#[test]
fn returns_definition_for_method_call() {
    let project = TempProject::new("lsp-definition-method-call");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    let text = "struct File {\n    fd: i32\n}\n\nimpl File {\n    method self.read(): i32 {\n        return 0\n    }\n}\n\nfunc main(): i32 {\n    let file = File { fd: 1 }\n    return file.read()\n}\n";
    let app = project.write_source("app.nct", text);
    let app_uri = file_uri(&app);
    let server = LspServer {
        documents: HashMap::from([(
            app_uri.clone(),
            open_document(app_uri.clone(), Some(1), text.to_string()),
        )]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let response = server.definition_response(
        json!(9),
        Some(&json!({
            "textDocument": {
                "uri": app_uri
            },
            "position": {
                "line": 12,
                "character": 17
            }
        })),
    );

    assert_eq!(response["result"]["range"]["start"]["line"], json!(5));
    assert_eq!(response["result"]["range"]["start"]["character"], json!(16));
    assert_eq!(response["result"]["range"]["end"]["character"], json!(20));
}

#[test]
fn returns_document_symbols_for_top_level_declarations() {
    let uri = "file:///tmp/nocter-document-symbols.nct".to_string();
    let document = open_document(
        uri.clone(),
        Some(1),
        "struct Config {\n    path: &str\n}\n\nenum Mode {\n    fast\n    slow\n}\n\nfunc main(): i32 {\n    return 0\n}\n"
            .to_string(),
    );
    let server = LspServer {
        documents: HashMap::from([(uri.clone(), document)]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let response = server.document_symbol_response(
        json!(10),
        Some(&json!({
            "textDocument": {
                "uri": uri
            }
        })),
    );
    let symbols = response["result"]
        .as_array()
        .expect("expected document symbols");

    assert_eq!(symbols.len(), 3);
    assert_eq!(symbols[0]["name"], json!("Config"));
    assert_eq!(symbols[0]["kind"], json!(LSP_SYMBOL_KIND_STRUCT));
    assert_eq!(symbols[0]["children"][0]["name"], json!("path"));
    assert_eq!(
        symbols[0]["children"][0]["kind"],
        json!(LSP_SYMBOL_KIND_FIELD)
    );
    assert_eq!(symbols[1]["name"], json!("Mode"));
    assert_eq!(
        symbols[1]["children"][0]["kind"],
        json!(LSP_SYMBOL_KIND_ENUM_MEMBER)
    );
    assert_eq!(symbols[2]["name"], json!("main"));
    assert_eq!(symbols[2]["kind"], json!(LSP_SYMBOL_KIND_FUNCTION));
}

#[test]
fn returns_completion_items_for_keywords_and_top_level_symbols() {
    let uri = "file:///tmp/nocter-completion.nct".to_string();
    let document = open_document(
        uri.clone(),
        Some(1),
        "struct Config {\n    path: &str\n}\n\nfunc answer(): i32 {\n    return 42\n}\n"
            .to_string(),
    );
    let server = LspServer {
        documents: HashMap::from([(uri.clone(), document)]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let response = server.completion_response(
        json!(11),
        Some(&json!({
            "textDocument": {
                "uri": uri
            },
            "position": {
                "line": 5,
                "character": 4
            }
        })),
    );
    let items = response["result"]["items"]
        .as_array()
        .expect("expected completion items");

    assert!(completion_item_with_label(items, "return").is_some());
    assert!(completion_item_with_label(items, "loop").is_some());
    assert!(completion_item_with_label(items, "primitive").is_some());
    assert!(completion_item_with_label(items, "void").is_some());
    assert!(completion_item_with_label(items, "drop").is_none());
    assert_eq!(
        completion_item_with_label(items, "answer").and_then(|item| item["kind"].as_u64()),
        Some(LSP_COMPLETION_ITEM_KIND_FUNCTION as u64)
    );
    assert_eq!(
        completion_item_with_label(items, "Config").and_then(|item| item["kind"].as_u64()),
        Some(LSP_COMPLETION_ITEM_KIND_STRUCT as u64)
    );
}

#[test]
fn returns_completion_items_for_imported_symbols() {
    let project = TempProject::new("lsp-completion-import");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    let app = project.write_source(
        "app.nct",
        "use ./config.answer\n\nfunc main(): i32 {\n    return answer()\n}\n",
    );
    let config = project.write_source("config.nct", "pub func answer(): i32 {\n    return 42\n}\n");
    let app_uri = file_uri(&app);
    let config_uri = file_uri(&config);
    let server = LspServer {
        documents: HashMap::from([
            (
                app_uri.clone(),
                open_document(
                    app_uri.clone(),
                    Some(1),
                    "use ./config.answer\n\nfunc main(): i32 {\n    return answer()\n}\n"
                        .to_string(),
                ),
            ),
            (
                config_uri.clone(),
                open_document(
                    config_uri,
                    Some(1),
                    "pub func answer(): i32 {\n    return 42\n}\n".to_string(),
                ),
            ),
        ]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let response = server.completion_response(
        json!(12),
        Some(&json!({
            "textDocument": {
                "uri": app_uri
            },
            "position": {
                "line": 3,
                "character": 4
            }
        })),
    );
    let items = response["result"]["items"]
        .as_array()
        .expect("expected completion items");

    assert_eq!(
        completion_item_with_label(items, "answer").and_then(|item| item["kind"].as_u64()),
        Some(LSP_COMPLETION_ITEM_KIND_FUNCTION as u64)
    );
}

#[test]
fn returns_completion_items_for_enum_pattern_members() {
    let project = TempProject::new("lsp-completion-enum-pattern-members");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    let text = r#"enum Choice {
hit(value: i32)
miss
}

func main(choice: Choice): i32 {
if choice is Choice.hit(_) {
}
return match choice {
    Choice.hit(_) { 1 }
    Choice.miss { 2 }
}
}
"#;
    let app = project.write_source("app.nct", text);
    let uri = file_uri(&app);
    let document = open_document(uri.clone(), Some(1), text.to_string());
    let server = LspServer {
        documents: HashMap::from([(uri.clone(), document)]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };
    let offset = text.find("Choice.hit").expect("expected if-is pattern") + "Choice.".len();
    let position = byte_offset_to_lsp_position(text, offset);

    let response = server.completion_response(
        json!(14),
        Some(&json!({
            "textDocument": {
                "uri": uri
            },
            "position": {
                "line": position.line,
                "character": position.character
            }
        })),
    );
    let items = response["result"]["items"]
        .as_array()
        .expect("expected completion items");

    assert_eq!(
        completion_item_with_label(items, "hit").and_then(|item| item["kind"].as_u64()),
        Some(LSP_COMPLETION_ITEM_KIND_ENUM_MEMBER as u64)
    );
    assert_eq!(
        completion_item_with_label(items, "miss").and_then(|item| item["kind"].as_u64()),
        Some(LSP_COMPLETION_ITEM_KIND_ENUM_MEMBER as u64)
    );
    assert!(completion_item_with_label(items, "Choice").is_none());
}

#[test]
fn returns_completion_items_for_type_members() {
    let uri = "file:///tmp/nocter-type-member-completion.nct".to_string();
    let text = r#"enum Choice {
yes
no
}

struct File {
fd: i32
}

func File.open(): File {
return File { fd: 1 }
}

func main(): i32 {
let choice = Choice.yes
let file = File.open()
return file.fd
}
"#;
    let document = open_document(uri.clone(), Some(1), text.to_string());
    let server = LspServer {
        documents: HashMap::from([(uri.clone(), document)]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let variant_offset = text.find("Choice.yes").expect("expected enum member") + "Choice.".len();
    let variant_position = byte_offset_to_lsp_position(text, variant_offset);
    let variant_response = server.completion_response(
        json!(15),
        Some(&json!({
            "textDocument": {
                "uri": uri
            },
            "position": {
                "line": variant_position.line,
                "character": variant_position.character
            }
        })),
    );
    let variant_items = variant_response["result"]["items"]
        .as_array()
        .expect("expected enum member completion items");

    assert_eq!(
        completion_item_with_label(variant_items, "yes").and_then(|item| item["kind"].as_u64()),
        Some(LSP_COMPLETION_ITEM_KIND_ENUM_MEMBER as u64)
    );
    assert_eq!(
        completion_item_with_label(variant_items, "no").and_then(|item| item["kind"].as_u64()),
        Some(LSP_COMPLETION_ITEM_KIND_ENUM_MEMBER as u64)
    );
    assert!(completion_item_with_label(variant_items, "Choice").is_none());

    let function_offset = text
        .rfind("File.open")
        .expect("expected associated function call")
        + "File.".len();
    let function_position = byte_offset_to_lsp_position(text, function_offset);
    let function_response = server.completion_response(
        json!(16),
        Some(&json!({
            "textDocument": {
                "uri": uri
            },
            "position": {
                "line": function_position.line,
                "character": function_position.character
            }
        })),
    );
    let function_items = function_response["result"]["items"]
        .as_array()
        .expect("expected associated function completion items");
    let open_item =
        completion_item_with_label(function_items, "open").expect("expected open completion");

    assert_eq!(
        open_item["kind"].as_u64(),
        Some(LSP_COMPLETION_ITEM_KIND_FUNCTION as u64)
    );
    assert_eq!(open_item["detail"].as_str(), Some("func open(): File"));
    assert_eq!(open_item["insertText"].as_str(), Some("open()"));
    assert!(completion_item_with_label(function_items, "File").is_none());
}

#[test]
fn returns_completion_items_for_value_members() {
    let uri = "file:///tmp/nocter-value-member-completion.nct".to_string();
    let text = r#"struct File {
fd: i32
size: i32
}

impl File {
method &self.describe(): i32 {
    return self.size
}
}

func main(): i32 {
let file = File { fd: 1, size: 2 }
return file.fd
}
"#;
    let document = open_document(uri.clone(), Some(1), text.to_string());
    let server = LspServer {
        documents: HashMap::from([(uri.clone(), document)]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let offset = text.rfind("file.fd").expect("expected field access") + "file.".len();
    let position = byte_offset_to_lsp_position(text, offset);
    let response = server.completion_response(
        json!(17),
        Some(&json!({
            "textDocument": {
                "uri": uri
            },
            "position": {
                "line": position.line,
                "character": position.character
            }
        })),
    );
    let items = response["result"]["items"]
        .as_array()
        .expect("expected value member completion items");
    let fd_item = completion_item_with_label(items, "fd").expect("expected fd completion");
    let describe_item =
        completion_item_with_label(items, "describe").expect("expected describe completion");

    assert_eq!(
        fd_item["kind"].as_u64(),
        Some(LSP_COMPLETION_ITEM_KIND_FIELD as u64)
    );
    assert_eq!(fd_item["detail"].as_str(), Some("field fd: i32"));
    assert_eq!(fd_item["insertText"].as_str(), Some("fd"));
    assert_eq!(
        describe_item["kind"].as_u64(),
        Some(LSP_COMPLETION_ITEM_KIND_METHOD as u64)
    );
    assert_eq!(
        describe_item["detail"].as_str(),
        Some("method &File.describe(): i32")
    );
    assert_eq!(describe_item["insertText"].as_str(), Some("describe()"));
    assert!(completion_item_with_label(items, "File").is_none());
}

#[test]
fn returns_completion_items_for_incomplete_value_member_dot() {
    let uri = "file:///tmp/nocter-incomplete-value-member-completion.nct".to_string();
    let text = r#"struct File {
fd: i32
}

impl File {
method &self.describe(): i32 {
    return self.fd
}
}

func main(): i32 {
let file = File { fd: 1 }
return file.
}
"#;
    let document = open_document(uri.clone(), Some(1), text.to_string());
    let server = LspServer {
        documents: HashMap::from([(uri.clone(), document)]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let offset = text
        .rfind("file.")
        .expect("expected incomplete field access")
        + "file.".len();
    let position = byte_offset_to_lsp_position(text, offset);
    let response = server.completion_response(
        json!(18),
        Some(&json!({
            "textDocument": {
                "uri": uri
            },
            "position": {
                "line": position.line,
                "character": position.character
            }
        })),
    );
    let items = response["result"]["items"]
        .as_array()
        .expect("expected value member completion items");

    assert_eq!(
        completion_item_with_label(items, "fd").and_then(|item| item["kind"].as_u64()),
        Some(LSP_COMPLETION_ITEM_KIND_FIELD as u64)
    );
    assert_eq!(
        completion_item_with_label(items, "describe").and_then(|item| item["kind"].as_u64()),
        Some(LSP_COMPLETION_ITEM_KIND_METHOD as u64)
    );
    assert!(completion_item_with_label(items, "File").is_none());
    assert!(completion_item_with_label(items, "return").is_none());
}

#[test]
fn returns_completion_items_for_struct_literal_fields() {
    let uri = "file:///tmp/nocter-struct-literal-field-completion.nct".to_string();
    let text = r#"struct File {
fd: i32
size: i32
}

func main(): i32 {
let file = File { fd: 1,  }
return 0
}
"#;
    let document = open_document(uri.clone(), Some(1), text.to_string());
    let server = LspServer {
        documents: HashMap::from([(uri.clone(), document)]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let offset = text
        .find("File { fd: 1,  }")
        .expect("expected struct literal")
        + "File { fd: 1, ".len();
    let position = byte_offset_to_lsp_position(text, offset);
    let response = server.completion_response(
        json!(19),
        Some(&json!({
            "textDocument": {
                "uri": uri
            },
            "position": {
                "line": position.line,
                "character": position.character
            }
        })),
    );
    let items = response["result"]["items"]
        .as_array()
        .expect("expected struct literal field completion items");

    assert_eq!(
        completion_item_with_label(items, "size").and_then(|item| item["kind"].as_u64()),
        Some(LSP_COMPLETION_ITEM_KIND_FIELD as u64)
    );
    assert!(completion_item_with_label(items, "fd").is_none());
    assert!(completion_item_with_label(items, "File").is_none());
}

#[test]
fn returns_completion_items_for_incomplete_struct_literal_fields() {
    let uri = "file:///tmp/nocter-incomplete-struct-literal-field-completion.nct".to_string();
    let text = r#"struct File {
fd: i32
size: i32
}

func main(): i32 {
let file = File {
return 0
}
"#;
    let document = open_document(uri.clone(), Some(1), text.to_string());
    let server = LspServer {
        documents: HashMap::from([(uri.clone(), document)]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let offset = text
        .find("let file = File {")
        .expect("expected struct literal")
        + "let file = File {".len();
    let position = byte_offset_to_lsp_position(text, offset);
    let response = server.completion_response(
        json!(20),
        Some(&json!({
            "textDocument": {
                "uri": uri
            },
            "position": {
                "line": position.line,
                "character": position.character
            }
        })),
    );
    let items = response["result"]["items"]
        .as_array()
        .expect("expected struct literal field completion items");

    assert_eq!(
        completion_item_with_label(items, "fd").and_then(|item| item["kind"].as_u64()),
        Some(LSP_COMPLETION_ITEM_KIND_FIELD as u64)
    );
    assert_eq!(
        completion_item_with_label(items, "size").and_then(|item| item["kind"].as_u64()),
        Some(LSP_COMPLETION_ITEM_KIND_FIELD as u64)
    );
    assert!(completion_item_with_label(items, "File").is_none());
    assert!(completion_item_with_label(items, "return").is_none());
}

#[test]
fn returns_completion_items_for_namespace_import_without_member_leakage() {
    let project = TempProject::new("lsp-completion-namespace-import");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    let app_text = "use ./config\n\nfunc main(): i32 {\n    return config.answer()\n}\n";
    let config_text = "pub func answer(): i32 {\n    return 42\n}\n";
    let app = project.write_source("app.nct", app_text);
    let config = project.write_source("config.nct", config_text);
    let app_uri = file_uri(&app);
    let config_uri = file_uri(&config);
    let server = LspServer {
        documents: HashMap::from([
            (
                app_uri.clone(),
                open_document(app_uri.clone(), Some(1), app_text.to_string()),
            ),
            (
                config_uri,
                open_document(file_uri(&config), Some(1), config_text.to_string()),
            ),
        ]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let response = server.completion_response(
        json!(12),
        Some(&json!({
            "textDocument": {
                "uri": app_uri
            },
            "position": {
                "line": 3,
                "character": 4
            }
        })),
    );
    let items = response["result"]["items"]
        .as_array()
        .expect("expected completion items");

    assert_eq!(
        completion_item_with_label(items, "config").and_then(|item| item["kind"].as_u64()),
        Some(LSP_COMPLETION_ITEM_KIND_MODULE as u64)
    );
    assert!(completion_item_with_label(items, "answer").is_none());
}

#[test]
fn returns_completion_items_for_block_imports_only_inside_scope() {
    let project = TempProject::new("lsp-completion-block-import");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    let app_text = r#"func main(): i32 {
use ./config.answer

return answer()
}

func other(): i32 {
return 0
}
"#;
    let config_text = "pub func answer(): i32 {\n    return 42\n}\n";
    let app = project.write_source("app.nct", app_text);
    let config = project.write_source("config.nct", config_text);
    let app_uri = file_uri(&app);
    let config_uri = file_uri(&config);
    let server = LspServer {
        documents: HashMap::from([
            (
                app_uri.clone(),
                open_document(app_uri.clone(), Some(1), app_text.to_string()),
            ),
            (
                config_uri,
                open_document(file_uri(&config), Some(1), config_text.to_string()),
            ),
        ]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };

    let inside_response = server.completion_response(
        json!(12),
        Some(&json!({
            "textDocument": {
                "uri": app_uri
            },
            "position": {
                "line": 3,
                "character": 4
            }
        })),
    );
    let inside_items = inside_response["result"]["items"]
        .as_array()
        .expect("expected completion items");
    assert_eq!(
        completion_item_with_label(inside_items, "answer").and_then(|item| item["kind"].as_u64()),
        Some(LSP_COMPLETION_ITEM_KIND_FUNCTION as u64)
    );

    let outside_response = server.completion_response(
        json!(13),
        Some(&json!({
            "textDocument": {
                "uri": app_uri
            },
            "position": {
                "line": 7,
                "character": 4
            }
        })),
    );
    let outside_items = outside_response["result"]["items"]
        .as_array()
        .expect("expected completion items");
    assert!(completion_item_with_label(outside_items, "answer").is_none());
}

#[test]
fn initialize_stores_workspace_folders() {
    let mut server = LspServer::new();
    let mut output = Vec::new();

    server
        .handle_message(
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "rootUri": "file:///tmp/ignored-root",
                    "workspaceFolders": [
                        {
                            "uri": "file:///tmp/nocter-workspace-a",
                            "name": "workspace-a"
                        },
                        {
                            "uri": "file:///tmp/nocter-workspace-b",
                            "name": "workspace-b"
                        }
                    ]
                }
            }),
            &mut output,
        )
        .unwrap();

    assert_eq!(
        server.workspace_roots,
        vec![
            WorkspaceRoot {
                uri: "file:///tmp/nocter-workspace-a".to_string(),
                path: Some(PathBuf::from("/tmp/nocter-workspace-a")),
            },
            WorkspaceRoot {
                uri: "file:///tmp/nocter-workspace-b".to_string(),
                path: Some(PathBuf::from("/tmp/nocter-workspace-b")),
            },
        ]
    );
}

#[test]
fn initialize_falls_back_to_root_uri() {
    let mut server = LspServer::new();
    let mut output = Vec::new();

    server
        .handle_message(
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "rootUri": "file:///tmp/nocter-root"
                }
            }),
            &mut output,
        )
        .unwrap();

    assert_eq!(
        server.workspace_roots,
        vec![WorkspaceRoot {
            uri: "file:///tmp/nocter-root".to_string(),
            path: Some(PathBuf::from("/tmp/nocter-root")),
        }]
    );
}

#[test]
fn publishes_diagnostics_for_open_document() {
    let mut output = Vec::new();
    let input = frame(&json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": "file:///tmp/nocter-lsp-test.nct",
                "languageId": "nocter",
                "version": 1,
                "text": "func main(: i32 {\n"
            }
        }
    }));

    run_lsp_stream(Cursor::new(input), &mut output).unwrap();

    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("textDocument/publishDiagnostics"));
    assert!(text.contains("E0200"));
}

#[test]
fn ignores_stale_document_changes() {
    let mut output = Vec::new();
    let mut input = frame(&json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": "file:///tmp/nocter-lsp-stale.nct",
                "languageId": "nocter",
                "version": 2,
                "text": "func main(): i32 {\n    return 0\n}\n"
            }
        }
    }));
    input.extend(frame(&json!({
        "jsonrpc": "2.0",
        "method": "textDocument/didChange",
        "params": {
            "textDocument": {
                "uri": "file:///tmp/nocter-lsp-stale.nct",
                "version": 1
            },
            "contentChanges": [{
                "text": "func main(: i32 {\n"
            }]
        }
    })));

    run_lsp_stream(Cursor::new(input), &mut output).unwrap();

    let text = String::from_utf8(output).unwrap();
    assert_eq!(text.matches("textDocument/publishDiagnostics").count(), 1);
    assert!(!text.contains("E0200"));
}

#[test]
fn publishes_diagnostics_for_open_imported_document_text() {
    let project = TempProject::new("lsp-open-import");
    let app = project.write_source(
        "app.nct",
        "use ./config.answer\n\nfunc main(): i32 {\n    return answer()\n}\n",
    );
    let config = project.write_source("config.nct", "pub func answer(): i32 {\n    return 0\n}\n");
    let app_uri = file_uri(&app);
    let config_uri = file_uri(&config);
    let documents = HashMap::from([
        (
            app_uri.clone(),
            open_document(
                app_uri.clone(),
                Some(1),
                "use ./config.answer\n\nfunc main(): i32 {\n    return answer()\n}\n".to_string(),
            ),
        ),
        (
            config_uri.clone(),
            open_document(
                config_uri.clone(),
                Some(1),
                "pub func answer(: i32 {\n".to_string(),
            ),
        ),
    ]);

    let diagnostics = diagnostics_for_workspace(&app_uri, &documents);

    let config_diagnostics = diagnostics
        .iter()
        .find(|(uri, _)| uri == &config_uri)
        .map(|(_, diagnostics)| diagnostics)
        .expect("expected config diagnostics");
    assert!(
        config_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0200")
    );
}

#[test]
fn lsp_diagnostics_include_related_information_and_help() {
    let project = TempProject::new("lsp-diagnostic-related-info");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    let app = project.write_source(
        "app.nct",
        "use ./config.answer\n\nfunc main(): i32 {\n    return answer()\n}\n",
    );
    let config = project.write_source(
        "config.nct",
        "pub func answer(value: i32): i32 {\n    return value\n}\n",
    );
    let app_uri = file_uri(&app);
    let config_uri = file_uri(&config);
    let documents = HashMap::from([
        (
            app_uri.clone(),
            open_document(
                app_uri.clone(),
                Some(1),
                "use ./config.answer\n\nfunc main(): i32 {\n    return answer()\n}\n".to_string(),
            ),
        ),
        (
            config_uri.clone(),
            open_document(
                config_uri.clone(),
                Some(1),
                "pub func answer(value: i32): i32 {\n    return value\n}\n".to_string(),
            ),
        ),
    ]);

    let diagnostics = diagnostics_for_workspace(&app_uri, &documents);
    let document_diagnostics = diagnostics
        .iter()
        .find(|(diagnostic_uri, _)| diagnostic_uri == &app_uri)
        .map(|(_, diagnostics)| diagnostics)
        .expect("expected diagnostics for open document");
    let diagnostic = document_diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "E0320")
        .unwrap_or_else(|| {
            panic!("expected argument count diagnostic, got {document_diagnostics:#?}")
        });

    assert!(
        diagnostic
            .message
            .contains("help: pass exactly the parameters declared by the function")
    );
    assert_eq!(diagnostic.related_information.len(), 1);
    assert_eq!(
        diagnostic.related_information[0].message,
        "function `answer` is declared here"
    );
    assert_eq!(diagnostic.related_information[0].location.uri, config_uri);
    assert_eq!(
        diagnostic.related_information[0].location.range.start.line,
        0
    );

    let value = serde_json::to_value(diagnostic).unwrap();
    assert!(value.get("relatedInformation").is_some());
    assert!(value.get("related_information").is_none());
    assert_eq!(
        value["relatedInformation"][0]["location"]["uri"],
        json!(config_uri)
    );
}

#[test]
fn lsp_diagnostics_report_unresolved_identifiers() {
    let project = TempProject::new("lsp-unresolved-identifier");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    let app = project.write_source("app.nct", "func main(): i32 {\n    return missing\n}\n");
    let uri = file_uri(&app);
    let documents = HashMap::from([(
        uri.clone(),
        open_document(
            uri.clone(),
            Some(1),
            "func main(): i32 {\n    return missing\n}\n".to_string(),
        ),
    )]);

    let diagnostics = diagnostics_for_workspace(&uri, &documents);
    let document_diagnostics = diagnostics
        .iter()
        .find(|(diagnostic_uri, _)| diagnostic_uri == &uri)
        .map(|(_, diagnostics)| diagnostics)
        .expect("expected diagnostics for open document");

    let diagnostic = document_diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "E0416")
        .unwrap_or_else(|| {
            panic!("expected unresolved identifier diagnostic, got {document_diagnostics:#?}")
        });

    assert!(diagnostic.message.contains("missing"));
}

#[test]
fn lsp_diagnostics_report_unresolved_drop_targets() {
    let project = TempProject::new("lsp-unresolved-drop-target");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    let text = "func main(): i32 {\n    drop missing\n    return 0\n}\n";
    let app = project.write_source("app.nct", text);
    let uri = file_uri(&app);
    let documents = HashMap::from([(
        uri.clone(),
        open_document(uri.clone(), Some(1), text.to_string()),
    )]);

    let diagnostics = diagnostics_for_workspace(&uri, &documents);
    let document_diagnostics = diagnostics
        .iter()
        .find(|(diagnostic_uri, _)| diagnostic_uri == &uri)
        .map(|(_, diagnostics)| diagnostics)
        .expect("expected diagnostics for open document");

    let diagnostic = document_diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "E0416")
        .unwrap_or_else(|| {
            panic!("expected unresolved drop diagnostic, got {document_diagnostics:#?}")
        });

    assert!(diagnostic.message.contains("missing"));
}

#[test]
fn lsp_diagnostics_do_not_require_entry_function() {
    let uri = "file:///tmp/nocter-lsp-library.nct".to_string();
    let documents = HashMap::from([(
        uri.clone(),
        open_document(
            uri.clone(),
            Some(1),
            "pub func helper(): i32 {\n    return 0\n}\n".to_string(),
        ),
    )]);

    let diagnostics = diagnostics_for_workspace(&uri, &documents);
    let document_diagnostics = diagnostics
        .iter()
        .find(|(diagnostic_uri, _)| diagnostic_uri == &uri)
        .map(|(_, diagnostics)| diagnostics)
        .expect("expected diagnostics for open document");

    assert!(
        document_diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != "E0300"),
        "LSP diagnostics should not require `main` in every opened file"
    );
}

#[test]
fn clears_diagnostics_for_uris_missing_from_next_publish() {
    let mut server = LspServer::new();
    let mut output = Vec::new();
    let uri = "file:///tmp/nocter-cleared.nct".to_string();
    server.published_diagnostic_uris.insert(uri.clone());

    server
        .publish_workspace_diagnostics("file:///tmp/missing-root.nct", &mut output)
        .unwrap();

    let text = String::from_utf8(output).unwrap();
    assert!(text.contains(&uri));
    assert!(text.contains("\"diagnostics\":[]"));
    assert!(server.published_diagnostic_uris.is_empty());
}

#[test]
fn returns_only_lexically_visible_local_completion_items() {
    let project = TempProject::new("lsp-local-completion-scope");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    let text = r#"func main(input: i32): i32 {
    let outer = input
    if true {
        let inner = 2
        return inner
    }
    let later = 3
    return later
}
"#;
    let app = project.write_source("app.nct", text);
    let uri = file_uri(&app);
    let server = LspServer {
        documents: HashMap::from([(
            uri.clone(),
            open_document(uri.clone(), Some(1), text.to_string()),
        )]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };
    let position = byte_offset_to_lsp_position(
        text,
        text.find("return inner").expect("expected inner return"),
    );

    let response = server.completion_response(
        json!(4),
        Some(&json!({
            "textDocument": { "uri": uri },
            "position": position
        })),
    );
    let items = response["result"]["items"]
        .as_array()
        .expect("expected completion items");

    for expected in ["input", "outer", "inner"] {
        let item = completion_item_with_label(items, expected)
            .unwrap_or_else(|| panic!("expected local `{expected}`"));
        assert_eq!(item["kind"], json!(6));
        assert!(
            item["detail"]
                .as_str()
                .is_some_and(|detail| detail.contains("i32"))
        );
    }
    assert!(completion_item_with_label(items, "later").is_none());
}

#[test]
fn completion_items_include_signature_documentation_and_insert_text() {
    let project = TempProject::new("lsp-documented-completion");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    let text = r#"/// Computes an answer.
func answer(value: i32): i32 {
    return value
}

func main(): i32 {
    return answer(1)
}
"#;
    let app = project.write_source("app.nct", text);
    let uri = file_uri(&app);
    let server = LspServer {
        documents: HashMap::from([(
            uri.clone(),
            open_document(uri.clone(), Some(1), text.to_string()),
        )]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };
    let position = byte_offset_to_lsp_position(text, text.rfind("answer(1)").unwrap());

    let response = server.completion_response(
        json!(5),
        Some(&json!({
            "textDocument": { "uri": uri },
            "position": position
        })),
    );
    let items = response["result"]["items"].as_array().unwrap();
    let answer = completion_item_with_label(items, "answer").expect("expected answer completion");

    assert_eq!(answer["detail"], json!("func answer(value: i32): i32"));
    assert_eq!(answer["insertText"], json!("answer()"));
    assert_eq!(
        answer["documentation"]["value"],
        json!("Computes an answer.")
    );
}

#[test]
fn vec_string_completion_specializes_methods_and_includes_std_documentation() {
    let project = TempProject::new("lsp-vec-string-completion");
    let home = project.write_nocter_home();
    std::fs::write(
        home.join("std/vec.nct"),
        r#"pub struct Vec<T> {
    len: usize
}

impl<T> Vec<T> {
    /// Transfers `value` into the end of the initialized prefix.
    pub method &+self.push(value: T): void! {
        return
    }

    pub method &self.len(): usize {
        return self.len
    }
}
"#,
    )
    .unwrap();
    std::fs::write(
        home.join("std/string.nct"),
        "pub struct String {\n    len: usize\n}\n",
    )
    .unwrap();
    let _home = NocterHomeEnv::set(&home);
    let text = r#"use std/string.String
use std/vec.Vec

func edit(values: &+Vec<String>): void {
    values.clear()
    return
}
"#;
    let app = project.write_source("app.nct", text);
    let uri = file_uri(&app);
    let server = LspServer {
        documents: HashMap::from([(
            uri.clone(),
            open_document(uri.clone(), Some(1), text.to_string()),
        )]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };
    let position =
        byte_offset_to_lsp_position(text, text.find("values.clear").unwrap() + "values.".len());

    let response = server.completion_response(
        json!(6),
        Some(&json!({
            "textDocument": { "uri": uri },
            "position": position
        })),
    );
    let items = response["result"]["items"].as_array().unwrap();
    let push = completion_item_with_label(items, "push")
        .unwrap_or_else(|| panic!("expected Vec.push completion, got {items:#?}"));

    assert_eq!(
        push["detail"],
        json!("method &+Vec<String>.push(value: String): void!")
    );
    assert_eq!(push["insertText"], json!("push()"));
    assert_eq!(
        push["documentation"]["value"],
        json!("Transfers `value` into the end of the initialized prefix.")
    );
}

#[test]
fn signature_help_recovers_incomplete_imported_call() {
    let project = TempProject::new("lsp-incomplete-imported-call");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    project.write_source(
        "math.nct",
        "pub func add(left: i32, right: i32): i32 {\n    return left + right\n}\n",
    );
    let text = "use ./math.add\n\nfunc main(): i32 {\n    return add(20, \n}\n";
    let app = project.write_source("app.nct", text);
    let uri = file_uri(&app);
    let server = LspServer {
        documents: HashMap::from([(
            uri.clone(),
            open_document(uri.clone(), Some(2), text.to_string()),
        )]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };
    let offset = text.find("20, ").unwrap() + "20, ".len();
    let position = byte_offset_to_lsp_position(text, offset);

    let response = server.signature_help_response(
        json!(7),
        Some(&json!({
            "textDocument": { "uri": uri },
            "position": position
        })),
    );

    assert_eq!(
        response["result"]["signatures"][0]["label"],
        json!("func add(left: i32, right: i32): i32")
    );
    assert_eq!(response["result"]["activeParameter"], json!(1));
}

#[test]
fn call_argument_completion_ranks_assignable_locals_first() {
    let project = TempProject::new("lsp-expected-argument-completion");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    let text = r#"func choose(value: bool): bool {
    return value
}

func main(good: bool, bad: i32): bool {
    return choose(bad)
}
"#;
    let app = project.write_source("app.nct", text);
    let uri = file_uri(&app);
    let server = LspServer {
        documents: HashMap::from([(
            uri.clone(),
            open_document(uri.clone(), Some(1), text.to_string()),
        )]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };
    let offset = text.rfind("bad)").expect("expected call argument");
    let position = byte_offset_to_lsp_position(text, offset);

    let response = server.completion_response(
        json!(8),
        Some(&json!({
            "textDocument": { "uri": uri },
            "position": position
        })),
    );
    let items = response["result"]["items"].as_array().unwrap();
    let good = completion_item_with_label(items, "good").expect("expected compatible local");
    let bad = completion_item_with_label(items, "bad").expect("expected other local");

    assert_eq!(good["sortText"], json!("000-good"));
    assert_eq!(bad["sortText"], json!("001-bad"));
}

#[test]
fn import_symbol_completion_recovers_and_filters_visibility() {
    let project = TempProject::new("lsp-import-symbol-completion");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    project.write_source(
        "math.nct",
        "pub func add(value: i32): i32 {\n    return value\n}\n\nfunc hidden(): i32 {\n    return 0\n}\n",
    );
    let text = "use ./math.\n\nfunc main(): i32 {\n    return 0\n}\n";
    let app = project.write_source("app.nct", text);
    let uri = file_uri(&app);
    let server = LspServer {
        documents: HashMap::from([(
            uri.clone(),
            open_document(uri.clone(), Some(2), text.to_string()),
        )]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };
    let offset = text.find("math.").unwrap() + "math.".len();
    let position = byte_offset_to_lsp_position(text, offset);

    let response = server.completion_response(
        json!(9),
        Some(&json!({
            "textDocument": { "uri": uri },
            "position": position
        })),
    );
    let items = response["result"]["items"].as_array().unwrap();
    let add = completion_item_with_label(items, "add")
        .unwrap_or_else(|| panic!("expected exported symbol, got {items:#?}"));

    assert_eq!(add["detail"], json!("func add(value: i32): i32"));
    assert!(completion_item_with_label(items, "hidden").is_none());
}

#[test]
fn import_path_completion_discovers_reachable_module_segments() {
    let project = TempProject::new("lsp-import-path-completion");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    project.write_source(
        "lib/math.nct",
        "pub func answer(): i32 {\n    return 42\n}\n",
    );
    project.write_source("lib/value.nct", "pub struct Value {\n    raw: i32\n}\n");
    let text = "use lib/ma\n\nfunc main(): i32 {\n    return 0\n}\n";
    let app = project.write_source("app.nct", text);
    let uri = file_uri(&app);
    let server = LspServer {
        documents: HashMap::from([(
            uri.clone(),
            open_document(uri.clone(), Some(2), text.to_string()),
        )]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };
    let offset = text.find("lib/ma").unwrap() + "lib/ma".len();
    let position = byte_offset_to_lsp_position(text, offset);

    let response = server.completion_response(
        json!(10),
        Some(&json!({
            "textDocument": { "uri": uri },
            "position": position
        })),
    );
    let items = response["result"]["items"].as_array().unwrap();
    let math = completion_item_with_label(items, "math")
        .unwrap_or_else(|| panic!("expected module segment, got {items:#?}"));

    assert_eq!(math["kind"], json!(LSP_COMPLETION_ITEM_KIND_MODULE));
    assert_eq!(math["detail"], json!("module path segment"));
    assert!(completion_item_with_label(items, "value").is_none());
}

#[test]
fn member_completion_recovers_incomplete_imported_receiver() {
    let project = TempProject::new("lsp-incomplete-imported-member");
    let home = project.write_nocter_home();
    std::fs::write(
        home.join("std/box.nct"),
        r#"pub struct Box<T> {
    value: T
}

impl<T> Box<T> {
    pub method &self.inspect(): void {
        return
    }
}
"#,
    )
    .unwrap();
    let _home = NocterHomeEnv::set(&home);
    let text = "use std/box.Box\n\nfunc inspect(value: &Box<i32>): void {\n    value.\n}\n";
    let app = project.write_source("app.nct", text);
    let uri = file_uri(&app);
    let server = LspServer {
        documents: HashMap::from([(
            uri.clone(),
            open_document(uri.clone(), Some(2), text.to_string()),
        )]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        shutdown_requested: false,
    };
    let offset = text.find("value.").unwrap() + "value.".len();
    let position = byte_offset_to_lsp_position(text, offset);

    let response = server.completion_response(
        json!(8),
        Some(&json!({
            "textDocument": { "uri": uri },
            "position": position
        })),
    );
    let items = response["result"]["items"].as_array().unwrap();
    let inspect = completion_item_with_label(items, "inspect")
        .unwrap_or_else(|| panic!("expected recovered member completion: {items:#?}"));

    assert_eq!(inspect["detail"], json!("method &Box<i32>.inspect(): void"));
}

#[test]
fn json_rpc_recovers_consecutive_incomplete_call_member_and_import_edits() {
    let project = TempProject::new("lsp-consecutive-recovery");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    let library_text = r#"pub func add(left: i32, right: i32): i32 {
    return left + right
}

pub struct Box {
    value: i32
}

impl Box {
    pub method &self.inspect(): i32 {
        return self.value
    }
}
"#;
    let call_text = "use ./library.add\n\nfunc main(): i32 {\n    return add(20, \n}\n";
    let member_text =
        "use ./library.Box\n\nfunc inspect(value: &Box): i32 {\n    return value.\n}\n";
    let import_text = "use ./library.\n\nfunc main(): i32 {\n    return 0\n}\n";
    let library = project.write_source("library.nct", library_text);
    let app = project.write_source("app.nct", call_text);
    let library_uri = file_uri(&library);
    let app_uri = file_uri(&app);
    let mut input = frame(&json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "rootUri": file_uri(&project.root) }
    }));
    input.extend(frame(&json!({
        "jsonrpc": "2.0", "method": "textDocument/didOpen",
        "params": { "textDocument": {
            "uri": library_uri, "languageId": "nocter", "version": 1, "text": library_text
        }}
    })));
    input.extend(frame(&json!({
        "jsonrpc": "2.0", "method": "textDocument/didOpen",
        "params": { "textDocument": {
            "uri": app_uri, "languageId": "nocter", "version": 1, "text": call_text
        }}
    })));
    input.extend(frame(&json!({
        "jsonrpc": "2.0", "id": 10, "method": "textDocument/signatureHelp",
        "params": {
            "textDocument": { "uri": app_uri },
            "position": byte_offset_to_lsp_position(
                call_text,
                call_text.find("20, ").unwrap() + "20, ".len()
            )
        }
    })));
    input.extend(frame(&json!({
        "jsonrpc": "2.0", "method": "textDocument/didChange",
        "params": {
            "textDocument": { "uri": app_uri, "version": 2 },
            "contentChanges": [{ "text": member_text }]
        }
    })));
    input.extend(frame(&json!({
        "jsonrpc": "2.0", "id": 11, "method": "textDocument/completion",
        "params": {
            "textDocument": { "uri": app_uri },
            "position": byte_offset_to_lsp_position(
                member_text,
                member_text.find("value.").unwrap() + "value.".len()
            )
        }
    })));
    input.extend(frame(&json!({
        "jsonrpc": "2.0", "method": "textDocument/didChange",
        "params": {
            "textDocument": { "uri": app_uri, "version": 3 },
            "contentChanges": [{ "text": import_text }]
        }
    })));
    input.extend(frame(&json!({
        "jsonrpc": "2.0", "id": 12, "method": "textDocument/completion",
        "params": {
            "textDocument": { "uri": app_uri },
            "position": byte_offset_to_lsp_position(
                import_text,
                import_text.find("library.").unwrap() + "library.".len()
            )
        }
    })));

    let mut output = Vec::new();
    run_lsp_stream(Cursor::new(input), &mut output).unwrap();
    let messages = framed_messages(&output);
    let signature = response_with_id(&messages, 10);
    let member = response_with_id(&messages, 11);
    let import = response_with_id(&messages, 12);

    assert_eq!(
        signature["result"]["signatures"][0]["label"],
        json!("func add(left: i32, right: i32): i32")
    );
    assert!(
        completion_item_with_label(member["result"]["items"].as_array().unwrap(), "inspect")
            .is_some()
    );
    let import_items = import["result"]["items"].as_array().unwrap();
    assert!(completion_item_with_label(import_items, "add").is_some());
    assert!(completion_item_with_label(import_items, "Box").is_some());
}

#[test]
fn json_rpc_uses_one_open_overlay_for_diagnostics_definition_and_references() {
    let project = TempProject::new("lsp-overlay-consistency");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    let disk_library = "pub func stale(): i32 {\n    return 0\n}\n";
    let overlay_library = "pub func answer(): i32 {\n    return 42\n}\n";
    let app_text = "use ./library.answer\n\nfunc main(): i32 {\n    return answer()\n}\n";
    let library = project.write_source("library.nct", disk_library);
    let app = project.write_source("app.nct", app_text);
    let library_uri = file_uri(&library);
    let app_uri = file_uri(&app);
    let call_offset = app_text.rfind("answer()").unwrap();
    let mut input = frame(&json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "rootUri": file_uri(&project.root) }
    }));
    input.extend(frame(&json!({
        "jsonrpc": "2.0", "method": "textDocument/didOpen",
        "params": { "textDocument": {
            "uri": library_uri, "languageId": "nocter", "version": 1, "text": overlay_library
        }}
    })));
    input.extend(frame(&json!({
        "jsonrpc": "2.0", "method": "textDocument/didOpen",
        "params": { "textDocument": {
            "uri": app_uri, "languageId": "nocter", "version": 1, "text": app_text
        }}
    })));
    for (id, method) in [
        (20, "textDocument/definition"),
        (21, "textDocument/references"),
    ] {
        input.extend(frame(&json!({
            "jsonrpc": "2.0", "id": id, "method": method,
            "params": {
                "textDocument": { "uri": app_uri },
                "position": byte_offset_to_lsp_position(app_text, call_offset),
                "context": { "includeDeclaration": true }
            }
        })));
    }

    let mut output = Vec::new();
    run_lsp_stream(Cursor::new(input), &mut output).unwrap();
    let messages = framed_messages(&output);
    let definition = response_with_id(&messages, 20);
    let references = response_with_id(&messages, 21)["result"]
        .as_array()
        .unwrap();

    assert_eq!(definition["result"]["uri"], json!(library_uri));
    assert!(
        references
            .iter()
            .any(|location| location["uri"] == json!(library_uri))
    );
    assert!(
        references
            .iter()
            .any(|location| location["uri"] == json!(app_uri))
    );
    let app_diagnostics = messages
        .iter()
        .rev()
        .find(|message| {
            message["method"] == json!("textDocument/publishDiagnostics")
                && message["params"]["uri"] == json!(app_uri)
        })
        .expect("expected app diagnostics publication");
    assert_eq!(app_diagnostics["params"]["diagnostics"], json!([]));
}

fn frame(message: &Value) -> Vec<u8> {
    let body = serde_json::to_vec(message).unwrap();
    let mut framed = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    framed.extend(body);
    framed
}

fn framed_messages(bytes: &[u8]) -> Vec<Value> {
    let mut reader = Cursor::new(bytes);
    let mut messages = Vec::new();
    while let Some(message) = read_message(&mut reader).unwrap() {
        messages.push(message);
    }
    messages
}

fn response_with_id(messages: &[Value], id: i64) -> &Value {
    messages
        .iter()
        .find(|message| message.get("id").and_then(Value::as_i64) == Some(id))
        .unwrap_or_else(|| panic!("missing response {id}: {messages:#?}"))
}

fn file_uri(path: &Path) -> String {
    format!("file://{}", path.to_string_lossy())
}

fn completion_item_with_label<'a>(items: &'a [Value], label: &str) -> Option<&'a Value> {
    items
        .iter()
        .find(|item| item.get("label").and_then(Value::as_str) == Some(label))
}

fn classified_identifier_with_lexeme<'a>(
    text: &str,
    identifiers: &'a [ClassifiedIdentifier],
    lexeme: &str,
) -> Vec<&'a ClassifiedIdentifier> {
    identifiers
        .iter()
        .filter(|identifier| text.get(identifier.start_byte..identifier.end_byte) == Some(lexeme))
        .collect()
}

fn classified_identifier_starting_at(
    identifiers: &[ClassifiedIdentifier],
    start_byte: usize,
) -> Option<&ClassifiedIdentifier> {
    identifiers
        .iter()
        .find(|identifier| identifier.start_byte == start_byte)
}

fn field_name_offset_for_access(text: &str, access: &str) -> usize {
    text.find(access).unwrap() + access.find('.').unwrap() + 1
}

static NOCTER_HOME_ENV_LOCK: Mutex<()> = Mutex::new(());

struct NocterHomeEnv {
    previous: Option<std::ffi::OsString>,
    _guard: MutexGuard<'static, ()>,
}

impl NocterHomeEnv {
    fn set(home: &Path) -> Self {
        let guard = NOCTER_HOME_ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("NOCTER_HOME");
        // Exercise the same process-level home resolution path as the CLI.
        unsafe {
            std::env::set_var("NOCTER_HOME", home);
        }
        Self {
            previous,
            _guard: guard,
        }
    }
}

impl Drop for NocterHomeEnv {
    fn drop(&mut self) {
        // Restore the process environment before releasing the test lock.
        unsafe {
            match &self.previous {
                Some(value) => std::env::set_var("NOCTER_HOME", value),
                None => std::env::remove_var("NOCTER_HOME"),
            }
        }
    }
}

struct TempProject {
    root: PathBuf,
}

impl TempProject {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "nocter-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn write_source(&self, name: &str, text: &str) -> PathBuf {
        let path = self.root.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, text).unwrap();
        path
    }

    fn write_nocter_home(&self) -> PathBuf {
        let home = self.root.join(".nocter");
        std::fs::create_dir_all(home.join("std")).unwrap();
        std::fs::write(home.join("std/prelude.nct"), "").unwrap();
        home
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
