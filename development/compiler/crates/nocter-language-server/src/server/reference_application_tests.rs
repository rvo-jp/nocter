use std::fs;
use std::path::Path;

use super::tests::semantic_server;

#[test]
fn network_loopback_uses_one_checked_contract_across_editor_features() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../examples/network-loopback");
    let source = root.join("exchange.nct");
    let (mut server, text) = open_package_source(&root, &source);

    let (bind_line, bind_source) = source_line(&text, "TcpListener.bind_blocking");
    let bind_character = bind_source.find("bind").unwrap();
    let hover = server.receive(&position_request(
        2,
        "textDocument/hover",
        &source,
        bind_line,
        bind_character,
    ));
    let response = hover.response().unwrap();
    assert!(
        response.contains(
            "pub noalloc blocking func TcpListener.bind_blocking(address: SocketAddress): TcpListener!"
        ),
        "{response}"
    );
    assert!(hover.issue().is_none(), "{:?}", hover.issue());

    let definition = server.receive(&position_request(
        3,
        "textDocument/definition",
        &source,
        bind_line,
        bind_character,
    ));
    let response = definition.response().unwrap();
    assert!(response.contains("/std/net/index.nct"), "{response}");
    assert!(definition.issue().is_none(), "{:?}", definition.issue());

    let implementation = server.receive(&position_request(
        4,
        "textDocument/implementation",
        &source,
        bind_line,
        bind_character,
    ));
    let response = implementation.response().unwrap();
    assert!(response.contains("/std/net/tcp.nct"), "{response}");
    assert!(
        implementation.issue().is_none(),
        "{:?}",
        implementation.issue()
    );

    let (timeout_line, timeout_source) = source_line(&text, "set_read_timeout");
    let timeout_argument = timeout_source.find("short_timeout").unwrap() + 2;
    let signature = server.receive(&position_request(
        5,
        "textDocument/signatureHelp",
        &source,
        timeout_line,
        timeout_argument,
    ));
    let response = signature.response().unwrap();
    assert!(
        response.contains("method &+UdpSocket.set_read_timeout(timeout: Duration?): void"),
        "{response}"
    );
    assert!(response.contains("\"activeParameter\":0"), "{response}");
    assert!(signature.issue().is_none(), "{:?}", signature.issue());

    let (completion_line, completion_source) =
        source_line(&text, "let received = receiver.receive");
    let completion_character = completion_source.find("receiver.").unwrap() + "receiver.".len();
    let completion = server.receive(&position_request(
        6,
        "textDocument/completion",
        &source,
        completion_line,
        completion_character,
    ));
    let response = completion.response().unwrap();
    for method in ["local_address", "receive", "set_read_timeout"] {
        assert!(
            response.contains(&format!("\"label\":\"{method}\",\"kind\":2")),
            "{response}"
        );
    }
    assert!(completion.issue().is_none(), "{:?}", completion.issue());

    assert_network_loopback_source_features(&mut server, &source, &text);
}

fn assert_network_loopback_source_features(
    server: &mut super::LanguageServer,
    source: &Path,
    text: &str,
) {
    let tokens = server.receive(&format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"textDocument/semanticTokens/full\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\"}}}}}}",
        source.display()
    ));
    let response = tokens.response().unwrap();
    assert!(response.contains("\"data\":["), "{response}");
    assert!(!response.contains("\"data\":[]"), "{response}");
    assert!(tokens.issue().is_none(), "{:?}", tokens.issue());

    let (receiver_line, receiver_source) = source_line(text, "var receiver");
    let receiver_character = receiver_source.find("receiver").unwrap();
    let references = server.receive(&format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":8,\"method\":\"textDocument/references\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\"}},\"position\":{{\"line\":{receiver_line},\"character\":{receiver_character}}},\"context\":{{\"includeDeclaration\":true}}}}}}",
        source.display()
    ));
    let response = references.response().unwrap();
    assert!(response.matches("exchange.nct").count() >= 4, "{response}");
    assert!(references.issue().is_none(), "{:?}", references.issue());

    let rename = server.receive(&format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":9,\"method\":\"textDocument/rename\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\"}},\"position\":{{\"line\":{receiver_line},\"character\":{receiver_character}}},\"newName\":\"destination\"}}}}",
        source.display()
    ));
    let response = rename.response().unwrap();
    assert!(response.matches("destination").count() >= 4, "{response}");
    assert!(rename.issue().is_none(), "{:?}", rename.issue());

    let end_line = text.lines().count();
    let hints = server.receive(&format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":10,\"method\":\"textDocument/inlayHint\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\"}},\"range\":{{\"start\":{{\"line\":0,\"character\":0}},\"end\":{{\"line\":{end_line},\"character\":0}}}}}}}}",
        source.display()
    ));
    let response = hints.response().unwrap();
    assert!(
        response.contains("\"label\":\": SocketAddress\""),
        "{response}"
    );
    assert!(hints.issue().is_none(), "{:?}", hints.issue());
}

#[test]
fn async_udp_uses_one_checked_contract_across_editor_features() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../examples/async-udp");
    let source = root.join("exchange.nct");
    let (mut server, text) = open_package_source(&root, &source);

    assert_async_udp_navigation_and_calls(&mut server, &source, &text);
    assert_async_udp_source_features(&mut server, &source, &text);
}

fn assert_async_udp_navigation_and_calls(
    server: &mut super::LanguageServer,
    source: &Path,
    text: &str,
) {
    let (send_line, send_source) = source_line(text, "sender.send_to_with_timeout");
    let send_character = send_source.find("send_to_with_timeout").unwrap();
    let hover = server.receive(&position_request(
        2,
        "textDocument/hover",
        source,
        send_line,
        send_character,
    ));
    let response = hover.response().unwrap();
    assert!(
        response.contains(
            "pub async method &+UdpSocket.send_to_with_timeout(bytes: &[u8], target: SocketAddress, timeout: Duration): void!"
        ),
        "{response}"
    );
    assert!(hover.issue().is_none(), "{:?}", hover.issue());

    let definition = server.receive(&position_request(
        3,
        "textDocument/definition",
        source,
        send_line,
        send_character,
    ));
    let response = definition.response().unwrap();
    assert!(response.contains("/std/net/index.nct"), "{response}");
    assert!(definition.issue().is_none(), "{:?}", definition.issue());

    let implementation = server.receive(&position_request(
        4,
        "textDocument/implementation",
        source,
        send_line,
        send_character,
    ));
    let response = implementation.response().unwrap();
    assert!(response.contains("/std/net/udp.nct"), "{response}");
    assert!(
        implementation.issue().is_none(),
        "{:?}",
        implementation.issue()
    );

    let send_target = send_source.find("receiver_address").unwrap() + 2;
    let signature = server.receive(&position_request(
        5,
        "textDocument/signatureHelp",
        source,
        send_line,
        send_target,
    ));
    let response = signature.response().unwrap();
    assert!(
        response.contains(
            "method &+UdpSocket.send_to_with_timeout(bytes: &[u8], target: SocketAddress, timeout: Duration): void!"
        ),
        "{response}"
    );
    assert!(response.contains("\"activeParameter\":1"), "{response}");
    assert!(signature.issue().is_none(), "{:?}", signature.issue());
}

fn assert_async_udp_source_features(server: &mut super::LanguageServer, source: &Path, text: &str) {
    let (completion_line, completion_source) =
        source_line(text, "let received = await receiver.receive");
    let completion_character = completion_source.find("receiver.").unwrap() + "receiver.".len();
    let completion = server.receive(&position_request(
        6,
        "textDocument/completion",
        source,
        completion_line,
        completion_character,
    ));
    let response = completion.response().unwrap();
    for method in [
        "receive",
        "receive_blocking",
        "receive_with_timeout",
        "send_to",
    ] {
        assert!(
            response.contains(&format!("\"label\":\"{method}\",\"kind\":2")),
            "{response}"
        );
    }
    assert!(completion.issue().is_none(), "{:?}", completion.issue());

    let tokens = server.receive(&format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"textDocument/semanticTokens/full\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\"}}}}}}",
        source.display()
    ));
    let response = tokens.response().unwrap();
    assert!(response.contains("\"data\":["), "{response}");
    assert!(!response.contains("\"data\":[]"), "{response}");
    assert!(tokens.issue().is_none(), "{:?}", tokens.issue());

    let end_line = text.lines().count();
    let hints = server.receive(&format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":8,\"method\":\"textDocument/inlayHint\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\"}},\"range\":{{\"start\":{{\"line\":0,\"character\":0}},\"end\":{{\"line\":{end_line},\"character\":0}}}}}}}}",
        source.display()
    ));
    let response = hints.response().unwrap();
    for inferred in [": UdpSocket", ": DatagramRead"] {
        assert!(
            response.contains(&format!("\"label\":\"{inferred}\"")),
            "{response}"
        );
    }
    assert!(hints.issue().is_none(), "{:?}", hints.issue());
}

