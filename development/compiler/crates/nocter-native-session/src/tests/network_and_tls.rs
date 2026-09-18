use super::*;

#[test]
fn standard_network_contract_crosses_native_tests() {
    let standard_root = nocter_test_support::standard_library_root();
    let standard_package = PackageIdentity::new("toolchain:std");
    let mut root_source = fs::read_to_string(standard_root.join("index.nct")).unwrap();
    root_source.push_str(concat!(
        "\n#test: { name: \"net\", module: \"./net\" }\n",
        "#test: { name: \"net-stream-policy\", module: \"./internal/net\" }\n",
        "#test: { name: \"net-resolver-adapter\", module: \"./internal/net/darwin\" }\n",
    ));
    let mut overlay = SourceOverlay::builder();
    overlay
        .insert_source(
            standard_root.join("index.nct"),
            SourceOverride::new(root_source.into_bytes()),
        )
        .unwrap();
    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph_with_overlay(
            vec![resolved_standard(&standard_root, &standard_package)],
            overlay.finish(),
        ),
        vec![
            ModuleIdentity::new(standard_package.clone(), Vec::<&str>::new()),
            ModuleIdentity::new(standard_package.clone(), ["net"]),
            ModuleIdentity::new(standard_package.clone(), ["internal", "net"]),
            ModuleIdentity::new(standard_package.clone(), ["internal", "net", "darwin"]),
        ],
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let target = compile_for_test(unit);
    let compiled = compile_native_tests(NativeTestCompileRequest::all(target)).unwrap();
    assert_eq!(compiled.targets().len(), 3);
    let output = TempPackage::new();
    let mut case_count = 0;
    for target in compiled.targets() {
        let NativeTestTargetOutcome::Compiled(cases) = target.outcome() else {
            panic!("standard network tests failed native compilation")
        };
        case_count += cases.len();
        for case in cases {
            execute_native_test(case.image(), &output.0, case.identity().name());
        }
    }
    assert_eq!(case_count, 30);
}

