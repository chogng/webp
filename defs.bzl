"""Project-level Bazel helpers for the workspace's conventional Rust crates."""

load("//:cargo_deps.bzl", "workspace_deps")
load("@crates//:defs.bzl", "all_crate_deps", "crate_edition")
load("@rules_rust//rust:defs.bzl", "rust_library", "rust_test")


def webp_rust_crate(
        name,
        crate_name,
        aliases = {},
        deps_extra = None,
        proc_macro_deps_extra = None,
        test_deps_extra = None,
        test_data = None,
        test_compile_data = None,
        visibility = None):
    """Defines a conventional workspace Rust library and its unit-test target.

    The target name follows its package directory while `crate_name` follows
    the Rust library name. Cargo metadata supplies dependencies and the Rust
    edition. Every workspace crate uses `src/lib.rs`, public visibility, and a
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
    if visibility == None:
        visibility = ["//visibility:public"]

    rust_library(
        name = name,
        aliases = aliases,
        crate_name = crate_name,
        srcs = native.glob(["src/**/*.rs"]),
        crate_root = "src/lib.rs",
        edition = crate_edition(),
        deps = workspace_deps() + all_crate_deps(normal = True) + deps_extra,
        proc_macro_deps = all_crate_deps(proc_macro = True) + proc_macro_deps_extra,
        visibility = visibility,
    )

    rust_test(
        name = "unit_tests",
        crate = ":" + name,
        compile_data = test_compile_data,
        data = test_data,
        deps = all_crate_deps(normal_dev = True) + test_deps_extra,
        proc_macro_deps = all_crate_deps(proc_macro_dev = True),
    )
