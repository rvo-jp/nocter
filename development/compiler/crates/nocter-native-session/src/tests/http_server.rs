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
fn public_http_server_streams_and_drains_request_bodies_before_response_authority() {
    let compiler_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let standard_root = compiler_root.join("../std");
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        "use std/http.{Client, OutgoingResponse, Request, Server}\n\
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
             var response = OutgoingResponse.ok()\n\
             response.set_text_body(\"ok\")\n\
             await responder.respond_with_timeout(move response, timeout)?\n\
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