#[test]
fn standard_http_framing_contract_crosses_native_tests() {
    let standard_root = nocter_test_support::standard_library_root();
    let standard_package = PackageIdentity::new("toolchain:std");
    let mut root_source = fs::read_to_string(standard_root.join("index.nct")).unwrap();
    root_source.push_str("\n#test: { name: \"http\", module: \"./http\" }\n");
    let mut overlay = SourceOverlay::builder();
    overlay
        .insert_source(
            standard_root.join("index.nct"),
            SourceOverride::new(root_source.into_bytes()),
        )
        .unwrap();
    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph_with_overlay(
            vec![resolved_standard(&standard_root, &standard_package)],
            overlay.finish(),
        ),
        vec![
            ModuleIdentity::new(standard_package.clone(), Vec::<&str>::new()),
            ModuleIdentity::new(standard_package.clone(), ["http"]),
        ],
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let target = compile_for_test(unit);
    let compiled = compile_native_tests(NativeTestCompileRequest::all(target)).unwrap();
    assert_eq!(compiled.targets().len(), 1);
    let NativeTestTargetOutcome::Compiled(cases) = compiled.targets()[0].outcome() else {
        panic!(
            "standard HTTP framing tests failed native compilation: {:?}",
            compiled.targets()[0].outcome()
        )
    };
    assert_eq!(cases.len(), 47);
    let output = TempPackage::new();
    for case in cases {
        execute_native_test(case.image(), &output.0, case.identity().name());
    }
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn public_http_client_crosses_localhost_resolution_and_streaming_fixture() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    let fixture = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = fixture.local_addr().unwrap().port();
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        &format!(
            "use std/http.{{Client, Request}}\n\
             use std/io.BlockingReader\n\
             use std/url.Url\n\
             \n\
             blocking func main(): i32 {{\n\
                 let url = Url.parse(\"http://localhost:{port}/from-fixture?q=1\") catch _ {{ return 1 }}\n\
                 let request = Request.get(move url) catch _ {{ return 2 }}\n\
                 let client = Client.new()\n\
                 var response = client.send_blocking(move request) catch _ {{ return 3 }}\n\
                 if response.status().code() != 200 {{ return 4 }}\n\
                 let _fixture = response.headers().first(\"x-fixture\") otherwise {{ return 5 }}\n\
                 let body = response.read_to_string_blocking() catch _ {{ return 6 }}\n\
                 if body != \"fixture\" {{ return 7 }}\n\
                 return 0\n\
             }}\n"
        ),
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

    let server = thread::spawn(move || {
        let (mut stream, _) = fixture.accept().unwrap();
        let mut request = Vec::new();
        let mut scratch = [0_u8; 256];
        while !request.ends_with(b"\r\n\r\n") {
            let received = stream.read(&mut scratch).unwrap();
            assert_ne!(received, 0, "HTTP client closed before completing its head");
            request.extend_from_slice(&scratch[..received]);
        }
        assert_eq!(
            request,
            format!(
                "GET /from-fixture?q=1 HTTP/1.1\r\nhost: localhost:{port}\r\nconnection: close\r\ncontent-length: 0\r\n\r\n"
            )
            .into_bytes()
        );
        stream
            .write_all(
                b"HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 200 OK\r\nContent-Length: 7\r\nX-Fixture: yes\r\n\r\nfi",
            )
            .unwrap();
        thread::sleep(Duration::from_millis(10));
        stream.write_all(b"xture").unwrap();
    });

    execute_native_status(image.image(), &package_root.0, "http-client", 0);
    server.join().unwrap();
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn serve_plain_tls_peers_and_require_https_alpn(fixture: &std::net::TcpListener) {
    use std::io::{Read, Write};
    use std::thread;
    use std::time::Duration;

    for connection_index in 0..5 {
        let (mut stream, _) = fixture.accept().unwrap();
        let mut record_header = [0_u8; 5];
        stream.read_exact(&mut record_header).unwrap();
        assert_eq!(
            record_header[0], 22,
            "client did not begin with a TLS handshake"
        );
        let record_len = usize::from(u16::from_be_bytes([record_header[3], record_header[4]]));
        let mut client_hello = vec![0_u8; record_len];
        stream.read_exact(&mut client_hello).unwrap();
        if connection_index == 1 || connection_index == 4 {
            assert!(
                client_hello
                    .windows(b"http/1.1".len())
                    .any(|window| window == b"http/1.1"),
                "HTTPS did not advertise the HTTP/1.1 ALPN protocol"
            );
        }
        if connection_index == 0 {
            stream.write_all(&[22, 3, 3, 0, 16, 1, 2]).unwrap();
        } else {
            stream.write_all(b"this is not a TLS record").unwrap();
        }
        thread::sleep(Duration::from_millis(100));
    }
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn plain_tls_rejection_source(port: u16, asynchronous: bool) -> String {
    let main = if asynchronous {
        "async func main(): i32 {\n\
             if !await rejects_async() { return 1 }\n\
             if !await rejects_https_async() { return 2 }\n\
             return 0\n\
         }"
    } else {
        "blocking func main(): i32 {\n\
             if !rejects_sync() { return 1 }\n\
             if !rejects_https_sync() { return 2 }\n\
             if !rejects_invalid_custom_anchor() { return 3 }\n\
             return 0\n\
         }"
    };
    format!(
        "use std/http.{{Client, Request}}\n\
         use std/time.Duration\n\
         use std/tls as tls\n\
         use std/tls.{{TlsStream, TrustAnchor}}\n\
         use std/url.Url\n\
         \n\
         blocking func rejects_sync(): bool {{\n\
             let _stream = TlsStream.connect_with_timeout_blocking(\n\
                 \"localhost\",\n\
                 {port},\n\
                 Duration.from_seconds(1),\n\
             ) catch failure {{\n\
                 return failure.has_code(\"std.net.tls_failed\")\n\
             }}\n\
             return false\n\
         }}\n\
         \n\
         blocking func rejects_https_sync(): bool {{\n\
             let client = Client.new()\n\
             let request = Request.get(Url.parse(\"https://localhost:{port}/\") catch _ {{\n\
                 return false\n\
             }}) catch _ {{ return false }}\n\
             let _response = client.send_with_timeout_blocking(\n\
                 move request,\n\
                 Duration.from_seconds(1),\n\
             ) catch failure {{\n\
                 return failure.has_code(\"std.net.tls_failed\")\n\
             }}\n\
             return false\n\
         }}\n\
         \n\
         async func rejects_async(): bool {{\n\
             let pending = tls.connect_with_timeout(\n\
                 \"localhost\",\n\
                 {port},\n\
                 Duration.from_seconds(1),\n\
             )\n\
             let _stream = await pending catch failure {{\n\
                 return failure.has_code(\"std.net.tls_failed\")\n\
             }}\n\
             return false\n\
         }}\n\
         \n\
         async func rejects_https_async(): bool {{\n\
             let client = Client.new()\n\
             let request = Request.get(Url.parse(\"https://localhost:{port}/\") catch _ {{\n\
                 return false\n\
             }}) catch _ {{ return false }}\n\
             let pending_response = client.send_with_timeout(\n\
                 move request,\n\
                 Duration.from_seconds(1),\n\
             )\n\
             let _response = await pending_response catch failure {{\n\
                 return failure.has_code(\"std.net.tls_failed\")\n\
             }}\n\
             return false\n\
         }}\n\
         \n\
         blocking func rejects_invalid_custom_anchor(): bool {{\n\
             let anchor = TrustAnchor.from_der(\"x\".bytes()) catch _ {{ return false }}\n\
             let _stream = TlsStream.connect_with_trust_anchor_and_timeout_blocking(\n\
                 \"localhost\",\n\
                 {port},\n\
                 &anchor,\n\
                 Duration.from_seconds(1),\n\
             ) catch failure {{\n\
                 return failure.has_code(\"std.net.tls_failed\")\n\
             }}\n\
             return false\n\
         }}\n\
         \n\
         {main}\n"
    )
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn public_tls_and_https_reject_plain_peers_and_https_advertises_http1() {
    use std::net::TcpListener;
    use std::thread;

    let fixture = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = fixture.local_addr().unwrap().port();
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    let sync_image = compile_single_file_native_source(
        &package_root,
        &standard_root,
        &plain_tls_rejection_source(port, false),
    );
    let async_image = compile_single_file_native_source(
        &package_root,
        &standard_root,
        &plain_tls_rejection_source(port, true),
    );

    let server = thread::spawn(move || serve_plain_tls_peers_and_require_https_alpn(&fixture));
    execute_native_status(&sync_image, &package_root.0, "tls-plain-peer-sync", 0);
    execute_native_status(&async_image, &package_root.0, "tls-plain-peer-async", 0);
    server.join().unwrap();
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn tls_handshake_timeout_source(port: u16, asynchronous: bool) -> String {
    let main = if asynchronous {
        "async func main(): i32 {\n\
             let timeout = Duration.from_milliseconds(500)\n\
             if !await async_tls_times_out(timeout) { return 1 }\n\
             let client = Client.new()\n\
             if !await async_https_times_out(&client, timeout) { return 2 }\n\
             return 0\n\
         }"
    } else {
        "blocking func main(): i32 {\n\
             let timeout = Duration.from_milliseconds(500)\n\
             if !sync_tls_times_out(timeout) { return 1 }\n\
             let client = Client.new()\n\
             if !sync_https_times_out(&client, timeout) { return 2 }\n\
             return 0\n\
         }"
    };
    format!(
        "use std/http.{{Client, Request}}\n\
         use std/time.Duration\n\
         use std/tls as tls\n\
         use std/tls.TlsStream\n\
         use std/url.Url\n\
         \n\
         blocking func sync_tls_times_out(timeout: Duration): bool {{\n\
             let _stream = TlsStream.connect_with_timeout_blocking(\n\
                 \"localhost\",\n\
                 {port},\n\
                 timeout,\n\
             ) catch failure {{ return failure.has_code(\"std.net.timed_out\") }}\n\
             return false\n\
         }}\n\
         \n\
         async func async_tls_times_out(timeout: Duration): bool {{\n\
             let pending = tls.connect_with_timeout(\n\
                 \"localhost\",\n\
                 {port},\n\
                 timeout,\n\
             )\n\
             let _stream = await pending catch failure {{\n\
                 return failure.has_code(\"std.net.timed_out\")\n\
             }}\n\
             return false\n\
         }}\n\
         \n\
         blocking func sync_https_times_out(client: &Client, timeout: Duration): bool {{\n\
             let url = Url.parse(\"https://localhost:{port}/\") catch _ {{ return false }}\n\
             let request = Request.get(move url) catch _ {{ return false }}\n\
             let _response = client.send_with_timeout_blocking(move request, timeout) catch failure {{\n\
                 return failure.has_code(\"std.net.timed_out\")\n\
             }}\n\
             return false\n\
         }}\n\
         \n\
         async func async_https_times_out(client: &Client, timeout: Duration): bool {{\n\
             let url = Url.parse(\"https://localhost:{port}/\") catch _ {{ return false }}\n\
             let request = Request.get(move url) catch _ {{ return false }}\n\
             let pending = client.send_with_timeout(\n\
                 move request,\n\
                 timeout,\n\
             )\n\
             let _response = await pending catch failure {{\n\
                 return failure.has_code(\"std.net.timed_out\")\n\
             }}\n\
             return false\n\
         }}\n\
         \n\
         {main}\n"
    )
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn tls_and_https_handshakes_share_the_fixed_timeout_contract() {
    use std::io::ErrorKind;
    use std::net::TcpListener;
    use std::thread;
    use std::time::{Duration, Instant};

    let fixture = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = fixture.local_addr().unwrap().port();
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    let sync_image = compile_single_file_native_source(
        &package_root,
        &standard_root,
        &tls_handshake_timeout_source(port, false),
    );
    let async_image = compile_single_file_native_source(
        &package_root,
        &standard_root,
        &tls_handshake_timeout_source(port, true),
    );

    let server = thread::spawn(move || {
        fixture.set_nonblocking(true).unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut peers = Vec::new();
        while peers.len() < 4 && Instant::now() < deadline {
            match fixture.accept() {
                Ok((stream, _)) => peers.push(stream),
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("failed to accept a TLS timeout peer: {error}"),
            }
        }
        let accepted = peers.len();
        if accepted == 4 {
            thread::sleep(Duration::from_secs(1));
        }
        accepted
    });
    execute_native_status(
        &sync_image,
        &package_root.0,
        "tls-handshake-timeout-sync",
        0,
    );
    execute_native_status(
        &async_image,
        &package_root.0,
        "tls-handshake-timeout-async",
        0,
    );
    assert_eq!(
        server.join().unwrap(),
        4,
        "every TLS and HTTPS timeout path must reach the network peer"
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
struct LocalTlsServer(std::process::Child);

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
impl Drop for LocalTlsServer {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn start_local_tls_server(
    port: u16,
    certificate: &Path,
    key: &Path,
    advertise_http1: bool,
) -> LocalTlsServer {
    use std::process::Stdio;
    use std::thread;
    use std::time::Duration;

    let mut command = std::process::Command::new("/usr/bin/openssl");
    command.args([
        "s_server",
        "-accept",
        &port.to_string(),
        "-cert",
        certificate.to_str().unwrap(),
        "-key",
        key.to_str().unwrap(),
        "-quiet",
        "-www",
    ]);
    if advertise_http1 {
        command.args(["-alpn", "http/1.1"]);
    }
    let child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut server = LocalTlsServer(child);
    for _ in 0..100 {
        assert!(
            server.0.try_wait().unwrap().is_none(),
            "local TLS server exited"
        );
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return server;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("local TLS server did not start");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
struct LocalTlsHttpServer {
    child: std::process::Child,
    exchange: Option<std::thread::JoinHandle<()>>,
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
impl LocalTlsHttpServer {
    fn finish(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.exchange.take().unwrap().join().unwrap();
    }
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
impl Drop for LocalTlsHttpServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(exchange) = self.exchange.take() {
            let _ = exchange.join();
        }
    }
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn start_local_tls_http_server(
    port: u16,
    certificate: &Path,
    key: &Path,
    exchange_count: usize,
) -> LocalTlsHttpServer {
    use std::io::{Read, Write};
    use std::process::Stdio;
    use std::thread;
    use std::time::Duration;

    let mut child = std::process::Command::new("/usr/bin/openssl")
        .args([
            "s_server",
            "-accept",
            &port.to_string(),
            "-cert",
            certificate.to_str().unwrap(),
            "-key",
            key.to_str().unwrap(),
            "-alpn",
            "http/1.1",
            "-quiet",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut server_input = child.stdin.take().unwrap();
    let mut server_output = child.stdout.take().unwrap();
    let exchange = thread::spawn(move || {
        for _ in 0..exchange_count {
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                let mut byte = [0_u8; 1];
                server_output.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            server_input
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .unwrap();
            server_input.flush().unwrap();
        }
    });
    let mut server = LocalTlsHttpServer {
        child,
        exchange: Some(exchange),
    };
    for _ in 0..100 {
        assert!(
            server.child.try_wait().unwrap().is_none(),
            "local TLS HTTP server exited"
        );
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return server;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("local TLS HTTP server did not start");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn create_local_tls_fixture(configuration_root: &Path, output_root: &Path) {
    use std::process::Command;

    fn run(command: &mut Command) {
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "OpenSSL fixture generation failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let root_key = output_root.join("root-key.pem");
    let root_certificate = output_root.join("root-cert.pem");
    let leaf_key = output_root.join("localhost-key.pem");
    let leaf_request = output_root.join("localhost.csr");
    let leaf_certificate = output_root.join("localhost-cert.pem");
    let root_der = output_root.join("root-cert.der");
    let root_configuration = configuration_root.join("root.cnf");
    let leaf_configuration = configuration_root.join("localhost.cnf");

    run(Command::new("/usr/bin/openssl")
        .args([
            "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "365",
        ])
        .args(["-sha256", "-config"])
        .arg(&root_configuration)
        .arg("-keyout")
        .arg(&root_key)
        .arg("-out")
        .arg(&root_certificate));
    run(Command::new("/usr/bin/openssl")
        .args(["req", "-newkey", "rsa:2048", "-nodes", "-sha256", "-config"])
        .arg(&leaf_configuration)
        .arg("-keyout")
        .arg(&leaf_key)
        .arg("-out")
        .arg(&leaf_request));
    run(Command::new("/usr/bin/openssl")
        .args(["x509", "-req", "-in"])
        .arg(&leaf_request)
        .arg("-CA")
        .arg(&root_certificate)
        .arg("-CAkey")
        .arg(&root_key)
        .args([
            "-CAcreateserial",
            "-set_serial",
            "1000",
            "-days",
            "300",
            "-sha256",
        ])
        .arg("-extfile")
        .arg(&leaf_configuration)
        .args(["-extensions", "certificate", "-out"])
        .arg(&leaf_certificate));
    run(Command::new("/usr/bin/openssl")
        .args(["x509", "-in"])
        .arg(&root_certificate)
        .args(["-outform", "der", "-out"])
        .arg(&root_der));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn create_expired_local_tls_certificate(configuration_root: &Path, output_root: &Path) {
    use std::process::Command;

    std::fs::write(output_root.join("index.txt"), []).unwrap();
    std::fs::write(output_root.join("serial"), b"1001\n").unwrap();
    let output = Command::new("/usr/bin/openssl")
        .current_dir(output_root)
        .args(["ca", "-batch", "-config"])
        .arg(configuration_root.join("expired.cnf"))
        .args([
            "-startdate",
            "20000101000000Z",
            "-enddate",
            "20000102000000Z",
            "-in",
            "localhost.csr",
            "-out",
            "expired-localhost-cert.pem",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "expired OpenSSL fixture generation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn custom_trust_augments_system_roots_and_preserves_hostname_authentication() {
    use std::net::TcpListener;

    let reservation = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = reservation.local_addr().unwrap().port();
    drop(reservation);
    let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tls");
    let package_root = TempPackage::new();
    create_local_tls_fixture(&fixture_root, &package_root.0);
    let certificate = package_root.0.join("localhost-cert.pem");
    let key = package_root.0.join("localhost-key.pem");
    let _server = start_local_tls_server(port, &certificate, &key, true);
    let certificate_source =
        byte_vector_source(&fs::read(package_root.0.join("root-cert.der")).unwrap());

    let standard_root = nocter_test_support::standard_library_root();
    let sync_main = format!(
        "blocking func main(): i32 {{\n\
             let _system = TlsStream.connect_with_timeout_blocking(\n\
                 \"localhost\", {port}, Duration.from_seconds(1),\n\
             ) catch failure {{\n\
                 if !failure.has_code(\"std.net.tls_failed\") {{ return 1 }}\n\
                 let certificate = fs.read_blocking(\"root-cert.der\") catch _ {{ return 2 }}\n\
                 let anchor = TrustAnchor.from_der(&certificate) catch _ {{ return 3 }}\n\
                 var stream = TlsStream.connect_with_trust_anchor_and_timeout_blocking(\n\
                     \"localhost\", {port}, &anchor, Duration.from_seconds(1),\n\
                 ) catch _ {{ return 4 }}\n\
                 stream.close()\n\
                 if !rejects_mismatched_name(&anchor) {{ return 5 }}\n\
                 return 0\n\
             }}\n\
             return 6\n\
         }}"
    );
    let async_main = format!(
        "async func main(): i32 {{\n\
             let certificate: Vec<u8> = {certificate_source}\n\
             let anchor = TrustAnchor.from_der(&certificate) catch _ {{ return 2 }}\n\
             if !await accepts_asynchronously(&anchor) {{ return 3 }}\n\
             return 0\n\
         }}"
    );
    let source = |main: &str| {
        format!(
            "use std/fs\n\
             use std/time.Duration\n\
             use std/tls as tls\n\
             use std/tls.{{TlsStream, TrustAnchor}}\n\
             use std/vec.Vec\n\
             \n\
             blocking func rejects_mismatched_name(anchor: &TrustAnchor): bool {{\n\
                 let _stream = TlsStream.connect_with_trust_anchor_and_timeout_blocking(\n\
                     \"127.0.0.1\", {port}, anchor, Duration.from_seconds(1),\n\
                 ) catch failure {{ return failure.has_code(\"std.net.tls_failed\") }}\n\
                 return false\n\
             }}\n\
             \n\
             async func accepts_asynchronously(anchor: &TrustAnchor): bool {{\n\
                 let pending = tls.connect_with_trust_anchor_and_timeout(\n\
                     \"localhost\", {port}, anchor, Duration.from_seconds(1),\n\
                 )\n\
                 var stream = await pending catch _ {{ return false }}\n\
                 stream.close()\n\
                 return true\n\
             }}\n\
             \n\
             {main}\n"
        )
    };
    let sync_image =
        compile_single_file_native_source(&package_root, &standard_root, &source(&sync_main));
    let async_image =
        compile_single_file_native_source(&package_root, &standard_root, &source(&async_main));
    execute_native_status(&sync_image, &package_root.0, "tls-custom-trust-sync", 0);
    execute_native_status(&async_image, &package_root.0, "tls-custom-trust-async", 0);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn custom_trust_does_not_override_certificate_validity() {
    use std::net::TcpListener;

    let reservation = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = reservation.local_addr().unwrap().port();
    drop(reservation);
    let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tls");
    let package_root = TempPackage::new();
    create_local_tls_fixture(&fixture_root, &package_root.0);
    create_expired_local_tls_certificate(&fixture_root, &package_root.0);
    let certificate = package_root.0.join("expired-localhost-cert.pem");
    let key = package_root.0.join("localhost-key.pem");
    let _server = start_local_tls_server(port, &certificate, &key, true);

    let standard_root = nocter_test_support::standard_library_root();
    package_root.source(
        "main.nct",
        &format!(
            "use std/fs\n\
             use std/time.Duration\n\
             use std/tls.{{TlsStream, TrustAnchor}}\n\
             \n\
             blocking func main(): i32 {{\n\
                 let certificate = fs.read_blocking(\"root-cert.der\") catch _ {{ return 1 }}\n\
                 let anchor = TrustAnchor.from_der(&certificate) catch _ {{ return 2 }}\n\
                 let _stream = TlsStream.connect_with_trust_anchor_and_timeout_blocking(\n\
                     \"localhost\", {port}, &anchor, Duration.from_seconds(1),\n\
                 ) catch failure {{\n\
                     if failure.has_code(\"std.net.tls_failed\") {{ return 0 }}\n\
                     return 3\n\
                 }}\n\
                 return 4\n\
             }}\n"
        ),
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
    execute_native_status(image.image(), &package_root.0, "tls-expired-certificate", 0);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn custom_trust_crosses_sync_and_async_https_without_a_second_http_codec() {
    use std::net::TcpListener;

    let reservation = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = reservation.local_addr().unwrap().port();
    drop(reservation);
    let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tls");
    let package_root = TempPackage::new();
    create_local_tls_fixture(&fixture_root, &package_root.0);
    let certificate = package_root.0.join("localhost-cert.pem");
    let key = package_root.0.join("localhost-key.pem");
    let server = start_local_tls_http_server(port, &certificate, &key, 2);
    let certificate_source =
        byte_vector_source(&fs::read(package_root.0.join("root-cert.der")).unwrap());

    let standard_root = nocter_test_support::standard_library_root();
    let sync_main = "blocking func main(): i32 {\n\
             let certificate = fs.read_blocking(\"root-cert.der\") catch _ { return 1 }\n\
             let anchor = TrustAnchor.from_der(&certificate) catch _ { return 2 }\n\
             let client = Client.new().with_trust_anchor(move anchor)\n\
             return accepts_sync(&client)\n\
         }";
    let async_main = format!(
        "async func main(): i32 {{\n\
             let certificate: Vec<u8> = {certificate_source}\n\
             let anchor = TrustAnchor.from_der(&certificate) catch _ {{ return 2 }}\n\
             let client = Client.new().with_trust_anchor(move anchor)\n\
             if !await accepts_async(&client) {{ return 3 }}\n\
             return 0\n\
         }}"
    );
    let source = |main: &str| {
        format!(
            "use std/fs\n\
             use std/http.{{Client, Request}}\n\
             use std/time.Duration\n\
             use std/tls.TrustAnchor\n\
             use std/url.Url\n\
             use std/vec.Vec\n\
             \n\
             blocking func accepts_sync(client: &Client): i32 {{\n\
                 let url = Url.parse(\"https://localhost:{port}/\") catch _ {{ return 1 }}\n\
                 let request = Request.get(move url) catch _ {{ return 2 }}\n\
                 var response = client.send_with_timeout_blocking(\n\
                     move request,\n\
                     Duration.from_seconds(1),\n\
                 ) catch failure {{\n\
                     if failure.has_code(\"std.net.tls_failed\") {{ return 3 }}\n\
                     if failure.has_code(\"std.net.timed_out\") {{ return 4 }}\n\
                     if failure.has_code(\"std.http.premature_eof\") {{ return 5 }}\n\
                     if failure.has_code(\"std.http.invalid_syntax\") {{ return 6 }}\n\
                     return 7\n\
                 }}\n\
                 let accepted = response.status().code() == 200\n\
                 response.close()\n\
                 if accepted {{ return 0 }}\n\
                 return 4\n\
             }}\n\
             \n\
             async func accepts_async(client: &Client): bool {{\n\
                 let url = Url.parse(\"https://localhost:{port}/\") catch _ {{ return false }}\n\
                 let request = Request.get(move url) catch _ {{ return false }}\n\
                 let pending = client.send_with_timeout(\n\
                     move request,\n\
                     Duration.from_seconds(1),\n\
                 )\n\
                 var response = await pending catch _ {{ return false }}\n\
                 let accepted = response.status().code() == 200\n\
                 response.close()\n\
                 return accepted\n\
             }}\n\
             \n\
             {main}\n"
        )
    };
    let sync_image =
        compile_single_file_native_source(&package_root, &standard_root, &source(sync_main));
    let async_image =
        compile_single_file_native_source(&package_root, &standard_root, &source(&async_main));
    execute_native_status(&sync_image, &package_root.0, "https-custom-trust-sync", 0);
    execute_native_status(&async_image, &package_root.0, "https-custom-trust-async", 0);
    server.finish();
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn https_requires_the_negotiated_http1_application_protocol() {
    use std::net::TcpListener;

    let reservation = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = reservation.local_addr().unwrap().port();
    drop(reservation);
    let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tls");
    let package_root = TempPackage::new();
    create_local_tls_fixture(&fixture_root, &package_root.0);
    let certificate = package_root.0.join("localhost-cert.pem");
    let key = package_root.0.join("localhost-key.pem");
    let _server = start_local_tls_server(port, &certificate, &key, false);
    let certificate_source =
        byte_vector_source(&fs::read(package_root.0.join("root-cert.der")).unwrap());

    let standard_root = nocter_test_support::standard_library_root();
    let sync_main = "blocking func main(): i32 {\n\
             let certificate = fs.read_blocking(\"root-cert.der\") catch _ { return 1 }\n\
             let anchor = TrustAnchor.from_der(&certificate) catch _ { return 2 }\n\
             let client = Client.new().with_trust_anchor(move anchor)\n\
             if !rejects_sync(&client) { return 3 }\n\
             return 0\n\
         }";
    let async_main = format!(
        "async func main(): i32 {{\n\
             let certificate: Vec<u8> = {certificate_source}\n\
             let anchor = TrustAnchor.from_der(&certificate) catch _ {{ return 2 }}\n\
             let client = Client.new().with_trust_anchor(move anchor)\n\
             if !await rejects_async(&client) {{ return 3 }}\n\
             return 0\n\
         }}"
    );
    let source = |main: &str| {
        format!(
            "use std/fs\n\
             use std/http.{{Client, Request}}\n\
             use std/time.Duration\n\
             use std/tls.TrustAnchor\n\
             use std/url.Url\n\
             use std/vec.Vec\n\
             \n\
             blocking func rejects_sync(client: &Client): bool {{\n\
                 let url = Url.parse(\"https://localhost:{port}/\") catch _ {{ return false }}\n\
                 let request = Request.get(move url) catch _ {{ return false }}\n\
                 var response = client.send_with_timeout_blocking(\n\
                     move request,\n\
                     Duration.from_seconds(1),\n\
                 ) catch failure {{ return failure.has_code(\"std.net.tls_failed\") }}\n\
                 response.close()\n\
                 return false\n\
             }}\n\
             \n\
             async func rejects_async(client: &Client): bool {{\n\
                 let url = Url.parse(\"https://localhost:{port}/\") catch _ {{ return false }}\n\
                 let request = Request.get(move url) catch _ {{ return false }}\n\
                 let pending = client.send_with_timeout(\n\
                     move request,\n\
                     Duration.from_seconds(1),\n\
                 )\n\
                 var response = await pending catch failure {{\n\
                     return failure.has_code(\"std.net.tls_failed\")\n\
                 }}\n\
                 response.close()\n\
                 return false\n\
             }}\n\
             \n\
             {main}\n"
        )
    };
    let sync_image =
        compile_single_file_native_source(&package_root, &standard_root, &source(sync_main));
    let async_image =
        compile_single_file_native_source(&package_root, &standard_root, &source(&async_main));
    execute_native_status(&sync_image, &package_root.0, "https-alpn-required-sync", 0);
    execute_native_status(
        &async_image,
        &package_root.0,
        "https-alpn-required-async",
        0,
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn public_async_http_client_crosses_reactor_and_fragmented_body_fixture() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    let fixture = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = fixture.local_addr().unwrap().port();
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        &format!(
            "use std/fs\n\
             use std/http.{{Client, Request}}\n\
             use std/io.{{File, TimeoutReader}}\n\
             use std/io\n\
             use std/time.Duration\n\
             use std/url.Url\n\
             \n\
             async func main(): i32 {{\n\
                 let url = Url.parse(\"http://localhost:{port}/async?q=1\") catch _ {{ return 1 }}\n\
                 var request = Request.post(move url) catch _ {{ return 2 }}\n\
                 request.append_header_text(\"X-Request\", \"phase3\") catch _ {{ return 3 }}\n\
                 request.set_text_body(\"payload\")\n\
                 let client = Client.new()\n\
                 let pending = client.send(move request)\n\
                 var response = await pending catch _ {{ return 5 }}\n\
                 if response.status().code() != 200 {{ return 6 }}\n\
                 let _fixture = response.headers().first(\"x-fixture\") otherwise {{ return 7 }}\n\
                 var timed = TimeoutReader.new(&+response, Duration.from_seconds(1))\n\
                 var destination = await File.create(\"downloaded\") catch _ {{ return 8 }}\n\
                 let copied = await io.copy(&+timed, &+destination) catch _ {{ return 9 }}\n\
                 drop timed\n\
                 await destination.close() catch _ {{ return 10 }}\n\
                 response.close()\n\
                 if copied != 10 {{ return 11 }}\n\
                 let text = await fs.read_to_string(\"downloaded\") catch _ {{ return 12 }}\n\
                 if text != \"fragmented\" {{ return 13 }}\n\
                 return 0\n\
             }}\n"
        ),
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

    let server = thread::spawn(move || {
        let (mut stream, _) = fixture.accept().unwrap();
        let mut request = Vec::new();
        let mut scratch = [0_u8; 256];
        let head_end = loop {
            if let Some(offset) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                break offset + 4;
            }
            let received = stream.read(&mut scratch).unwrap();
            assert_ne!(
                received, 0,
                "async HTTP client closed before completing its head"
            );
            request.extend_from_slice(&scratch[..received]);
        };
        while request.len() < head_end + 7 {
            let received = stream.read(&mut scratch).unwrap();
            assert_ne!(
                received, 0,
                "async HTTP client closed before completing its body"
            );
            request.extend_from_slice(&scratch[..received]);
        }
        assert_eq!(
            &request[..head_end],
            format!(
                "POST /async?q=1 HTTP/1.1\r\nx-request: phase3\r\nhost: localhost:{port}\r\nconnection: close\r\ncontent-length: 7\r\n\r\n"
            )
            .as_bytes()
        );
        assert_eq!(&request[head_end..], b"payload");
        stream
            .write_all(
                b"HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nX-Fixture: yes\r\n\r\n4\r\nfrag\r\n",
            )
            .unwrap();
        thread::sleep(Duration::from_millis(10));
        stream.write_all(b"6\r\nmented\r\n0\r\n\r\n").unwrap();
    });

    execute_native_status(image.image(), &package_root.0, "async-http-client", 0);
    server.join().unwrap();
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn async_http_timeout_source(abandoned_port: u16, fixture_port: u16) -> String {
    format!(
        "use std/http.{{Client, Request}}\n\
         use std/time.Duration\n\
         use std/url.Url\n\
         use std/vec.Vec\n\
         \n\
         async func head_times_out(client: &Client, url: Url, timeout: Duration): bool {{\n\
             let request = Request.get(move url) catch _ {{ return false }}\n\
             let pending = client.send_with_timeout(\n\
                 move request,\n\
                 timeout,\n\
             )\n\
             let _response = await pending catch failure {{\n\
                 return failure.has_code(\"std.net.timed_out\")\n\
             }}\n\
             return false\n\
         }}\n\
         \n\
         async func body_times_out(client: &Client, url: Url, timeout: Duration): bool {{\n\
             let request = Request.get(move url) catch _ {{ return false }}\n\
             let pending = client.send_with_timeout(\n\
                 move request,\n\
                 Duration.from_seconds(1),\n\
             )\n\
             var response = await pending catch _ {{ return false }}\n\
             var buffer: Vec<u8> = Vec [u8.truncate(0)]\n\
             let abandoned = response.read(&+buffer)\n\
             drop abandoned\n\
             let _body = await response.read_to_end_with_timeout(timeout) catch failure {{\n\
                 return failure.has_code(\"std.net.timed_out\")\n\
             }}\n\
             return false\n\
         }}\n\
         \n\
         async func truncated_peer_fails(client: &Client, url: Url): bool {{\n\
             let request = Request.get(move url) catch _ {{ return false }}\n\
             let pending = client.send_with_timeout(\n\
                 move request,\n\
                 Duration.from_seconds(1),\n\
             )\n\
             var response = await pending catch _ {{ return false }}\n\
             var buffer: Vec<u8> = Vec [\n\
                 u8.truncate(0),\n\
                 u8.truncate(0),\n\
                 u8.truncate(0),\n\
                 u8.truncate(0),\n\
             ]\n\
             let first = await response.read_with_timeout(\n\
                 &+buffer,\n\
                 Duration.from_seconds(1),\n\
             ) catch _ {{ return false }}\n\
             if first != 2 {{ return false }}\n\
             let _second = await response.read_with_timeout(\n\
                 &+buffer,\n\
                 Duration.from_seconds(1),\n\
             ) catch failure {{\n\
                 return failure.has_code(\"std.http.premature_eof\")\n\
             }}\n\
             return false\n\
         }}\n\
         \n\
         async func main(): i32 {{\n\
             let client = Client.new()\n\
             let abandoned_request = Request.get(\n\
                 Url.parse(\"http://localhost:{abandoned_port}/abandoned\") catch _ {{ return 1 }},\n\
             ) catch _ {{ return 2 }}\n\
             let abandoned = client.send(move abandoned_request)\n\
             drop abandoned\n\
             let short = Duration.from_seconds(1)\n\
             let head_url = Url.parse(\"http://localhost:{fixture_port}/head\") catch _ {{ return 4 }}\n\
             if !await head_times_out(&client, move head_url, short) {{ return 5 }}\n\
             let body_url = Url.parse(\"http://localhost:{fixture_port}/body\") catch _ {{ return 6 }}\n\
             if !await body_times_out(&client, move body_url, short) {{ return 7 }}\n\
             let truncated_url = Url.parse(\"http://localhost:{fixture_port}/truncated\") catch _ {{ return 8 }}\n\
             if !await truncated_peer_fails(&client, move truncated_url) {{ return 9 }}\n\
             return 0\n\
         }}\n"
    )
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn public_async_http_timeouts_and_abandoned_operations_preserve_ownership() {
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::thread;
    use std::time::Duration;

    fn read_request_head(stream: &mut TcpStream) -> Vec<u8> {
        let mut request = Vec::new();
        let mut scratch = [0_u8; 256];
        while request.windows(4).all(|bytes| bytes != b"\r\n\r\n") {
            let received = stream.read(&mut scratch).unwrap();
            assert_ne!(received, 0, "HTTP client closed before sending its head");
            request.extend_from_slice(&scratch[..received]);
        }
        request
    }

    let abandoned_fixture = TcpListener::bind("127.0.0.1:0").unwrap();
    let abandoned_port = abandoned_fixture.local_addr().unwrap().port();
    let fixture = TcpListener::bind("127.0.0.1:0").unwrap();
    let fixture_port = fixture.local_addr().unwrap().port();
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        &async_http_timeout_source(abandoned_port, fixture_port),
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

    let server = thread::spawn(move || {
        let (mut head_stream, _) = fixture.accept().unwrap();
        let head_request = read_request_head(&mut head_stream);
        assert!(head_request.starts_with(b"GET /head HTTP/1.1\r\n"));
        thread::sleep(Duration::from_millis(1200));
        drop(head_stream);

        let (mut body_stream, _) = fixture.accept().unwrap();
        let body_request = read_request_head(&mut body_stream);
        assert!(body_request.starts_with(b"GET /body HTTP/1.1\r\n"));
        body_stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\n")
            .unwrap();
        thread::sleep(Duration::from_millis(1200));
        let _ = body_stream.write_all(b"late");

        let (mut truncated_stream, _) = fixture.accept().unwrap();
        let truncated_request = read_request_head(&mut truncated_stream);
        assert!(truncated_request.starts_with(b"GET /truncated HTTP/1.1\r\n"));
        truncated_stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nab")
            .unwrap();
    });

    execute_native_status(image.image(), &package_root.0, "async-http-timeouts", 0);
    server.join().unwrap();
    drop(abandoned_fixture);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn public_async_http_request_body_observes_write_backpressure_timeout() {
    use std::io::Read;
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    let fixture = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = fixture.local_addr().unwrap().port();
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    let body_chunk = "x".repeat(1024);
    package_root.source(
        "main.nct",
        &format!(
            "use std/http.{{Client, Method, Request}}\n\
             use std/time.Duration\n\
             use std/url.Url\n\
             use std/vec.Vec\n\
             \n\
             async func main(): i32 {{\n\
                 let body_text = \"{body_chunk}\".repeat(2048)\n\
                 let body_view: &str = &body_text\n\
                 let body = Vec.from_slice(body_view.bytes())\n\
                 let url = Url.parse(\"http://localhost:{port}/backpressure\") catch _ {{ return 1 }}\n\
                 var request = Request.new(Method.post(), move url) catch _ {{ return 2 }}\n\
                 request.set_body(move body)\n\
                 let client = Client.new()\n\
                 let pending = client.send_with_timeout(\n\
                     move request,\n\
                     Duration.from_seconds(1),\n\
                 )\n\
                 let _response = await pending catch failure {{\n\
                     if failure.has_code(\"std.net.timed_out\")\n\
                         && failure.message() == \"while writing the HTTP request body\" {{\n\
                         return 0\n\
                     }}\n\
                     return 4\n\
                 }}\n\
                 return 5\n\
             }}\n"
        ),
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

    let server = thread::spawn(move || {
        let (mut stream, _) = fixture.accept().unwrap();
        let mut request_prefix = Vec::new();
        let mut scratch = [0_u8; 256];
        while request_prefix.windows(4).all(|bytes| bytes != b"\r\n\r\n") {
            let received = stream.read(&mut scratch).unwrap();
            assert_ne!(received, 0, "HTTP client closed before its request body");
            request_prefix.extend_from_slice(&scratch[..received]);
        }
        assert!(request_prefix.starts_with(b"POST /backpressure HTTP/1.1\r\n"));
        thread::sleep(Duration::from_millis(1200));
    });

    execute_native_status(
        image.image(),
        &package_root.0,
        "async-http-write-timeout",
        0,
    );
    server.join().unwrap();
}
