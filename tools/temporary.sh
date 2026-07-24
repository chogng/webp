#!/bin/sh
# Shared, repository-local scratch helpers for one-shot tooling.

# Create a uniquely named temporary directory under the ignored repository
# scratch root. Callers install their EXIT trap immediately afterwards.
webp_mktemp_dir() {
    root=$1
    name=$2
    scratch_root="$root/target/temporary"
    mkdir -p "$scratch_root"
    mktemp -d "$scratch_root/$name.XXXXXX"
}

# Create a uniquely named temporary file under the ignored repository scratch
# root. Callers install their EXIT trap immediately afterwards.
webp_mktemp_file() {
    root=$1
    name=$2
    scratch_root="$root/target/temporary"
    mkdir -p "$scratch_root"
    mktemp "$scratch_root/$name.XXXXXX"
}

# Remove a one-shot temporary path when the caller exits or is interrupted.
# This helper deliberately owns one path per script, which matches the
# benchmark and smoke-check callers that use it.
webp_cleanup_on_exit() {
    webp_temporary_cleanup_target=$1
    webp_temporary_cleanup_root=$(dirname "$webp_temporary_cleanup_target")
    webp_temporary_target_root=$(dirname "$webp_temporary_cleanup_root")
    webp_temporary_cleanup() {
        status=$?
        trap - EXIT HUP INT TERM
        rm -rf -- "$webp_temporary_cleanup_target"
        rmdir -- "$webp_temporary_cleanup_root" 2>/dev/null || :
        rmdir -- "$webp_temporary_target_root" 2>/dev/null || :
        exit "$status"
    }
    trap webp_temporary_cleanup EXIT
    trap 'exit 129' HUP
    trap 'exit 130' INT
    trap 'exit 143' TERM
}
