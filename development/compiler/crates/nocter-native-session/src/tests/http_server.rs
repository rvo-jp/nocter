use nocter_discovery::DiscoveryRequest;
use nocter_model::{CompilationTarget, PackageIdentity};
use nocter_session::ExecutableCompileRequest;
use nocter_standard_profile::bundled_standard_toolchain;

use super::{
    TempPackage, compile_for_test, compile_native_image, discover, execute_native_status,
    package_graph, resolved_standard,
};

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn public_http_server_deadlines_cancel_owned_operations_without_poisoning_listener() {
    let compiler_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        "use std/http.Server\n\
         use std/net\n\
         use std/net.{IpAddress, Ipv4Address, SocketAddress}\n\
         use std/time.Duration\n\
         \n\
         func loopback(): SocketAddress {\n\
             return SocketAddress.new(IpAddress.from_ipv4(Ipv4Address.loopback()), 0)\n\
         }\n\
         \n\
         async func main(): i32 {\n\
             var server = await Server.bind(loopback()) catch _ { return 1 }\n\
             let short = Duration.from_milliseconds(5)\n\
             let _idle = await server.accept_with_timeout(short) catch accept_failure {\n\
                 if !accept_failure.has_code(\"std.http.timed_out\") { return 2 }\n\
                 let address = server.local_address() catch _ { return 3 }\n\
                 var client = await net.connect_tcp(address) catch _ { return 4 }\n\
                 let connection = await server.accept_with_timeout(\n\
                     Duration.from_seconds(1),\n\
                 ) catch _ { return 5 }\n\
                 let _request = await connection.read_request_with_timeout(short)\n\
                     catch read_failure {\n\
                         client.close()\n\
                         if read_failure.has_code(\"std.http.timed_out\") { return 0 }\n\
                         return 6\n\
                     }\n\
                 client.close()\n\
                 return 7\n\
             }\n\
             return 8\n\
         }\n",
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();
    let target = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(target)).unwrap();

    execute_native_status(image.image(), &package_root.0, "http-server-deadlines", 0);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn public_http_server_streams_request_and_response_bodies_through_linear_authority() {
    let compiler_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        "use std/http.{Client, Request, ResponseHead, Server}\n\
         use std/net.{IpAddress, Ipv4Address, SocketAddress}\n\
         use std/string.String\n\
         use std/task\n\
         use std/time.Duration\n\
         use std/url.Url\n\
         use std/vec.Vec\n\
         \n\
         func loopback(): SocketAddress {\n\
             return SocketAddress.new(IpAddress.from_ipv4(Ipv4Address.loopback()), 0)\n\
         }\n\
         \n\
         async func serve_once(server: Server, timeout: Duration): void! {\n\
             var owner = move server\n\
             let connection = await owner.accept_with_timeout(timeout)?\n\
             var request = await connection.read_request_with_timeout(timeout)?\n\
             if request.target() != \"/stream\" {\n\
                 return error.new(\"test.target\", \"request target changed\")\n\
             }\n\
             var prefix: Vec<u8> = Vec [u8.truncate(0), u8.truncate(0)]\n\
             let count = await request.read_with_timeout(&+prefix, timeout)?\n\
             if count != 2 || prefix[0] != 104 || prefix[1] != 101 {\n\
                 return error.new(\"test.body\", \"streaming request prefix changed\")\n\
             }\n\
             let responder = await request.finish_body_with_timeout(timeout)?\n\
             let head = ResponseHead.ok()\n\
             var writer = await responder.begin_chunked_with_timeout(move head, timeout)?\n\
             await writer.write_with_timeout(\"o\".bytes(), timeout)?\n\
             await writer.write_with_timeout(\"k\".bytes(), timeout)?\n\
             let _next = await writer.finish_with_timeout(timeout)?\n\
             return\n\
         }\n\
         \n\
         async func main(): i32 {\n\
             let timeout = Duration.from_seconds(1)\n\
             let server = await Server.bind(loopback()) catch _ { return 1 }\n\
             let address = server.local_address() catch _ { return 2 }\n\
             let port = address.port().to_string()\n\
             let url_text = String.concat(\"http://127.0.0.1:\", &port, \"/stream\")\n\
             var request = Request.post(Url.parse(&url_text) catch _ { return 3 })\n\
                 catch _ { return 4 }\n\
             request.set_text_body(\"hello\")\n\
             let client = Client.new()\n\
             let exchange = await task.join(\n\
                 client.send_with_timeout(move request, timeout),\n\
                 serve_once(move server, timeout),\n\
             )\n\
             var response = move exchange.0 catch _ { return 5 }\n\
             move exchange.1 catch _ { return 6 }\n\
             let body = await response.read_to_string_with_timeout(timeout) catch _ { return 7 }\n\
             if body != \"ok\" { return 8 }\n\
             return 0\n\
         }\n",
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();
    let target = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(target)).unwrap();

    execute_native_status(image.image(), &package_root.0, "http-server-streaming", 0);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn public_http_server_reuses_one_connection_and_retains_pipelined_input() {
    let compiler_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        "use std/http.{OutgoingResponse, Server}\n\
         use std/net\n\
         use std/net.{IpAddress, Ipv4Address, SocketAddress}\n\
         use std/task\n\
         use std/time.Duration\n\
         use std/vec.Vec\n\
         \n\
         func loopback(): SocketAddress {\n\
             return SocketAddress.new(IpAddress.from_ipv4(Ipv4Address.loopback()), 0)\n\
         }\n\
         \n\
         async func serve_two(server: Server, timeout: Duration): void! {\n\
             var owner = move server\n\
             let first_connection = await owner.accept_with_timeout(timeout)?\n\
             let first_request = await first_connection.read_request_with_timeout(timeout)?\n\
             if first_request.target() != \"/one\" {\n\
                 return error.new(\"test.first\", \"first retained request changed\")\n\
             }\n\
             let first_responder = await first_request.finish_body_with_timeout(timeout)?\n\
             var first_response = OutgoingResponse.ok()\n\
             first_response.set_text_body(\"a\")\n\
             let next = await first_responder.respond_with_timeout(\n\
                 move first_response,\n\
                 timeout,\n\
             )?\n\
             let second_connection = move next otherwise {\n\
                 return error.new(\"test.reuse\", \"reusable connection was not returned\")\n\
             }\n\
             let second_request = await second_connection.read_request_with_timeout(timeout)?\n\
             if second_request.target() != \"/two\" {\n\
                 return error.new(\"test.second\", \"second retained request changed\")\n\
             }\n\
             let second_responder = await second_request.finish_body_with_timeout(timeout)?\n\
             var second_response = OutgoingResponse.ok()\n\
             second_response.set_text_body(\"b\")\n\
             let terminal = await second_responder.respond_with_timeout(\n\
                 move second_response,\n\
                 timeout,\n\
             )?\n\
             let unexpected = move terminal otherwise { return }\n\
             unexpected.close()\n\
             return error.new(\"test.close\", \"Connection close returned reusable authority\")\n\
         }\n\
         \n\
         async func exchange(address: SocketAddress, timeout: Duration): void! {\n\
             var stream = await net.connect_tcp(address)?\n\
             await stream.write_with_timeout(\n\
                 \"GET /one HTTP/1.1\\r\\nHost: localhost\\r\\n\\r\\nGET /two HTTP/1.1\\r\\nHost: localhost\\r\\nConnection: close\\r\\n\\r\\n\".bytes(),\n\
                 timeout,\n\
             )?\n\
             var scratch: Vec<u8> = Vec.with_capacity(256)\n\
             while scratch.len() < 256 { scratch.push(u8.truncate(0)) }\n\
             var received: usize = 0\n\
             loop {\n\
                 let count = await stream.read_with_timeout(&+scratch, timeout)?\n\
                 if count == 0 {\n\
                     if received == 0 {\n\
                         return error.new(\"test.response\", \"server returned no response bytes\")\n\
                     }\n\
                     return\n\
                 }\n\
                 received += count\n\
             }\n\
         }\n\
         \n\
         async func main(): i32 {\n\
             let timeout = Duration.from_seconds(1)\n\
             let server = await Server.bind(loopback()) catch _ { return 1 }\n\
             let address = server.local_address() catch _ { return 2 }\n\
             let completed = await task.join(\n\
                 serve_two(move server, timeout),\n\
                 exchange(address, timeout),\n\
             )\n\
             move completed.0 catch _ { return 3 }\n\
             move completed.1 catch _ { return 4 }\n\
             return 0\n\
         }\n",
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();
    let target = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(target)).unwrap();

    execute_native_status(image.image(), &package_root.0, "http-server-reuse", 0);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn public_http_server_gracefully_drains_and_cancels_every_owned_typestate() {
    let compiler_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        include_str!("../../../../tests/fixtures/native/http_graceful_shutdown.nct"),
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();
    let target = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(target)).unwrap();

    execute_native_status(image.image(), &package_root.0, "http-server-shutdown", 0);
}
