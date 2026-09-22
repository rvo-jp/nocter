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
fn public_lifecycle_observation_reports_reload_and_termination_signals() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    let image = compile_single_file_native_source(
        &package_root,
        &standard_root,
        "use std/service\n\
         use std/service.LifecycleRequest\n\
         async func main(): i32! {\n\
             match await service.lifecycle_requested()? {\n\
                 LifecycleRequest.reload {\n\
                     match await service.lifecycle_requested()? {\n\
                         LifecycleRequest.reload { return 41 }\n\
                         LifecycleRequest.interrupt { return 52 }\n\
                         LifecycleRequest.terminate { return 53 }\n\
                     }\n\
                 }\n\
                 LifecycleRequest.interrupt { return 42 }\n\
                 LifecycleRequest.terminate { return 43 }\n\
             }\n\
         }\n",
    );

    execute_after_process_signal(&image, &package_root.0, "service-reload", "HUP", 41);
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
    use std::time::{Duration, Instant};

    let executable = root.join(name);
    let launcher_ready = root.join(format!("{name}.launcher-ready"));
    fs::write(&executable, image.bytes()).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    let mut child = Command::new("/bin/sh")
        .arg("-c")
        .arg("trap '' \"$1\"; : > \"$2\"; exec \"$3\"")
        .arg("nocter-service-signal-launcher")
        .arg(signal)
        .arg(&launcher_ready)
        .arg(&executable)
        .current_dir(root)
        .spawn()
        .unwrap();

    // The launcher makes the selected disposition harmless before exec. Signals sent before the
    // Nocter process installs its kqueue source are ignored; repeating delivery then synchronizes
    // on observable process completion instead of a host-load-dependent startup delay.
    let launcher_deadline = Instant::now() + Duration::from_secs(5);
    while !launcher_ready.exists() {
        if let Some(status) = child.try_wait().unwrap() {
            panic!("signal-test launcher exited before readiness with {status:?}");
        }
        if Instant::now() >= launcher_deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("signal-test launcher did not become ready");
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    let observation_deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        let delivery = Command::new("/bin/kill")
            .arg(format!("-{signal}"))
            .arg(child.id().to_string())
            .status()
            .unwrap();
        if !delivery.success() {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            panic!("could not deliver SIG{signal}");
        }
        if Instant::now() >= observation_deadline {
            let _ = child.kill();
            let status = child.wait().unwrap();
            panic!("service did not observe SIG{signal} before timeout: {status:?}");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(
        status.code(),
        Some(expected),
        "service terminated with {status:?} after SIG{signal}"
    );
}