#[test]
fn http_service_uses_public_server_typestate_across_editor_features() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../examples/http-service");
    let source = root.join("service.nct");
    let (mut server, text) = open_package_source(&root, &source);

    assert_http_connection_editor_features(&mut server, &source, &text);
    assert_http_router_editor_features(&mut server, &source, &text);
    assert_http_lending_query_editor_features(&mut server, &source, &text);
    assert_http_durable_store_editor_features(&mut server, &source, &text);
    assert_http_request_and_shutdown_editor_features(&mut server, &source, &text);
}

fn assert_http_durable_store_editor_features(
    server: &mut super::LanguageServer,
    source: &Path,
    text: &str,
) {
    let (open_line, open_source) = source_line(text, "Store.open(path)");
    let open_character = open_source.find("open").unwrap();
    let hover = server.receive(&position_request(
        50,
        "textDocument/hover",
        source,
        open_line,
        open_character,
    ));
    let response = hover.response().unwrap();
    assert!(
        response.contains("pub async func Store.open(path: &str): Store!"),
        "{response}"
    );
    assert!(hover.issue().is_none(), "{:?}", hover.issue());

    let definition = server.receive(&position_request(
        51,
        "textDocument/definition",
        source,
        open_line,
        open_character,
    ));
    let response = definition.response().unwrap();
    assert!(response.contains("/std/store/index.nct"), "{response}");
    assert!(definition.issue().is_none(), "{:?}", definition.issue());

    let implementation = server.receive(&position_request(
        52,
        "textDocument/implementation",
        source,
        open_line,
        open_character,
    ));
    let response = implementation.response().unwrap();
    assert!(response.contains("/std/store/store.nct"), "{response}");
    assert!(
        implementation.issue().is_none(),
        "{:?}",
        implementation.issue()
    );

    let (set_line, set_source) = source_line(text, "durable.set(");
    let completion_character = set_source.find("durable.").unwrap() + "durable.".len();
    let completion = server.receive(&position_request(
        53,
        "textDocument/completion",
        source,
        set_line,
        completion_character,
    ));
    let response = completion.response().unwrap();
    for method in [
        "close",
        "commit",
        "compact",
        "get",
        "has_uncommitted_changes",
        "remove",
        "set",
    ] {
        assert!(
            response.contains(&format!("\"label\":\"{method}\",\"kind\":2")),
            "{response}"
        );
    }
    assert!(completion.issue().is_none(), "{:?}", completion.issue());
}

fn assert_http_connection_editor_features(
    server: &mut super::LanguageServer,
    source: &Path,
    text: &str,
) {
    let (read_line, read_source) = source_line(text, "owner.read_request_with_timeout");
    let read_character = read_source.find("read_request_with_timeout").unwrap();
    let hover = server.receive(&position_request(
        2,
        "textDocument/hover",
        source,
        read_line,
        read_character,
    ));
    let response = hover.response().unwrap();
    assert!(
        response.contains(concat!(
            "pub async method ServerConnection.read_request_with_timeout(",
            "timeout: Duration): IncomingRequest!"
        )),
        "{response}"
    );
    assert!(hover.issue().is_none(), "{:?}", hover.issue());

    let definition = server.receive(&position_request(
        3,
        "textDocument/definition",
        source,
        read_line,
        read_character,
    ));
    let response = definition.response().unwrap();
    assert!(response.contains("/std/http/index.nct"), "{response}");
    assert!(definition.issue().is_none(), "{:?}", definition.issue());

    let implementation = server.receive(&position_request(
        4,
        "textDocument/implementation",
        source,
        read_line,
        read_character,
    ));
    let response = implementation.response().unwrap();
    assert!(response.contains("/std/http/server.nct"), "{response}");
    assert!(
        implementation.issue().is_none(),
        "{:?}",
        implementation.issue()
    );

    let completion_character = read_source.find("owner.").unwrap() + "owner.".len();
    let completion = server.receive(&position_request(
        5,
        "textDocument/completion",
        source,
        read_line,
        completion_character,
    ));
    let response = completion.response().unwrap();
    for method in [
        "close",
        "peer_address",
        "read_request",
        "read_request_with_timeout",
    ] {
        assert!(
            response.contains(&format!("\"label\":\"{method}\",\"kind\":2")),
            "{response}"
        );
    }
    assert!(completion.issue().is_none(), "{:?}", completion.issue());
}

fn assert_http_lending_query_editor_features(
    server: &mut super::LanguageServer,
    source: &Path,
    text: &str,
) {
    let (query_line, query_source) = source_line(text, "request.target().query_pairs");
    let query_character = query_source.find("query_pairs").unwrap();
    let hover = server.receive(&position_request(
        14,
        "textDocument/hover",
        source,
        query_line,
        query_character,
    ));
    let response = hover.response().unwrap();
    assert!(
        response.contains(concat!(
            "pub noalloc method &RequestTarget.query_pairs(): ",
            "QueryIter from self"
        )),
        "{response}"
    );
    assert!(hover.issue().is_none(), "{:?}", hover.issue());

    let definition = server.receive(&position_request(
        15,
        "textDocument/definition",
        source,
        query_line,
        query_character,
    ));
    let response = definition.response().unwrap();
    assert!(response.contains("/std/http/index.nct"), "{response}");
    let standard_source =
        fs::read_to_string(nocter_test_support::standard_library_root().join("http/index.nct"))
            .unwrap();
    let (definition_line, definition_source) =
        source_line(&standard_source, "method &self.query_pairs");
    let definition_character = definition_source.find("query_pairs").unwrap();
    assert!(
        response.contains(&format!(
            "\"start\":{{\"line\":{definition_line},\"character\":{definition_character}}},\"end\":{{\"line\":{definition_line},\"character\":{}}}",
            definition_character + "query_pairs".len()
        )),
        "{response}"
    );
    assert!(definition.issue().is_none(), "{:?}", definition.issue());

    let (parameter_line, parameter_source) = source_line(text, "parameter.name()");
    let parameter_character = parameter_source.find("parameter").unwrap();
    let hover = server.receive(&position_request(
        16,
        "textDocument/hover",
        source,
        parameter_line,
        parameter_character,
    ));
    let response = hover.response().unwrap();
    assert!(
        response.contains("let parameter: QueryParameter"),
        "{response}"
    );
    assert!(!response.contains("from query"), "{response}");
    assert!(hover.issue().is_none(), "{:?}", hover.issue());
}

fn assert_http_request_and_shutdown_editor_features(
    server: &mut super::LanguageServer,
    source: &Path,
    text: &str,
) {
    let (finish_line, finish_source) = source_line(text, "request.finish_body_with_timeout");
    let finish_character = finish_source.find("finish_body_with_timeout").unwrap();
    let hover = server.receive(&position_request(
        6,
        "textDocument/hover",
        source,
        finish_line,
        finish_character,
    ));
    let response = hover.response().unwrap();
    assert!(
        response.contains(concat!(
            "pub async method IncomingRequest.finish_body_with_timeout(",
            "timeout: Duration): Responder!"
        )),
        "{response}"
    );
    assert!(hover.issue().is_none(), "{:?}", hover.issue());

    let implementation = server.receive(&position_request(
        7,
        "textDocument/implementation",
        source,
        finish_line,
        finish_character,
    ));
    let response = implementation.response().unwrap();
    assert!(response.contains("/std/http/server.nct"), "{response}");
    assert!(
        implementation.issue().is_none(),
        "{:?}",
        implementation.issue()
    );

    let (close_line, close_source) = source_line(text, "    owner.close()");
    let close_character = close_source.find("close").unwrap();
    let hover = server.receive(&position_request(
        8,
        "textDocument/hover",
        source,
        close_line,
        close_character,
    ));
    let response = hover.response().unwrap();
    assert!(
        response.contains("pub noalloc method Server.close(): void"),
        "{response}"
    );
    assert!(hover.issue().is_none(), "{:?}", hover.issue());

    let invalid = text.replace(
        "    owner.close()\n    let drained",
        concat!(
            "    owner.close()\n",
            "    let _late = await owner.accept_with_timeout(shutdown_deadline)?\n",
            "    let drained",
        ),
    );
    assert_ne!(invalid, text);
    let mut invalid_json = String::new();
    nocter_json::write_string(&mut invalid_json, &invalid);
    let changed = server.receive(&format!(
        "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didChange\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\",\"version\":2}},\"contentChanges\":[{{\"text\":{invalid_json}}}]}}}}",
        source.display()
    ));
    let snapshot = changed.analysis().unwrap().snapshot().unwrap();
    assert_eq!(
        snapshot.status(),
        nocter_analysis::AnalysisStatus::CompilationFailed
    );
    assert!(
        snapshot
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "E0378"),
        "{:?}",
        snapshot.diagnostics()
    );
    assert!(changed.issue().is_none(), "{:?}", changed.issue());
}

