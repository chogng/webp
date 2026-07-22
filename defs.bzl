"""Project-level Bazel helpers for the workspace's conventional Rust crates."""

load("@crates//:defs.bzl", cargo_aliases = "aliases", "all_crate_deps")
load("@rules_rs//rs:rust_library.bzl", "rust_library")
load("@rules_rs//rs:rust_test.bzl", "rust_test")


def webp_rust_crate(
        name,
        crate_name,
        aliases = {},
        deps_extra = None,
        proc_macro_deps_extra = None,
        test_deps_extra = None,
        test_data = None,
        test_compile_data = None,
        rustc_flags = None,
        visibility = None):
    """Defines a conventional workspace Rust library and its unit-test target.

    The target name follows its package directory while `crate_name` follows
    the Rust library name. Cargo metadata supplies dependencies. Every workspace
    crate uses Rust 2024, `src/lib.rs`, public visibility, and a
    `unit_tests` target. Keep the non-default parts explicit at the call site.
    """
    if deps_extra == None:
        deps_extra = []
    if proc_macro_deps_extra == None:
        proc_macro_deps_extra = []
    if test_deps_extra == None:
        test_deps_extra = []
    if test_data == None:
        test_data = []
    if test_compile_data == None:
        test_compile_data = []
    if rustc_flags == None:
        rustc_flags = []
    if visibility == None:
        visibility = ["//visibility:public"]

    rust_library(
        name = name,
        aliases = cargo_aliases() | aliases,
        crate_name = crate_name,
        srcs = native.glob(["src/**/*.rs"]),
        crate_root = "src/lib.rs",
        edition = "2024",
        deps = all_crate_deps(normal = True) + deps_extra,
        proc_macro_deps = proc_macro_deps_extra,
        rustc_flags = rustc_flags,
        visibility = visibility,
    )

    rust_test(
        name = "unit_tests",
        crate = ":" + name,
        compile_data = test_compile_data,
        data = test_data,
        deps = all_crate_deps(normal_dev = True) + test_deps_extra,
        rustc_flags = rustc_flags,
    )
