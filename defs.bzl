"""Project-level Bazel helpers for the workspace's conventional Rust crates."""

load("@rules_rust//rust:defs.bzl", "rust_library", "rust_test")


def webp_rust_crate(
        name,
        aliases = {},
        deps = None,
        proc_macro_deps = None,
        test_deps = None,
        test_data = None,
        test_compile_data = None,
        edition = "2024",
        visibility = None):
    """Defines a conventional workspace Rust library and its unit-test target.

    Every workspace crate uses `src/lib.rs`, Rust 2024, public visibility, and
    a `unit_tests` target. Keep the non-default parts explicit at the call site.
    """
    if deps == None:
        deps = []
    if proc_macro_deps == None:
        proc_macro_deps = []
    if test_deps == None:
        test_deps = []
    if test_data == None:
        test_data = []
    if test_compile_data == None:
        test_compile_data = []
    if visibility == None:
        visibility = ["//visibility:public"]

    rust_library(
        name = name,
        aliases = aliases,
        srcs = native.glob(["src/**/*.rs"]),
        crate_root = "src/lib.rs",
        edition = edition,
        deps = deps,
        proc_macro_deps = proc_macro_deps,
        visibility = visibility,
    )

    rust_test(
        name = "unit_tests",
        crate = ":" + name,
        compile_data = test_compile_data,
        data = test_data,
        deps = test_deps,
    )