fn assert_http_router_editor_features(
    server: &mut super::LanguageServer,
    source: &Path,
    text: &str,
) {
    let (dispatch_line, dispatch_source) = source_line(text, "router.dispatch");
    let dispatch_character = dispatch_source.find("router.dispatch").unwrap() + "router.".len();
    let hover = server.receive(&position_request(
        9,
        "textDocument/hover",
        source,
        dispatch_line,
        dispatch_character,
    ));
    let response = hover.response().unwrap();
    assert!(
        response.contains(concat!(
            "pub async method &Router<State>.dispatch(",
            "request: IncomingRequest): RouteDispatch!"
        )),
        "{response}"
    );
    assert!(hover.issue().is_none(), "{:?}", hover.issue());

    let implementation = server.receive(&position_request(
        10,
        "textDocument/implementation",
        source,
        dispatch_line,
        dispatch_character,
    ));
    let response = implementation.response().unwrap();
    assert!(response.contains("/std/http/router.nct"), "{response}");
    assert!(
        implementation.issue().is_none(),
        "{:?}",
        implementation.issue()
    );

    let (handler_line, handler_source) = source_line(text, "let large: Handler");
    let handler_character = handler_source.find("Handler").unwrap();
    let hover = server.receive(&position_request(
        11,
        "textDocument/hover",
        source,
        handler_line,
        handler_character,
    ));
    let response = hover.response().unwrap();
    assert!(
        response.contains("type Handler<State> = any &func("),
        "{response}"
    );
    assert!(hover.issue().is_none(), "{:?}", hover.issue());

    let definition = server.receive(&position_request(
        12,
        "textDocument/definition",
        source,
        handler_line,
        handler_character,
    ));
    let response = definition.response().unwrap();
    assert!(response.contains("/std/http/index.nct"), "{response}");
    assert!(definition.issue().is_none(), "{:?}", definition.issue());

    let completion_character = dispatch_source.find("router.").unwrap() + "router.".len();
    let completion = server.receive(&position_request(
        13,
        "textDocument/completion",
        source,
        dispatch_line,
        completion_character,
    ));
    let response = completion.response().unwrap();
    for available in ["dispatch", "state"] {
        assert!(
            response.contains(&format!("\"label\":\"{available}\",\"kind\":2")),
            "{response}"
        );
    }
    for unavailable in ["add", "get", "into_state", "post"] {
        assert!(!response.contains(&format!("\"label\":\"{unavailable}\"")));
    }
    assert!(completion.issue().is_none(), "{:?}", completion.issue());
}

#[test]
fn async_http_projects_streaming_response_writer_contracts() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../examples/async-http");
    let source = root.join("exchange.nct");
    let (mut server, text) = open_package_source(&root, &source);

    let (begin_line, begin_source) = source_line(&text, "responder.begin_chunked_with_timeout");
    let begin_character = begin_source.find("begin_chunked_with_timeout").unwrap();
    let hover = server.receive(&position_request(
        20,
        "textDocument/hover",
        &source,
        begin_line,
        begin_character,
    ));
    let response = hover.response().unwrap();
    assert!(
        response.contains(concat!(
            "pub async method Responder.begin_chunked_with_timeout(",
            "head: ResponseHead, timeout: Duration): ResponseWriter!"
        )),
        "{response}"
    );
    assert!(hover.issue().is_none(), "{:?}", hover.issue());

    let (write_line, write_source) = source_line(&text, "writer.write_with_timeout");
    let write_character = write_source.find("write_with_timeout").unwrap();
    let implementation = server.receive(&position_request(
        21,
        "textDocument/implementation",
        &source,
        write_line,
        write_character,
    ));
    let response = implementation.response().unwrap();
    assert!(
        response.contains("/std/http/response_writer.nct"),
        "{response}"
    );
    assert!(
        implementation.issue().is_none(),
        "{:?}",
        implementation.issue()
    );

    let completion_character = write_source.find("writer.").unwrap() + "writer.".len();
    let completion = server.receive(&position_request(
        22,
        "textDocument/completion",
        &source,
        write_line,
        completion_character,
    ));
    let response = completion.response().unwrap();
    for method in [
        "flush",
        "write",
        "write_line",
        "write_text",
        "write_with_timeout",
    ] {
        assert!(
            response.contains(&format!("\"label\":\"{method}\",\"kind\":2")),
            "{response}"
        );
    }
    assert!(completion.issue().is_none(), "{:?}", completion.issue());

    let (finish_line, finish_source) = source_line(&text, "writer.finish_with_timeout");
    let finish_character = finish_source.find("finish_with_timeout").unwrap();
    let hover = server.receive(&position_request(
        23,
        "textDocument/hover",
        &source,
        finish_line,
        finish_character,
    ));
    let response = hover.response().unwrap();
    assert!(
        response.contains(concat!(
            "pub async method ResponseWriter.finish_with_timeout(",
            "timeout: Duration): ServerConnection?!"
        )),
        "{response}"
    );
    assert!(hover.issue().is_none(), "{:?}", hover.issue());
}

#[test]
fn recursive_text_search_uses_ordinary_package_editor_semantics() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../examples/text-search");
    let source = root.join("search.nct");
    let (mut server, text) = open_package_source(&root, &source);

    let (read_dir_line, read_dir_source) = source_line(&text, "var stream = fs.read_dir_blocking");
    let read_dir_character = read_dir_source.find("read_dir_blocking").unwrap();
    let hover = server.receive(&position_request(
        2,
        "textDocument/hover",
        &source,
        read_dir_line,
        read_dir_character,
    ));
    let response = hover.response().unwrap();
    assert!(
        response.contains("pub blocking func read_dir_blocking(path: &str): BlockingReadDir!"),
        "{response}"
    );
    assert!(hover.issue().is_none(), "{:?}", hover.issue());

    let definition = server.receive(&position_request(
        3,
        "textDocument/definition",
        &source,
        read_dir_line,
        read_dir_character,
    ));
    let response = definition.response().unwrap();
    assert!(response.contains("/std/fs/index.nct"), "{response}");
    assert!(definition.issue().is_none(), "{:?}", definition.issue());

    let (buffered_output_line, buffered_output_source) =
        source_line(&text, "BlockingBufWriter.new");
    let buffered_output_character = buffered_output_source.find("BlockingBufWriter").unwrap();
    let hover = server.receive(&position_request(
        4,
        "textDocument/hover",
        &source,
        buffered_output_line,
        buffered_output_character,
    ));
    let response = hover.response().unwrap();
    assert!(
        response.contains("pub struct BlockingBufWriter<W>"),
        "{response}"
    );
    assert!(hover.issue().is_none(), "{:?}", hover.issue());

    let (line_read_line, line_read_source) = source_line(&text, "reader.read_line_into_blocking");
    let line_read_character = line_read_source.find("read_line_into_blocking").unwrap();
    let hover = server.receive(&position_request(
        5,
        "textDocument/hover",
        &source,
        line_read_line,
        line_read_character,
    ));
    let response = hover.response().unwrap();
    assert!(
        response.contains(
            "pub blocking method &+BlockingBufReader<R>.read_line_into_blocking(destination: &+String): bool!"
        ),
        "{response}"
    );
    assert!(hover.issue().is_none(), "{:?}", hover.issue());

    let (sort_line, sort_source) = source_line(&text, "paths.sort()");
    let sort_character = sort_source.find("sort").unwrap();
    let definition = server.receive(&position_request(
        6,
        "textDocument/definition",
        &source,
        sort_line,
        sort_character,
    ));
    let response = definition.response().unwrap();
    assert!(response.contains("/std/slice/index.nct"), "{response}");
    assert!(definition.issue().is_none(), "{:?}", definition.issue());
}

