# Python bytecode-cache permission invocation

`python3 -m py_compile experiments/vp8l-profile-hybrid-clustering/summarize.py`
first failed because the host Python attempted to create its bytecode cache
under `/Users/lance/Library/Caches`, which is outside the worktree sandbox.
The summarizer itself had already executed successfully. The syntax check was
rerun with `PYTHONPYCACHEPREFIX=/private/tmp/p18-pycache` and passed. No result
or algorithm was changed.
