use super::*;

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn public_service_scope_cancels_and_joins_owned_work() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    let image = compile_single_file_native_source(
        &package_root,
        &standard_root,
        "use std/service.{ServiceAdmission, ServiceCompletion, ServiceScope}\n\
         use std/sync.CancellationToken\n\
         async func worker(token: CancellationToken): void! {\n\
             await token.cancelled()\n\
             return\n\
         }\n\
         async func main(): i32 {\n\
             var scope = ServiceScope.new() catch _ { return 1 }\n\
             let token = scope.token()\n\
             match scope.add(worker(move token)) {\n\
                 ServiceAdmission.accepted {}\n\
                 ServiceAdmission.stopped(_) { return 2 }\n\
             }\n\
             match await scope.shutdown() {\n\
                 ServiceCompletion.completed { return 0 }\n\
                 ServiceCompletion.failed(_) { return 3 }\n\
             }\n\
         }\n",
    );

    execute_native_status(&image, &package_root.0, "service-scope", 0);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn public_termination_observation_reports_process_signals() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    let image = compile_single_file_native_source(
        &package_root,
        &standard_root,
        "use std/service\n\
         use std/service.TerminationSignal\n\
         async func main(): i32! {\n\
             match await service.termination_requested()? {\n\
                 TerminationSignal.interrupt { return 42 }\n\
                 TerminationSignal.terminate { return 43 }\n\
             }\n\
         }\n",
    );

    execute_after_process_signal(&image, &package_root.0, "service-interrupt", "INT", 42);
    execute_after_process_signal(&image, &package_root.0, "service-terminate", "TERM", 43);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn execute_after_process_signal(
    image: &NativeImage,
    root: &Path,
    name: &str,
    signal: &str,
    expected: i32,
) {
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;
    use std::time::Duration;

    let executable = root.join(name);
    fs::write(&executable, image.bytes()).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    let mut child = Command::new(&executable).current_dir(root).spawn().unwrap();
    // Compilation tests may run under heavy parallel load. Delay signal delivery until the root
    // computation has installed the process-owned source; the child then proves readiness-driven
    // observation rather than startup timing.
    std::thread::sleep(Duration::from_secs(1));
    let delivery = Command::new("/bin/kill")
        .arg(format!("-{signal}"))
        .arg(child.id().to_string())
        .status()
        .unwrap();
    assert!(delivery.success(), "could not deliver SIG{signal}");
    let status = child.wait().unwrap();
    assert_eq!(
        status.code(),
        Some(expected),
        "service terminated with {status:?} after SIG{signal}"
    );
}