#[test]
fn json_normalize_uses_public_json_editor_semantics_end_to_end() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../examples/json-normalize");
    let source = root.join("normalize.nct");
    let (mut server, text) = open_package_source(&root, &source);

    let (parse_line, parse_source) = source_line(&text, "let value = json.parse");
    let parse_character = parse_source.find("parse").unwrap();
    let hover = server.receive(&position_request(
        2,
        "textDocument/hover",
        &source,
        parse_line,
        parse_character,
    ));
    let response = hover.response().unwrap();
    assert!(
        response.contains("pub func parse(text: &str): Value!"),
        "{response}"
    );
    for internal in [
        "ParserState",
        "Continuation",
        "EncodingFrame",
        "EncoderConstruction",
    ] {
        assert!(!response.contains(internal), "{response}");
    }
    assert!(hover.issue().is_none(), "{:?}", hover.issue());

    let definition = server.receive(&position_request(
        3,
        "textDocument/definition",
        &source,
        parse_line,
        parse_character,
    ));
    let response = definition.response().unwrap();
    assert!(response.contains("/std/json/index.nct"), "{response}");
    assert!(!response.contains("parsing.nct"), "{response}");
    assert!(definition.issue().is_none(), "{:?}", definition.issue());

    let (write_line, write_source) = source_line(&text, "json.write(&+output, &value)");
    let write_argument = write_source.find("&value").unwrap() + 2;
    let signature = server.receive(&position_request(
        4,
        "textDocument/signatureHelp",
        &source,
        write_line,
        write_argument,
    ));
    let response = signature.response().unwrap();
    assert!(
        response.contains(concat!(
            "func write<BlockingFile>(destination: &+BlockingFile, ",
            "value: &Value): void!"
        )),
        "{response}"
    );
    assert!(response.contains("\"activeParameter\":1"), "{response}");
    assert!(!response.contains("GenerationAttempt"), "{response}");
    assert!(signature.issue().is_none(), "{:?}", signature.issue());

    let incomplete = text.replace("output.write_text_blocking(\"\\n\")", "output.");
    let mut incomplete_json = String::new();
    nocter_json::write_string(&mut incomplete_json, &incomplete);
    let changed = server.receive(&format!(
        "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didChange\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\",\"version\":2}},\"contentChanges\":[{{\"text\":{incomplete_json}}}]}}}}",
        source.display()
    ));
    assert_eq!(
        changed.analysis().unwrap().snapshot().unwrap().status(),
        nocter_analysis::AnalysisStatus::SyntaxFailed
    );
    let (completion_line, completion_source) = source_line(&incomplete, "output.");
    let completion_character = completion_source.find("output.").unwrap() + "output.".len();
    let completion = server.receive(&position_request(
        5,
        "textDocument/completion",
        &source,
        completion_line,
        completion_character,
    ));
    let response = completion.response().unwrap();
    for method in ["flush_blocking", "write_blocking", "write_text_blocking"] {
        assert!(
            response.contains(&format!("\"label\":\"{method}\",\"kind\":2")),
            "{response}"
        );
    }
    for internal in ["emit", "Encoder", "EncodingStep"] {
        assert!(!response.contains(internal), "{response}");
    }
    assert!(completion.issue().is_none(), "{:?}", completion.issue());
}

#[test]
fn unicode_text_uses_one_checked_contract_across_editor_features() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../examples");
    let source = root.join("unicode-text.nct");
    let (mut server, text) = open_package_source(&root, &source);

    let (lower_line, lower_source) = source_line(&text, "trimmed.to_lowercase");
    let lower_character = lower_source.find("to_lowercase").unwrap();
    let hover = server.receive(&position_request(
        2,
        "textDocument/hover",
        &source,
        lower_line,
        lower_character,
    ));
    let response = hover.response().unwrap();
    assert!(
        response.contains("pub method &str.to_lowercase(): String"),
        "{response}"
    );
    assert!(hover.issue().is_none(), "{:?}", hover.issue());

    let definition = server.receive(&position_request(
        3,
        "textDocument/definition",
        &source,
        lower_line,
        lower_character,
    ));
    let response = definition.response().unwrap();
    assert!(response.contains("/std/str/index.nct"), "{response}");
    assert!(definition.issue().is_none(), "{:?}", definition.issue());

    let implementation = server.receive(&position_request(
        4,
        "textDocument/implementation",
        &source,
        lower_line,
        lower_character,
    ));
    let response = implementation.response().unwrap();
    assert!(response.contains("/std/str/casing.nct"), "{response}");
    assert!(
        implementation.issue().is_none(),
        "{:?}",
        implementation.issue()
    );

    let references = server.receive(&format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":5,\"method\":\"textDocument/references\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\"}},\"position\":{{\"line\":{lower_line},\"character\":{lower_character}}},\"context\":{{\"includeDeclaration\":true}}}}}}",
        source.display()
    ));
    let response = references.response().unwrap();
    for location in [
        "/std/str/index.nct",
        "/std/str/casing.nct",
        "/unicode-text.nct",
    ] {
        assert!(response.contains(location), "{response}");
    }
    assert!(references.issue().is_none(), "{:?}", references.issue());

    assert_unicode_text_assistance(&mut server, &source, &text);
}

fn assert_unicode_text_assistance(server: &mut super::LanguageServer, source: &Path, text: &str) {
    let (truncate_line, truncate_source) = source_line(text, "suffix.truncate");
    let truncate_argument = truncate_source.find("1)").unwrap() + 1;
    let signature = server.receive(&position_request(
        6,
        "textDocument/signatureHelp",
        source,
        truncate_line,
        truncate_argument,
    ));
    let response = signature.response().unwrap();
    assert!(
        response.contains("method &+String.truncate(byte_len: usize): void!"),
        "{response}"
    );
    assert!(response.contains("\"activeParameter\":0"), "{response}");
    assert!(signature.issue().is_none(), "{:?}", signature.issue());

    let tokens = server.receive(&format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"textDocument/semanticTokens/full\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\"}}}}}}",
        source.display()
    ));
    let response = tokens.response().unwrap();
    assert!(response.contains("\"data\":["), "{response}");
    assert!(!response.contains("\"data\":[]"), "{response}");
    assert!(tokens.issue().is_none(), "{:?}", tokens.issue());

    let incomplete = text.replace(
        "if trimmed != \"ΟΣ\" || trimmed.char_count() != 2 {",
        "if trimmed. {",
    );
    let mut incomplete_json = String::new();
    nocter_json::write_string(&mut incomplete_json, &incomplete);
    let changed = server.receive(&format!(
        "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didChange\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\",\"version\":2}},\"contentChanges\":[{{\"text\":{incomplete_json}}}]}}}}",
        source.display()
    ));
    assert_ne!(
        changed.analysis().unwrap().snapshot().unwrap().status(),
        nocter_analysis::AnalysisStatus::Complete
    );
    let (completion_line, completion_source) = source_line(&incomplete, "if trimmed.");
    let completion_character = completion_source.find("trimmed.").unwrap() + "trimmed.".len();
    let completion = server.receive(&position_request(
        8,
        "textDocument/completion",
        source,
        completion_line,
        completion_character,
    ));
    let response = completion.response().unwrap();
    for method in ["to_lowercase", "to_uppercase"] {
        assert!(
            response.contains(&format!("\"label\":\"{method}\",\"kind\":2")),
            "{response}"
        );
    }
    assert!(completion.issue().is_none(), "{:?}", completion.issue());
}

