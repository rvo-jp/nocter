use super::*;

fn write_iteration_std(home: &Path) {
    crate::test_files::write(
        home.join("std/iter/index.nct"),
        r#"pub struct ViewIter<T> {
    view: &[T]
    next_index: usize
}

construct ViewIter<T> {
    pub default func from_view(view: &[T]): Self {
        return ViewIter<T> { view: view, next_index: 0 }
    }
}

impl<T> ViewIter<T> {
    pub method &self.remaining(): usize {
        return self.view.len - self.next_index
    }

    /// Returns the next element borrowed from the source view.
    pub method &+self.next(): &T? {
        if self.next_index >= self.view.len {
            return none
        }
        let index = self.next_index
        self.next_index = index + 1
        return &self.view[index]
    }
}
"#,
    )
    .unwrap();
    crate::test_files::write(
        home.join("std/vec_into_iter/index.nct"),
        r#"pub struct VecIntoIter<T> {
    remaining_count: usize
}

impl<T> VecIntoIter<T> {
    /// Moves the next element out of the owned remaining range.
    pub method &+self.next(): T? {
        return none
    }
}
"#,
    )
    .unwrap();
    crate::test_files::write(
        home.join("std/vec/index.nct"),
        r#"use std/iter.ViewIter

pub struct Vec<T> {
    view: &[T]
}

impl<T> Vec<T> {
    /// Creates an allocation-free readonly iterator.
    pub method &self.iter(): ViewIter<T> {
        return ViewIter.from_view(self.view)
    }
}
"#,
    )
    .unwrap();
}

fn iteration_server(name: &str, text: &str) -> (TempProject, NocterHomeEnv, String, LspServer) {
    let project = TempProject::new(name);
    let home = project.write_nocter_home();
    write_iteration_std(&home);
    let home_guard = NocterHomeEnv::set(&home);
    let app = project.write_source("app.nct", text);
    let uri = file_uri(&app);
    let server = LspServer {
        documents: HashMap::from([(
            uri.clone(),
            open_document(uri.clone(), Some(1), text.to_string()),
        )]),
        published_diagnostic_uris: HashSet::new(),
        workspace_roots: Vec::new(),
        snapshots: SnapshotStore::default(),
        lifecycle: ServerLifecycle::Running,
        file_watching: FileWatchingRegistration::Unsupported,
    };
    (project, home_guard, uri, server)
}

#[test]
fn iterator_queries_expose_specialized_capability_and_provenance() {
    let text = r#"use std/iter.ViewIter
use std/vec.Vec
use std/vec_into_iter.VecIntoIter

func inspect(values: &Vec<i32>, initial: VecIntoIter<i32>): void {
    var iterator: ViewIter<i32> = values.iter()
    let item = iterator.next() otherwise { return }
    var owned: VecIntoIter<i32> = move initial
    let moved = owned.next() otherwise { return }
    drop moved
    return
}

func inspect_readonly(iterator: &ViewIter<i32>): void {
    iterator.remaining()
    return
}
"#;
    let (_project, _home, uri, server) = iteration_server("lsp-iterator-facts", text);
    let method_offset = text.find("iterator.next").unwrap() + "iterator.".len();
    let method_position = byte_offset_to_lsp_position(text, method_offset);

    let completion = server.completion_response(
        json!(1),
        Some(&json!({
            "textDocument": { "uri": uri },
            "position": method_position
        })),
    );
    let items = completion["result"]["items"].as_array().unwrap();
    let next = completion_item_with_label(items, "next")
        .unwrap_or_else(|| panic!("expected ViewIter.next completion: {items:#?}"));
    assert_eq!(
        next["detail"],
        json!("method &+ViewIter<i32>.next(): &i32?")
    );
    assert_eq!(next["insertText"], json!("next()"));

    let hover = server.hover_response(
        json!(2),
        Some(&json!({
            "textDocument": { "uri": uri },
            "position": method_position
        })),
    );
    let hover_text = hover["result"]["contents"]["value"]
        .as_str()
        .expect("expected iterator hover");
    assert!(
        hover_text.contains("method &+ViewIter<i32>.next(): &i32?"),
        "hover:\n{hover_text}"
    );
    assert!(
        !hover_text.contains("Result provenance:"),
        "hover:\n{hover_text}"
    );

    let call_offset = text.find("next()").unwrap() + "next(".len();
    let signature = server.signature_help_response(
        json!(3),
        Some(&json!({
            "textDocument": { "uri": uri },
            "position": byte_offset_to_lsp_position(text, call_offset)
        })),
    );
    assert_eq!(
        signature["result"]["signatures"][0]["label"],
        json!("method &+ViewIter<i32>.next(): &i32?")
    );

    let owned_offset = text.find("owned.next").unwrap() + "owned.".len();
    let owned_completion = server.completion_response(
        json!(4),
        Some(&json!({
            "textDocument": { "uri": uri },
            "position": byte_offset_to_lsp_position(text, owned_offset)
        })),
    );
    let owned_items = owned_completion["result"]["items"].as_array().unwrap();
    let owned_next = completion_item_with_label(owned_items, "next")
        .unwrap_or_else(|| panic!("expected VecIntoIter.next completion: {owned_items:#?}"));
    assert_eq!(
        owned_next["detail"],
        json!("method &+VecIntoIter<i32>.next(): i32?")
    );

    let readonly_offset = text.find("iterator.remaining").unwrap() + "iterator.".len();
    let readonly_completion = server.completion_response(
        json!(5),
        Some(&json!({
            "textDocument": { "uri": uri },
            "position": byte_offset_to_lsp_position(text, readonly_offset)
        })),
    );
    let readonly_items = readonly_completion["result"]["items"].as_array().unwrap();
    assert!(completion_item_with_label(readonly_items, "remaining").is_some());
    assert!(completion_item_with_label(readonly_items, "next").is_none());
}

