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