#[test]
fn text_banner_uses_public_text_and_output_editor_semantics_end_to_end() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../examples/text-banner");
    let source = root.join("banner.nct");
    let (mut server, text) = open_package_source(&root, &source);

    let (trim_line, trim_source) = source_line(&text, "input.trim_ascii()");
    let trim_character = trim_source.find("trim_ascii").unwrap();
    let hover = server.receive(&position_request(
        2,
        "textDocument/hover",
        &source,
        trim_line,
        trim_character,
    ));
    let response = hover.response().unwrap();
    assert!(
        response.contains("pub noalloc method &str.trim_ascii(): &str from self"),
        "{response}"
    );
    assert!(hover.issue().is_none(), "{:?}", hover.issue());

    let (println_line, println_source) = source_line(&text, "io.println(&report)");
    let println_character = println_source.find("println").unwrap();
    let hover = server.receive(&position_request(
        3,
        "textDocument/hover",
        &source,
        println_line,
        println_character,
    ));
    let response = hover.response().unwrap();
    assert!(
        response.contains("pub noalloc blocking func println(text: &str): void!"),
        "{response}"
    );
    assert!(hover.issue().is_none(), "{:?}", hover.issue());

    let definition = server.receive(&position_request(
        4,
        "textDocument/definition",
        &source,
        println_line,
        println_character,
    ));
    let response = definition.response().unwrap();
    assert!(response.contains("/std/io/index.nct"), "{response}");
    assert!(definition.issue().is_none(), "{:?}", definition.issue());

    let incomplete = text.replace("io.println(&report)?", "io.pr");
    let mut incomplete_json = String::new();
    nocter_json::write_string(&mut incomplete_json, &incomplete);
    let changed = server.receive(&format!(
        "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didChange\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\",\"version\":2}},\"contentChanges\":[{{\"text\":{incomplete_json}}}]}}}}",
        source.display()
    ));
    assert_eq!(
        changed.analysis().unwrap().snapshot().unwrap().status(),
        nocter_analysis::AnalysisStatus::CompilationFailed
    );
    let (completion_line, completion_source) = incomplete
        .lines()
        .enumerate()
        .find(|(_, line)| line.trim_end().ends_with("io.pr"))
        .unwrap();
    let completion_character = completion_source.find("io.pr").unwrap() + "io.pr".len();
    let completion = server.receive(&position_request(
        5,
        "textDocument/completion",
        &source,
        completion_line,
        completion_character,
    ));
    let response = completion.response().unwrap();
    for function in ["print", "println"] {
        assert!(
            response.contains(&format!("\"label\":\"io.{function}\"")),
            "{response}"
        );
    }
    assert!(completion.issue().is_none(), "{:?}", completion.issue());
}

#[test]
fn stdin_prefix_uses_public_process_and_input_editor_semantics_end_to_end() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../examples/stdin-prefix");
    let source = root.join("prefix.nct");
    let (mut server, text) = open_package_source(&root, &source);

    let (stdin_line, stdin_source) = source_line(&text, "BlockingBufReader.new(io.stdin())");
    let stdin_character = stdin_source.find("stdin").unwrap();
    let hover = server.receive(&position_request(
        2,
        "textDocument/hover",
        &source,
        stdin_line,
        stdin_character,
    ));
    let response = hover.response().unwrap();
    assert!(
        response.contains("pub noalloc func stdin(): BlockingFile"),
        "{response}"
    );
    assert!(hover.issue().is_none(), "{:?}", hover.issue());

    let definition = server.receive(&position_request(
        3,
        "textDocument/definition",
        &source,
        stdin_line,
        stdin_character,
    ));
    let response = definition.response().unwrap();
    assert!(response.contains("/std/io/index.nct"), "{response}");
    assert!(!response.contains("input.nct"), "{response}");
    assert!(definition.issue().is_none(), "{:?}", definition.issue());

    let (read_line, read_source) = source_line(&text, "input.read_line_into_blocking");
    let read_character = read_source.find("read_line_into_blocking").unwrap();
    let hover = server.receive(&position_request(
        4,
        "textDocument/hover",
        &source,
        read_line,
        read_character,
    ));
    let response = hover.response().unwrap();
    assert!(
        response.contains(
            "pub blocking method &+BlockingBufReader<R>.read_line_into_blocking(destination: &+String): bool!"
        ),
        "{response}"
    );
    assert!(hover.issue().is_none(), "{:?}", hover.issue());

    let incomplete = text.replace("io.stdin()", "io.st");
    let mut incomplete_json = String::new();
    nocter_json::write_string(&mut incomplete_json, &incomplete);
    let changed = server.receive(&format!(
        "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didChange\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\",\"version\":2}},\"contentChanges\":[{{\"text\":{incomplete_json}}}]}}}}",
        source.display()
    ));
    assert_eq!(
        changed.analysis().unwrap().snapshot().unwrap().status(),
        nocter_analysis::AnalysisStatus::CompilationFailed
    );
    let (completion_line, completion_source) = source_line(&incomplete, "io.st");
    let completion_character = completion_source.find("io.st").unwrap() + "io.st".len();
    let completion = server.receive(&position_request(
        5,
        "textDocument/completion",
        &source,
        completion_line,
        completion_character,
    ));
    let response = completion.response().unwrap();
    for function in ["stderr", "stdin", "stdout"] {
        assert!(
            response.contains(&format!("\"label\":\"io.{function}\"")),
            "{response}"
        );
    }
    assert!(completion.issue().is_none(), "{:?}", completion.issue());
}

#[test]
fn async_file_uses_one_public_contract_across_editor_features() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/standard");
    let source = root.join("async-file-runtime.nct");
    let (mut server, text) = open_package_source(&root, &source);

    let (create_line, create_source) = source_line(&text, "File.create(path)");
    let create_character = create_source.find("create").unwrap();
    let hover = server.receive(&position_request(
        2,
        "textDocument/hover",
        &source,
        create_line,
        create_character,
    ));
    let response = hover.response().unwrap();
    assert!(
        response.contains("pub async func File.create(path: &str): File!"),
        "{response}"
    );
    assert!(hover.issue().is_none(), "{:?}", hover.issue());

    let definition = server.receive(&position_request(
        3,
        "textDocument/definition",
        &source,
        create_line,
        create_character,
    ));
    let response = definition.response().unwrap();
    assert!(response.contains("/std/io/index.nct"), "{response}");
    assert!(definition.issue().is_none(), "{:?}", definition.issue());

    let implementation = server.receive(&position_request(
        4,
        "textDocument/implementation",
        &source,
        create_line,
        create_character,
    ));
    let response = implementation.response().unwrap();
    assert!(response.contains("/std/io/async_file.nct"), "{response}");
    assert!(
        implementation.issue().is_none(),
        "{:?}",
        implementation.issue()
    );
}

#[test]
fn subprocess_status_uses_one_public_contract_across_editor_features() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../examples/subprocess-status");
    let source = root.join("status.nct");
    let (mut server, text) = open_package_source(&root, &source);

    let (status_line, status_source) = source_line(&text, "command.status_blocking()");
    let status_character = status_source.rfind("status").unwrap();
    let hover = server.receive(&position_request(
        2,
        "textDocument/hover",
        &source,
        status_line,
        status_character,
    ));
    let response = hover.response().unwrap();
    assert!(
        response.contains("pub blocking method Command.status_blocking(): ExitStatus!"),
        "{response}"
    );
    assert!(hover.issue().is_none(), "{:?}", hover.issue());

    let definition = server.receive(&position_request(
        3,
        "textDocument/definition",
        &source,
        status_line,
        status_character,
    ));
    let response = definition.response().unwrap();
    assert!(response.contains("/std/process/index.nct"), "{response}");
    assert!(definition.issue().is_none(), "{:?}", definition.issue());

    let implementation = server.receive(&position_request(
        4,
        "textDocument/implementation",
        &source,
        status_line,
        status_character,
    ));
    let response = implementation.response().unwrap();
    assert!(
        response.contains("/std/process/closed_command_darwin.nct"),
        "{response}"
    );
    assert!(
        implementation.issue().is_none(),
        "{:?}",
        implementation.issue()
    );

    let incomplete = text.replace("if status.success()", "if status.");
    let mut incomplete_json = String::new();
    nocter_json::write_string(&mut incomplete_json, &incomplete);
    let changed = server.receive(&format!(
        "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didChange\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\",\"version\":2}},\"contentChanges\":[{{\"text\":{incomplete_json}}}]}}}}",
        source.display()
    ));
    assert_eq!(
        changed.analysis().unwrap().snapshot().unwrap().status(),
        nocter_analysis::AnalysisStatus::SyntaxFailed
    );
    let (completion_line, completion_source) = source_line(&incomplete, "if status.");
    let completion_character = completion_source.find("status.").unwrap() + "status.".len();
    let completion = server.receive(&position_request(
        5,
        "textDocument/completion",
        &source,
        completion_line,
        completion_character,
    ));
    let response = completion.response().unwrap();
    for method in ["code", "signal", "success"] {
        assert!(
            response.contains(&format!("\"label\":\"{method}\",\"kind\":2")),
            "{response}"
        );
    }
    assert!(completion.issue().is_none(), "{:?}", completion.issue());
}

