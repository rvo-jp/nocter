use super::*;

#[test]
fn public_byte_codecs_cross_the_complete_native_session() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    let image = compile_single_file_native_source(
        &package_root,
        &standard_root,
        include_str!("../../../../tests/fixtures/native/bytes_codecs.nct"),
    );
    execute_native_status(&image, &package_root.0, "byte-codecs", 0);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn fixed_binary_staging_composes_with_both_writer_contracts() {
    let standard_root = nocter_test_support::standard_library_root();
    let fixture = include_str!("../../../../tests/fixtures/native/byte_stream_composition.nct");

    let blocking_package = TempPackage::new();
    let blocking_source =
        format!("{fixture}\nblocking func main(): i32! {{ return blocking_entry()? }}\n");
    let blocking_image =
        compile_single_file_native_source(&blocking_package, &standard_root, &blocking_source);
    execute_native_status(
        &blocking_image,
        &blocking_package.0,
        "binary-blocking-writer",
        0,
    );

    let async_package = TempPackage::new();
    let async_source =
        format!("{fixture}\nasync func main(): i32! {{ return await async_entry()? }}\n");
    let async_image =
        compile_single_file_native_source(&async_package, &standard_root, &async_source);
    execute_native_status(&async_image, &async_package.0, "binary-async-writer", 0);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn erased_readonly_callable_crosses_the_complete_native_session() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    let image = compile_single_file_native_source(
        &package_root,
        &standard_root,
        "func main(): i32 {\n\
         \x20   let factor: i32 = 3\n\
         \x20   let callback: any &func(i32): i32 = (&factor; value) { value * factor }\n\
         \x20   if callback(14) != 42 { return 1 }\n\
         \x20   if callback(7) != 21 { return 2 }\n\
         \x20   return 0\n\
         }\n",
    );
    execute_native_status(&image, &package_root.0, "erased-readonly-callable", 0);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn erased_readwrite_callable_crosses_the_complete_native_session() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    let image = compile_single_file_native_source(
        &package_root,
        &standard_root,
        "func main(): i32 {\n\
         \x20   var total: i32 = 4\n\
         \x20   var callback: any &+func(i32): i32 = (&+total; value) {\n\
         \x20       total += value\n\
         \x20       return total\n\
         \x20   }\n\
         \x20   if callback(3) != 7 { return 1 }\n\
         \x20   if callback(5) != 12 { return 2 }\n\
         \x20   return 0\n\
         }\n",
    );
    execute_native_status(&image, &package_root.0, "erased-readwrite-callable", 0);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn erased_callable_releases_an_owned_capture_after_invocation() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    let image = compile_single_file_native_source(
        &package_root,
        &standard_root,
        "use std/string.String\n\
         \n\
         func main(): i32 {\n\
         \x20   let text = String \"captured\"\n\
         \x20   let callback: any &func(): usize = (move text;) { text.len() }\n\
         \x20   if callback() != 8 { return 1 }\n\
         \x20   return 0\n\
         }\n",
    );
    execute_native_status(&image, &package_root.0, "erased-owned-capture", 0);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn erased_callable_deferred_result_crosses_the_complete_native_session() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    let image = compile_single_file_native_source(
        &package_root,
        &standard_root,
        "async func deferred_double(value: i32): i32 { value * 2 }\n\
         \n\
         async func main(): i32 {\n\
         \x20   let callback: any &func(i32): future i32 = (value) { deferred_double(value) }\n\
         \x20   if await callback(21) != 42 { return 1 }\n\
         \x20   if await callback(7) != 14 { return 2 }\n\
         \x20   return 0\n\
         }\n",
    );
    execute_native_status(&image, &package_root.0, "erased-deferred-callable", 0);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn erased_callable_fallible_result_crosses_a_persistent_async_frame() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    let image = compile_single_file_native_source(
        &package_root,
        &standard_root,
        "async func main(): i32! {\n\
         \x20   let callback: any &func(bool): i32! = (fail) {\n\
         \x20       if fail { return error.new(\"test.erased\", \"expected failure\") }\n\
         \x20       return 17\n\
         \x20   }\n\
         \x20   if callback(false)? != 17 { return 1 }\n\
         \x20   let _unexpected = callback(true) catch failure {\n\
         \x20       if failure.has_code(\"test.erased\") { return 0 }\n\
         \x20       return 2\n\
         \x20   }\n\
         \x20   return 3\n\
         }\n",
    );
    execute_native_status(&image, &package_root.0, "erased-fallible-callable", 0);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn erased_consuming_callable_moves_a_direct_environment() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    let image = compile_single_file_native_source(
        &package_root,
        &standard_root,
        "struct Token { value: i32 }\n\
         func consume(token: Token): i32 { token.value }\n\
         func main(): i32 {\n\
         \x20   let token = Token { value: 42 }\n\
         \x20   let callback: any func(): i32 = (move token;) { consume(move token) }\n\
         \x20   if callback() != 42 { return 1 }\n\
         \x20   return 0\n\
         }\n",
    );
    execute_native_status(&image, &package_root.0, "erased-consuming-callable", 0);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn erased_consuming_callable_destroys_a_retained_environment_once() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    let image = compile_single_file_native_source(
        &package_root,
        &standard_root,
        "struct Counter { value: i32 }\n\
         struct Token { value: i32\n    counter: &+Counter\n}\n\
         drop Token(&+self) { self.counter.value += 1 }\n\
         func main(): i32 {\n\
         \x20   var counter = Counter { value: 0 }\n\
         \x20   let token = Token { value: 42, counter: &+counter }\n\
         \x20   let callback: any func(): i32 = (move token;) { token.value }\n\
         \x20   if callback() != 42 { return 1 }\n\
         \x20   if counter.value != 1 { return 2 }\n\
         \x20   return 0\n\
         }\n",
    );
    execute_native_status(
        &image,
        &package_root.0,
        "erased-consuming-retained-environment",
        0,
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn erased_consuming_callable_does_not_redestroy_a_moved_environment() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    let image = compile_single_file_native_source(
        &package_root,
        &standard_root,
        "struct Counter { value: i32 }\n\
         struct Token { value: i32\n    counter: &+Counter\n}\n\
         drop Token(&+self) { self.counter.value += 1 }\n\
         func consume(token: Token): i32 { token.value }\n\
         func main(): i32 {\n\
         \x20   var counter = Counter { value: 0 }\n\
         \x20   let token = Token { value: 42, counter: &+counter }\n\
         \x20   let callback: any func(): i32 = (move token;) { consume(move token) }\n\
         \x20   if callback() != 42 { return 1 }\n\
         \x20   if counter.value != 1 { return 2 }\n\
         \x20   return 0\n\
         }\n",
    );
    execute_native_status(
        &image,
        &package_root.0,
        "erased-consuming-moved-environment",
        0,
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn erased_consuming_callable_preserves_fallible_results_during_cleanup() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    let image = compile_single_file_native_source(
        &package_root,
        &standard_root,
        "struct Counter { value: i32 }\n\
         struct Token { value: i32\n    counter: &+Counter\n}\n\
         drop Token(&+self) { self.counter.value += 1 }\n\
         func main(): i32! {\n\
         \x20   var counter = Counter { value: 0 }\n\
         \x20   let token = Token { value: 42, counter: &+counter }\n\
         \x20   let callback: any func(): i32! = (move token;) {\n\
         \x20       return error.new(\"test.consuming\", \"expected failure\")\n\
         \x20   }\n\
         \x20   let _unexpected = callback() catch failure {\n\
         \x20       if !failure.has_code(\"test.consuming\") { return 1 }\n\
         \x20       if counter.value != 1 { return 2 }\n\
         \x20       return 0\n\
         \x20   }\n\
         \x20   return 3\n\
         }\n",
    );
    execute_native_status(
        &image,
        &package_root.0,
        "erased-consuming-fallible-callable",
        0,
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn erased_consuming_callable_transfers_owned_state_into_future_results() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    let image = compile_single_file_native_source(
        &package_root,
        &standard_root,
        "struct Counter { value: i32 }\n\
         struct Token { value: i32\n    counter: &+Counter\n}\n\
         drop Token(&+self) { self.counter.value += 1 }\n\
         async func finish(token: Token): i32 { token.value }\n\
         async func main(): i32 {\n\
         \x20   var counter = Counter { value: 0 }\n\
         \x20   let first = Token { value: 42, counter: &+counter }\n\
         \x20   let completed: any func(): future i32 = (move first;) {\n\
         \x20       finish(move first)\n\
         \x20   }\n\
         \x20   if await completed() != 42 { return 1 }\n\
         \x20   if counter.value != 1 { return 2 }\n\
         \x20   let second = Token { value: 7, counter: &+counter }\n\
         \x20   let cancelled: any func(): future i32 = (move second;) {\n\
         \x20       finish(move second)\n\
         \x20   }\n\
         \x20   let pending = cancelled()\n\
         \x20   drop pending\n\
         \x20   if counter.value != 2 { return 3 }\n\
         \x20   return 0\n\
         }\n",
    );
    execute_native_status(
        &image,
        &package_root.0,
        "erased-consuming-future-callable",
        0,
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn erased_consuming_callable_cleans_staged_ownership_when_an_argument_fails() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    let image = compile_single_file_native_source(
        &package_root,
        &standard_root,
        "struct Counter { value: i32 }\n\
         struct Token { value: i32\n    counter: &+Counter\n}\n\
         drop Token(&+self) { self.counter.value += 1 }\n\
         func missing(): i32! {\n\
         \x20   return error.new(\"test.argument\", \"expected failure\")\n\
         }\n\
         func invoke(counter: &+Counter): void! {\n\
         \x20   let token = Token { value: 42, counter: counter }\n\
         \x20   let callback: any func(i32): i32 = (move token; value) {\n\
         \x20       value + token.value\n\
         \x20   }\n\
         \x20   let _result = callback(missing()?)\n\
         \x20   return\n\
         }\n\
         func main(): i32 {\n\
         \x20   var counter = Counter { value: 0 }\n\
         \x20   invoke(&+counter) catch failure {\n\
         \x20       if !failure.has_code(\"test.argument\") { return 1 }\n\
         \x20       if counter.value != 1 { return 2 }\n\
         \x20       return 0\n\
         \x20   }\n\
         \x20   return 3\n\
         }\n",
    );
    execute_native_status(
        &image,
        &package_root.0,
        "erased-consuming-argument-failure",
        0,
    );
}

#[test]
fn scalar_floating_values_cross_the_complete_native_session() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        "use std/vec.Vec\n\
         \n\
         const COMPILED_SUM: f64 = 0.1 + 0.2\n\
         const COMPILED_NAN: f64 = 0.0 / 0.0\n\
         const COMPILED_TINY: f64 = 5e-324\n\
         \n\
         func identity(value: f64): f64 { value }\n\
         func sum9(\n\
             a: f64, b: f64, c: f64, d: f64, e: f64,\n\
             f: f64, g: f64, h: f64, i: f64,\n\
         ): f64 { a + b + c + d + e + f + g + h + i }\n\
         func mixed9(\n\
             n0: i32, f0: f64, n1: i32, f1: f64, n2: i32, f2: f64,\n\
             n3: i32, f3: f64, n4: i32, f4: f64, n5: i32, f5: f64,\n\
             n6: i32, f6: f64, n7: i32, f7: f64, n8: i32, f8: f64,\n\
         ): f64 { n8 as f64 + f8 }\n\
         func narrow(value: f32): f32 { -(value * 2.0f32) }\n\
         func main(): i32 {\n\
             let retained = 1.25\n\
             let returned = identity(0.75)\n\
             let stacked = sum9(1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0)\n\
             if retained + returned != 2.0 { return 1 }\n\
             if stacked != 9.0 { return 2 }\n\
             if narrow(0.5f32) != -1.0f32 { return 3 }\n\
             if !(returned < retained) { return 4 }\n\
             var values = Vec [1.5, 2.5]\n\
             let popped = values.pop() otherwise { return 5 }\n\
             if popped != 2.5 { return 6 }\n\
             let signed: i32 = -7\n\
             if signed as f64 != -7.0 { return 7 }\n\
             let unsigned: u16 = 9\n\
             if unsigned as f32 != 9.0f32 { return 8 }\n\
             let widened = 1.5f32 as f64\n\
             if widened != 1.5 { return 9 }\n\
             let nan = 0.0 / 0.0\n\
             if nan <= 1.0 { return 10 }\n\
             if nan >= 1.0 { return 11 }\n\
             if nan == nan { return 12 }\n\
             if !(nan != nan) { return 13 }\n\
             if !(1.0 <= 1.0) || !(1.0 >= 1.0) { return 14 }\n\
             if COMPILED_SUM != 0.30000000000000004 { return 15 }\n\
             if COMPILED_NAN <= 1.0 || COMPILED_NAN >= 1.0 { return 16 }\n\
             let mixed = mixed9(\n\
                 0, 0.0, 1, 1.0, 2, 2.0, 3, 3.0, 4, 4.0,\n\
                 5, 5.0, 6, 6.0, 7, 7.0, 8, 8.0,\n\
             )\n\
             if mixed != 16.0 { return 17 }\n\
             let tiny64 = 5e-324\n\
             if !(tiny64 > 0.0) || tiny64 / 2.0 != 0.0 { return 18 }\n\
             if COMPILED_TINY != tiny64 { return 19 }\n\
             let tiny32 = 1e-45f32\n\
             if !(tiny32 > 0.0f32) || tiny32 / 2.0f32 != 0.0f32 { return 20 }\n\
             let positive_infinity = 1.0 / 0.0\n\
             let negative_infinity = 1.0 / -0.0\n\
             if !(positive_infinity > 1.7976931348623157e308) { return 21 }\n\
             if !(negative_infinity < -1.7976931348623157e308) { return 22 }\n\
             if -0.0 != 0.0 { return 23 }\n\
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
    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_native_status(image.image(), &package_root.0, "floating", 0);
}
