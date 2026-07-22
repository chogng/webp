# Invalidated summary invocation: Python compatibility

The first post-Phase-A summary invocation failed before creating any summary
because the installed `python3` does not support `zip(..., strict=True)`:

```text
TypeError: zip() takes no keyword arguments
```

The locked Phase A binary had already exited zero and its raw TSV/stderr were
not changed. The summarizer was corrected to compare field counts explicitly
before ordinary `zip`, preserving the intended truncation check without
changing any codec rule, stream, or rate result.