#[test]
fn subprocess_output_uses_one_public_contract_across_editor_features() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../examples/subprocess-output");
    let source = root.join("capture.nct");
    let (mut server, text) = open_package_source(&root, &source);

    let (output_line, output_source) = source_line(&text, "command.output_blocking()");
    let output_character = output_source.rfind("output").unwrap();
    let hover = server.receive(&position_request(
        2,
        "textDocument/hover",
        &source,
        output_line,
        output_character,
    ));
    let response = hover.response().unwrap();
    assert!(
        response.contains("pub blocking method Command.output_blocking(): Output!"),
        "{response}"
    );
    assert!(hover.issue().is_none(), "{:?}", hover.issue());

    let definition = server.receive(&position_request(
        3,
        "textDocument/definition",
        &source,
        output_line,
        output_character,
    ));
    let response = definition.response().unwrap();
    assert!(response.contains("/std/process/index.nct"), "{response}");
    assert!(definition.issue().is_none(), "{:?}", definition.issue());

    let implementation = server.receive(&position_request(
        4,
        "textDocument/implementation",
        &source,
        output_line,
        output_character,
    ));
    let response = implementation.response().unwrap();
    assert!(
        response.contains("/std/process/closed_command_darwin.nct"),
        "{response}"
    );
    assert!(
        implementation.issue().is_none(),
        "{:?}",
        implementation.issue()
    );

    let incomplete = text.replace("if output.status.success()", "if output.status.");
    let mut incomplete_json = String::new();
    nocter_json::write_string(&mut incomplete_json, &incomplete);
    let changed = server.receive(&format!(
        "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didChange\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\",\"version\":2}},\"contentChanges\":[{{\"text\":{incomplete_json}}}]}}}}",
        source.display()
    ));
    assert_eq!(
        changed.analysis().unwrap().snapshot().unwrap().status(),
        nocter_analysis::AnalysisStatus::SyntaxFailed
    );
    let (completion_line, completion_source) = source_line(&incomplete, "output.status.");
    let completion_character =
        completion_source.find("output.status.").unwrap() + "output.status.".len();
    let completion = server.receive(&position_request(
        5,
        "textDocument/completion",
        &source,
        completion_line,
        completion_character,
    ));
    let response = completion.response().unwrap();
    for method in ["code", "signal", "success"] {
        assert!(
            response.contains(&format!("\"label\":\"{method}\",\"kind\":2")),
            "{response}"
        );
    }
    assert!(completion.issue().is_none(), "{:?}", completion.issue());
}

#[test]
fn configured_subprocess_uses_one_public_contract_across_editor_features() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../examples/subprocess-configured");
    let source = root.join("configured.nct");
    let (mut server, text) = open_package_source(&root, &source);

    let (method_line, method_source) = source_line(&text, "command.current_dir");
    let method_character = method_source.find("current_dir").unwrap();
    let hover = server.receive(&position_request(
        2,
        "textDocument/hover",
        &source,
        method_line,
        method_character,
    ));
    let response = hover.response().unwrap();
    assert!(
        response.contains("pub method &+Command.current_dir(path: &str): void!"),
        "{response}"
    );
    assert!(hover.issue().is_none(), "{:?}", hover.issue());

    let definition = server.receive(&position_request(
        3,
        "textDocument/definition",
        &source,
        method_line,
        method_character,
    ));
    let response = definition.response().unwrap();
    assert!(response.contains("/std/process/index.nct"), "{response}");
    assert!(definition.issue().is_none(), "{:?}", definition.issue());

    let implementation = server.receive(&position_request(
        4,
        "textDocument/implementation",
        &source,
        method_line,
        method_character,
    ));
    let response = implementation.response().unwrap();
    assert!(
        response.contains("/std/process/configuration.nct"),
        "{response}"
    );
    assert!(
        implementation.issue().is_none(),
        "{:?}",
        implementation.issue()
    );

    let incomplete = text.replace("command.clear_env()", "if command. { return 90 }");
    let mut incomplete_json = String::new();
    nocter_json::write_string(&mut incomplete_json, &incomplete);
    let changed = server.receive(&format!(
        "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didChange\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\",\"version\":2}},\"contentChanges\":[{{\"text\":{incomplete_json}}}]}}}}",
        source.display()
    ));
    assert_ne!(
        changed.analysis().unwrap().snapshot().unwrap().status(),
        nocter_analysis::AnalysisStatus::Complete
    );
    let (completion_line, completion_source) = source_line(&incomplete, "if command.");
    let completion_character = completion_source.find("command.").unwrap() + "command.".len();
    let completion = server.receive(&position_request(
        5,
        "textDocument/completion",
        &source,
        completion_line,
        completion_character,
    ));
    let response = completion.response().unwrap();
    for method in [
        "arg",
        "clear_env",
        "current_dir",
        "env",
        "input",
        "output",
        "remove_env",
        "status",
    ] {
        assert!(
            response.contains(&format!("\"label\":\"{method}\",\"kind\":2")),
            "{response}"
        );
    }
    assert!(completion.issue().is_none(), "{:?}", completion.issue());
}

#[test]
fn subprocess_pipeline_uses_generic_io_and_structured_process_editor_contracts() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../examples/subprocess-pipeline");
    let source = root.join("pipeline.nct");
    let (mut server, text) = open_package_source(&root, &source);

    assert_pipeline_generic_copy_contracts(&mut server, &source, &text);
    assert_pipeline_child_completion(&mut server, &source, &text);
    assert_pipeline_source_projection(&mut server, &source, &text);
}

#[test]
fn async_file_report_uses_streaming_and_filesystem_editor_contracts() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../examples/async-file-report");
    let source = root.join("report.nct");
    let (mut server, text) = open_package_source(&root, &source);

    let (chunks_line, chunks_source) = source_line(&text, "ByteChunks.with_chunk_size");
    let chunks_character = chunks_source.find("with_chunk_size").unwrap();
    let hover = server.receive(&position_request(
        2,
        "textDocument/hover",
        &source,
        chunks_line,
        chunks_character,
    ));
    let response = hover.response().unwrap();
    assert!(
        response.contains("pub func ByteChunks<R>.with_chunk_size("),
        "{response}"
    );
    assert!(response.contains("R impl Reader"), "{response}");
    assert!(hover.issue().is_none(), "{:?}", hover.issue());

    let definition = server.receive(&position_request(
        3,
        "textDocument/definition",
        &source,
        chunks_line,
        chunks_character,
    ));
    let response = definition.response().unwrap();
    assert!(response.contains("/std/io/stream/index.nct"), "{response}");
    assert!(definition.issue().is_none(), "{:?}", definition.issue());

    let implementation = server.receive(&position_request(
        4,
        "textDocument/implementation",
        &source,
        chunks_line,
        chunks_character,
    ));
    let response = implementation.response().unwrap();
    assert!(response.contains("/std/io/stream/chunks.nct"), "{response}");
    assert!(
        implementation.issue().is_none(),
        "{:?}",
        implementation.issue()
    );

    let (next_line, next_source) = source_line(&text, "chunks.next()");
    let completion_character = next_source.find("chunks.").unwrap() + "chunks.".len();
    let completion = server.receive(&position_request(
        5,
        "textDocument/completion",
        &source,
        next_line,
        completion_character,
    ));
    let response = completion.response().unwrap();
    assert!(
        response.contains("\"label\":\"next\",\"kind\":2"),
        "{response}"
    );
    assert!(completion.issue().is_none(), "{:?}", completion.issue());

    let tokens = server.receive(&format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":6,\"method\":\"textDocument/semanticTokens/full\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\"}}}}}}",
        source.display()
    ));
    let response = tokens.response().unwrap();
    assert!(response.contains("\"data\":["), "{response}");
    assert!(!response.contains("\"data\":[]"), "{response}");
    assert!(tokens.issue().is_none(), "{:?}", tokens.issue());

    let end_line = text.lines().count();
    let hints = server.receive(&format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"textDocument/inlayHint\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\"}},\"range\":{{\"start\":{{\"line\":0,\"character\":0}},\"end\":{{\"line\":{end_line},\"character\":0}}}}}}}}",
        source.display()
    ));
    let response = hints.response().unwrap();
    for inferred in [": File", ": WalkDir"] {
        assert!(
            response.contains(&format!("\"label\":\"{inferred}\"")),
            "{response}"
        );
    }
    assert!(hints.issue().is_none(), "{:?}", hints.issue());
}

