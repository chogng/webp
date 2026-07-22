# Retained py_compile cache-path failure

The first `python3 -m py_compile summarize.py` validation failed before parsing
the file because Apple's Python attempted to create its cache under
`~/Library/Caches/com.apple.python`, outside the writable worktree sandbox.

The replacement validation set `PYTHONPYCACHEPREFIX` to a task-specific
directory under `/private/tmp`; it is the applicable syntax result.
