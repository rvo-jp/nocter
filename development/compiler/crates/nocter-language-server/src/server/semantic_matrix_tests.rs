use super::tests::{TemporaryDirectory, semantic_server};

struct MatrixCase {
    name: &'static str,
    source: &'static str,
    status: nocter_analysis::AnalysisStatus,
}

#[test]
fn semantic_features_share_one_expected_state_matrix() {
    let cases = [
        MatrixCase {
            name: "complete",
            source: concat!(
                "func subject(input: i32): i32 {\n",
                "    let value = input\n",
                "    value\n",
                "}\n",
            ),
            status: nocter_analysis::AnalysisStatus::Complete,
        },
        MatrixCase {
            name: "declaration-rejected",
            source: concat!(
                "primitive func unavailable(): i32\n",
                "func subject(input: i32): i32 {\n",
                "    let value = input\n",
                "    value\n",
                "}\n",
            ),
            status: nocter_analysis::AnalysisStatus::CompilationFailed,
        },
        MatrixCase {
            name: "name-rejected",
            source: concat!(
                "func subject(input: i32): i32 {\n",
                "    let value = input\n",
                "    unknown(value)\n",
                "}\n",
            ),
            status: nocter_analysis::AnalysisStatus::CompilationFailed,
        },
        MatrixCase {
            name: "body-rejected",
            source: concat!(
                "func subject(input: i32?): i32 {\n",
                "    let value = 1\n",
                "    input?\n",
                "}\n",
            ),
            status: nocter_analysis::AnalysisStatus::CompilationFailed,
        },
        MatrixCase {
            name: "syntax-incomplete",
            source: concat!(
                "func subject(input: i32): i32 {\n",
                "    let value = input\n",
                "    value.\n",
                "}\n",
            ),
            status: nocter_analysis::AnalysisStatus::SyntaxFailed,
        },
    ];

    for case in cases {
        exercise_case(&case);
    }
}

fn exercise_case(case: &MatrixCase) {
    let temporary = TemporaryDirectory::new();
    let path = temporary.path().join("main.nct");
    std::fs::write(&path, case.source).unwrap();
    let uri = format!("file://{}", path.display());
    let mut server = semantic_server(temporary.path());
    server.receive(&format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
        temporary.path().display()
    ));
    server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
    let mut source_json = String::new();
    nocter_json::write_string(&mut source_json, case.source);
    let opened = server.receive(&format!(
        "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\",\"languageId\":\"nocter\",\"version\":1,\"text\":{source_json}}}}}}}"
    ));
    assert!(
        opened.issue().is_none(),
        "{}: {:?}",
        case.name,
        opened.issue()
    );
    assert_eq!(
        opened.analysis().unwrap().snapshot().unwrap().status(),
        case.status,
        "{}",
        case.name
    );

    let (line, character) = line_character(case.source, "subject");
    let (end_line, end_character) = end_position(case.source);
    let requests = [
        format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"textDocument/hover\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":{line},\"character\":{character}}}}}}}"
        ),
        format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"textDocument/completion\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":{line},\"character\":{character}}}}}}}"
        ),
        format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"textDocument/definition\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":{line},\"character\":{character}}}}}}}"
        ),
        format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":5,\"method\":\"textDocument/implementation\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":{line},\"character\":{character}}}}}}}"
        ),
        format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":6,\"method\":\"textDocument/references\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":{line},\"character\":{character}}},\"context\":{{\"includeDeclaration\":true}}}}}}"
        ),
        format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"textDocument/semanticTokens/full\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}}}}}}"
        ),
        format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":8,\"method\":\"textDocument/inlayHint\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"range\":{{\"start\":{{\"line\":0,\"character\":0}},\"end\":{{\"line\":{end_line},\"character\":{end_character}}}}}}}}}"
        ),
        format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":9,\"method\":\"textDocument/signatureHelp\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":{line},\"character\":{character}}}}}}}"
        ),
        format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":10,\"method\":\"textDocument/codeAction\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"range\":{{\"start\":{{\"line\":{line},\"character\":{character}}},\"end\":{{\"line\":{line},\"character\":{character}}}}},\"context\":{{\"diagnostics\":[]}}}}}}"
        ),
        format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":11,\"method\":\"textDocument/rename\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":{line},\"character\":{character}}},\"newName\":\"renamed\"}}}}"
        ),
    ];
    for request in requests {
        let result = server.receive(&request);
        assert!(
            result.issue().is_none(),
            "{}: request {request} produced {:?}",
            case.name,
            result.issue()
        );
    }
}

fn line_character(source: &str, needle: &str) -> (usize, usize) {
    let offset = source.find(needle).unwrap();
    let prefix = &source[..offset];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count();
    let character = prefix.rsplit('\n').next().unwrap().chars().count();
    (line, character)
}

fn end_position(source: &str) -> (usize, usize) {
    let line = source.bytes().filter(|byte| *byte == b'\n').count();
    let character = source.rsplit('\n').next().unwrap().chars().count();
    (line, character)
}