#[test]
fn binary_record_uses_one_codec_and_framing_contract_across_editor_features() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../examples/binary-record");
    let application = root.join("application.nct");
    let (mut server, application_text) = open_package_source(&root, &application);

    assert_binary_record_decode_contract(&mut server, &application, &application_text);

    let parsing = root.join("parsing.nct");
    let parsing_text = fs::read_to_string(&parsing).unwrap();
    assert_binary_record_prefix_contract(&mut server, &parsing, &parsing_text);

    let wire = root.join("wire.nct");
    let wire_text = fs::read_to_string(&wire).unwrap();
    assert_binary_record_checksum_contract(&mut server, &wire, &wire_text);
    assert_binary_record_source_projection(
        &mut server,
        [&application, &parsing, &wire],
        &parsing_text,
    );
}

fn assert_binary_record_decode_contract(
    server: &mut super::LanguageServer,
    application: &Path,
    text: &str,
) {
    let (decode_line, decode_source) = source_line(text, "decode_async");
    let decode_character = decode_source.find("decode_async").unwrap();
    let hover = server.receive(&position_request(
        2,
        "textDocument/hover",
        application,
        decode_line,
        decode_character,
    ));
    let response = hover.response().unwrap();
    assert!(
        response.contains("async func decode_async<R>"),
        "{response}"
    );
    assert!(response.contains("reader: &+R"), "{response}");
    assert!(response.contains(": RecordLog!"), "{response}");
    assert!(response.contains("R impl Reader"), "{response}");
    assert!(hover.issue().is_none(), "{:?}", hover.issue());

    let definition = server.receive(&position_request(
        3,
        "textDocument/definition",
        application,
        decode_line,
        decode_character,
    ));
    let response = definition.response().unwrap();
    assert!(response.contains("/binary-record/index.nct"), "{response}");
    assert!(definition.issue().is_none(), "{:?}", definition.issue());

    let implementation = server.receive(&position_request(
        4,
        "textDocument/implementation",
        application,
        decode_line,
        decode_character,
    ));
    let response = implementation.response().unwrap();
    assert!(
        response.contains("/binary-record/streams.nct"),
        "{response}"
    );
    assert!(
        implementation.issue().is_none(),
        "{:?}",
        implementation.issue()
    );
}

fn assert_binary_record_prefix_contract(
    server: &mut super::LanguageServer,
    parsing: &Path,
    text: &str,
) {
    let (uleb_line, uleb_source) = source_line(text, "bytes.decode_uleb128");
    let uleb_character = uleb_source.find("decode_uleb128").unwrap();
    let hover = server.receive(&position_request(
        5,
        "textDocument/hover",
        parsing,
        uleb_line,
        uleb_character,
    ));
    let response = hover.response().unwrap();
    assert!(
        response.contains("decode_uleb128(input: &[u8]): PrefixDecode<u64>"),
        "{response}"
    );
    assert!(hover.issue().is_none(), "{:?}", hover.issue());

    let definition = server.receive(&position_request(
        6,
        "textDocument/definition",
        parsing,
        uleb_line,
        uleb_character,
    ));
    let response = definition.response().unwrap();
    assert!(response.contains("/std/bytes/index.nct"), "{response}");
    assert!(definition.issue().is_none(), "{:?}", definition.issue());

    let implementation = server.receive(&position_request(
        7,
        "textDocument/implementation",
        parsing,
        uleb_line,
        uleb_character,
    ));
    let response = implementation.response().unwrap();
    assert!(response.contains("/std/bytes/variable.nct"), "{response}");
    assert!(
        implementation.issue().is_none(),
        "{:?}",
        implementation.issue()
    );

    let (cursor_line, cursor_source) = source_line(text, "cursor.take_u32_be");
    let completion_character = cursor_source.find("cursor.").unwrap() + "cursor.".len();
    let completion = server.receive(&position_request(
        8,
        "textDocument/completion",
        parsing,
        cursor_line,
        completion_character,
    ));
    let response = completion.response().unwrap();
    for method in ["remaining", "take", "take_u32_be", "take_uleb128"] {
        assert!(
            response.contains(&format!("\"label\":\"{method}\",\"kind\":2")),
            "{response}"
        );
    }
    assert!(completion.issue().is_none(), "{:?}", completion.issue());
}

fn assert_binary_record_checksum_contract(
    server: &mut super::LanguageServer,
    wire: &Path,
    text: &str,
) {
    let (crc_line, crc_source) = source_line(text, "checksum.crc32");
    let crc_character = crc_source.find("crc32").unwrap();
    let hover = server.receive(&position_request(
        9,
        "textDocument/hover",
        wire,
        crc_line,
        crc_character,
    ));
    let response = hover.response().unwrap();
    assert!(response.contains("crc32(input: &[u8]): u32"), "{response}");
    assert!(hover.issue().is_none(), "{:?}", hover.issue());

    let definition = server.receive(&position_request(
        10,
        "textDocument/definition",
        wire,
        crc_line,
        crc_character,
    ));
    let response = definition.response().unwrap();
    assert!(response.contains("/std/checksum/index.nct"), "{response}");
    assert!(definition.issue().is_none(), "{:?}", definition.issue());

    let implementation = server.receive(&position_request(
        11,
        "textDocument/implementation",
        wire,
        crc_line,
        crc_character,
    ));
    let response = implementation.response().unwrap();
    assert!(response.contains("/std/checksum/crc32.nct"), "{response}");
    assert!(
        implementation.issue().is_none(),
        "{:?}",
        implementation.issue()
    );
}

fn assert_binary_record_source_projection(
    server: &mut super::LanguageServer,
    sources: [&Path; 3],
    parsing_text: &str,
) {
    for (id, source) in [(12, sources[0]), (13, sources[1]), (14, sources[2])] {
        let tokens = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":{id},\"method\":\"textDocument/semanticTokens/full\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\"}}}}}}",
            source.display()
        ));
        let response = tokens.response().unwrap();
        assert!(response.contains("\"data\":["), "{response}");
        assert!(!response.contains("\"data\":[]"), "{response}");
        assert!(tokens.issue().is_none(), "{:?}", tokens.issue());
    }

    let hints = server.receive(&format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":15,\"method\":\"textDocument/inlayHint\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\"}},\"range\":{{\"start\":{{\"line\":0,\"character\":0}},\"end\":{{\"line\":{},\"character\":0}}}}}}}}",
        sources[1].display(),
        parsing_text.lines().count()
    ));
    let response = hints.response().unwrap();
    assert!(response.contains("\"label\":\": u32\""), "{response}");
    assert!(response.contains("\"label\":\": u64\""), "{response}");
    assert!(hints.issue().is_none(), "{:?}", hints.issue());
}

#[test]
fn archive_inspection_preserves_compression_archive_and_policy_boundaries_in_editor_features() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../examples/archive-inspect");
    let inspection = root.join("inspection.nct");
    let (mut server, text) = open_package_source(&root, &inspection);

    assert_archive_reader_contract(&mut server, &inspection, &text);
    assert_archive_stream_contract(&mut server, &inspection, &text);
    assert_archive_source_projection(&mut server, &root, &inspection, &text);
}

fn assert_archive_reader_contract(server: &mut super::LanguageServer, source: &Path, text: &str) {
    let (line, line_source) = source_line(text, "BlockingGzipReader.with_capacity");
    let character = line_source.find("with_capacity").unwrap();
    let hover = server.receive(&position_request(
        2,
        "textDocument/hover",
        source,
        line,
        character,
    ));
    let response = hover.response().unwrap();
    assert!(
        response.contains("func BlockingGzipReader<R>.with_capacity"),
        "{response}"
    );
    assert!(hover.issue().is_none(), "{:?}", hover.issue());

    let definition = server.receive(&position_request(
        3,
        "textDocument/definition",
        source,
        line,
        character,
    ));
    let response = definition.response().unwrap();
    assert!(response.contains("/std/compress/index.nct"), "{response}");
    assert!(definition.issue().is_none(), "{:?}", definition.issue());

    let implementation = server.receive(&position_request(
        4,
        "textDocument/implementation",
        source,
        line,
        character,
    ));
    let response = implementation.response().unwrap();
    assert!(
        response.contains("/std/compress/blocking_reader.nct"),
        "{response}"
    );
    assert!(
        implementation.issue().is_none(),
        "{:?}",
        implementation.issue()
    );
}