#[test]
fn iterator_signature_help_recovers_an_incomplete_next_call() {
    let text = r#"use std/iter.ViewIter
use std/vec.Vec

func inspect(values: &Vec<i32>): void {
    var iterator: ViewIter<i32> = values.iter()
    iterator.next(
}
"#;
    let (_project, _home, uri, server) = iteration_server("lsp-iterator-recovery", text);
    let offset = text.find("next(").unwrap() + "next(".len();

    let response = server.signature_help_response(
        json!(4),
        Some(&json!({
            "textDocument": { "uri": uri },
            "position": byte_offset_to_lsp_position(text, offset)
        })),
    );

    assert_eq!(
        response["result"]["signatures"][0]["label"],
        json!("method &+ViewIter<i32>.next(): &i32?")
    );
    assert_eq!(response["result"]["activeParameter"], json!(0));
}

#[test]
fn json_rpc_recovers_an_incomplete_iterator_call() {
    let project = TempProject::new("lsp-iterator-json-rpc-recovery");
    let home = project.write_nocter_home();
    write_iteration_std(&home);
    let _home = NocterHomeEnv::set(&home);
    let text = r#"use std/iter.ViewIter
use std/vec.Vec

func inspect(values: &Vec<i32>): void {
    var iterator: ViewIter<i32> = values.iter()
    iterator.next(
}
"#;
    let app = project.write_source("app.nct", text);
    let uri = file_uri(&app);
    let offset = text.find("next(").unwrap() + "next(".len();
    let mut input = frame(&json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "rootUri": file_uri(&project.root) }
    }));
    input.extend(frame(&json!({
        "jsonrpc": "2.0", "method": "textDocument/didOpen",
        "params": { "textDocument": {
            "uri": uri, "languageId": "nocter", "version": 1, "text": text
        }}
    })));
    input.extend(frame(&json!({
        "jsonrpc": "2.0", "id": 6, "method": "textDocument/signatureHelp",
        "params": {
            "textDocument": { "uri": uri },
            "position": byte_offset_to_lsp_position(text, offset)
        }
    })));

    let mut output = Vec::new();
    run_lsp_stream(Cursor::new(input), &mut output).unwrap();
    let messages = framed_messages(&output);
    let response = response_with_id(&messages, 6);

    assert_eq!(
        response["result"]["signatures"][0]["label"],
        json!("method &+ViewIter<i32>.next(): &i32?")
    );
}
