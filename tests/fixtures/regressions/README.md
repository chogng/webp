# Historical regressions

Do not put a raw fuzzer artifact here. First minimize it, then run:

```sh
tools/promote-regression.sh minimized.webp issue-123-riff-overflow \
  https://example.invalid/issues/123 CC0-1.0
```

The command copies bytes, computes the fixture SHA-256, and creates the
sidecar consumed by the Rust smoke runner. It begins as `MustReject`; edit the
manifest if the regression is a valid image that needs a pixel, metadata, or
`ReadInfo` golden expectation.