fn assert_archive_stream_contract(server: &mut super::LanguageServer, source: &Path, text: &str) {
    let (line, line_source) = source_line(text, "archive.next_blocking");
    let character = line_source.find("next_blocking").unwrap();
    let hover = server.receive(&position_request(
        5,
        "textDocument/hover",
        source,
        line,
        character,
    ));
    let response = hover.response().unwrap();
    assert!(
        response.contains("blocking func next_blocking<R>"),
        "{response}"
    );
    assert!(response.contains(": TarStreamStep!"), "{response}");
    assert!(hover.issue().is_none(), "{:?}", hover.issue());

    let definition = server.receive(&position_request(
        6,
        "textDocument/definition",
        source,
        line,
        character,
    ));
    let response = definition.response().unwrap();
    assert!(response.contains("/std/archive/index.nct"), "{response}");
    assert!(definition.issue().is_none(), "{:?}", definition.issue());

    let implementation = server.receive(&position_request(
        7,
        "textDocument/implementation",
        source,
        line,
        character,
    ));
    let response = implementation.response().unwrap();
    assert!(
        response.contains("/std/archive/blocking_stream.nct"),
        "{response}"
    );
    assert!(
        implementation.issue().is_none(),
        "{:?}",
        implementation.issue()
    );

    let (entry_line, entry_source) = source_line(text, "tar.entry()");
    let completion_character = entry_source.find("tar.").unwrap() + "tar.".len();
    let completion = server.receive(&position_request(
        8,
        "textDocument/completion",
        source,
        entry_line,
        completion_character,
    ));
    let response = completion.response().unwrap();
    assert!(
        response.contains("\"label\":\"entry\",\"kind\":2"),
        "{response}"
    );
    assert!(completion.issue().is_none(), "{:?}", completion.issue());
}

fn assert_archive_source_projection(
    server: &mut super::LanguageServer,
    root: &Path,
    inspection: &Path,
    inspection_text: &str,
) {
    for (id, source) in [
        (9, inspection.to_path_buf()),
        (10, root.join("source.nct")),
        (11, root.join("application.nct")),
    ] {
        let tokens = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":{id},\"method\":\"textDocument/semanticTokens/full\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\"}}}}}}",
            source.display()
        ));
        let response = tokens.response().unwrap();
        assert!(response.contains("\"data\":["), "{response}");
        assert!(!response.contains("\"data\":[]"), "{response}");
        assert!(tokens.issue().is_none(), "{:?}", tokens.issue());
    }

    let hints = server.receive(&format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":12,\"method\":\"textDocument/inlayHint\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\"}},\"range\":{{\"start\":{{\"line\":0,\"character\":0}},\"end\":{{\"line\":{},\"character\":0}}}}}}}}",
        inspection.display(),
        inspection_text.lines().count()
    ));
    let response = hints.response().unwrap();
    for inferred in [
        ": LimitedSource<R>",
        ": BlockingGzipReader<LimitedSource<R>>",
        ": TarStream",
    ] {
        assert!(
            response.contains(&format!("\"label\":\"{inferred}\"")),
            "{response}"
        );
    }
    assert!(hints.issue().is_none(), "{:?}", hints.issue());
}

fn assert_pipeline_generic_copy_contracts(
    server: &mut super::LanguageServer,
    source: &Path,
    text: &str,
) {
    let (copy_line, copy_source) = source_line(text, "await io.copy");
    let copy_character = copy_source.find("copy").unwrap();
    let hover = server.receive(&position_request(
        2,
        "textDocument/hover",
        source,
        copy_line,
        copy_character,
    ));
    let response = hover.response().unwrap();
    assert!(
        response.contains("pub async func copy<R, W>("),
        "{response}"
    );
    assert!(
        response.contains("R impl Reader, W impl Writer"),
        "{response}"
    );
    assert!(hover.issue().is_none(), "{:?}", hover.issue());

    let definition = server.receive(&position_request(
        3,
        "textDocument/definition",
        source,
        copy_line,
        copy_character,
    ));
    let response = definition.response().unwrap();
    assert!(response.contains("/std/io/index.nct"), "{response}");
    assert!(definition.issue().is_none(), "{:?}", definition.issue());

    let implementation = server.receive(&position_request(
        4,
        "textDocument/implementation",
        source,
        copy_line,
        copy_character,
    ));
    let response = implementation.response().unwrap();
    assert!(response.contains("/std/io/transfer.nct"), "{response}");
    assert!(
        implementation.issue().is_none(),
        "{:?}",
        implementation.issue()
    );

    let copy_argument = copy_source.find("&+writer").unwrap() + 2;
    let signature = server.receive(&position_request(
        5,
        "textDocument/signatureHelp",
        source,
        copy_line,
        copy_argument,
    ));
    let response = signature.response().unwrap();
    assert!(
        response.contains("func copy<ChildStdout, ChildStdin>("),
        "{response}"
    );
    assert!(
        response.contains("ChildStdout impl Reader, ChildStdin impl Writer"),
        "{response}"
    );
    assert!(response.contains("\"activeParameter\":1"), "{response}");
    assert!(signature.issue().is_none(), "{:?}", signature.issue());
}

fn assert_pipeline_child_completion(server: &mut super::LanguageServer, source: &Path, text: &str) {
    let (producer_line, producer_source) = source_line(text, "producer.take_stdout");
    let completion_character = producer_source.find("producer.").unwrap() + "producer.".len();
    let completion = server.receive(&position_request(
        6,
        "textDocument/completion",
        source,
        producer_line,
        completion_character,
    ));
    let response = completion.response().unwrap();
    for method in [
        "kill",
        "take_stderr",
        "take_stdin",
        "take_stdout",
        "terminate",
        "try_wait",
    ] {
        assert!(
            response.contains(&format!("\"label\":\"{method}\",\"kind\":2")),
            "{response}"
        );
    }
    assert!(completion.issue().is_none(), "{:?}", completion.issue());
}

fn assert_pipeline_source_projection(
    server: &mut super::LanguageServer,
    source: &Path,
    text: &str,
) {
    let tokens = server.receive(&format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"textDocument/semanticTokens/full\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\"}}}}}}",
        source.display()
    ));
    let response = tokens.response().unwrap();
    assert!(response.contains("\"data\":["), "{response}");
    assert!(!response.contains("\"data\":[]"), "{response}");
    assert!(tokens.issue().is_none(), "{:?}", tokens.issue());

    let end_line = text.lines().count();
    let hints = server.receive(&format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":8,\"method\":\"textDocument/inlayHint\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\"}},\"range\":{{\"start\":{{\"line\":0,\"character\":0}},\"end\":{{\"line\":{end_line},\"character\":0}}}}}}}}",
        source.display()
    ));
    let response = hints.response().unwrap();
    for inferred in [": ProcessIo", ": Command", ": Child"] {
        assert!(
            response.contains(&format!("\"label\":\"{inferred}\"")),
            "{response}"
        );
    }
    assert!(hints.issue().is_none(), "{:?}", hints.issue());
}

fn open_package_source(root: &Path, source: &Path) -> (super::LanguageServer, String) {
    let text = fs::read_to_string(source).unwrap();
    let mut server = semantic_server(root);
    server.receive(&format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
        root.display()
    ));
    server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
    let mut did_open = format!(
        "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\",\"languageId\":\"nocter\",\"version\":1,\"text\":",
        source.display()
    );
    nocter_json::write_string(&mut did_open, &text);
    did_open.push_str("}}}");
    let opened = server.receive(&did_open);
    let snapshot = opened.analysis().unwrap().snapshot().unwrap();
    assert_eq!(
        snapshot.status(),
        nocter_analysis::AnalysisStatus::Complete,
        "{:?}",
        snapshot.diagnostics()
    );
    assert!(opened.issue().is_none(), "{:?}", opened.issue());
    (server, text)
}

fn source_line<'a>(source: &'a str, needle: &str) -> (usize, &'a str) {
    source
        .lines()
        .enumerate()
        .find(|(_, line)| line.contains(needle))
        .unwrap()
}

fn position_request(
    id: usize,
    method: &str,
    source: &Path,
    line: usize,
    character: usize,
) -> String {
    format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":{id},\"method\":\"{method}\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\"}},\"position\":{{\"line\":{line},\"character\":{character}}}}}}}",
        source.display()
    )
}
