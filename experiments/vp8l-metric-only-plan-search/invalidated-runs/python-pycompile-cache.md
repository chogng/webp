# Invalidated Python syntax-check cache attempt

Before the A-baseline runner was committed or invoked, the first
`python3 -m py_compile` syntax check tried to create its default macOS bytecode
cache below `/Users/lance/Library/Caches/com.apple.python/...` and failed with
`PermissionError: [Errno 1] Operation not permitted` under the workspace
sandbox.

No corpus build, encode, timing, or gate sample ran. The rerun sets
`PYTHONPYCACHEPREFIX` to an external `/private/tmp` directory. This is a
syntax-check environment invalidation, not experimental evidence.
