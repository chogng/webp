# Historical regressions

Do not put a raw fuzzer artifact here. First minimize it, then run:

```sh
tools/promote-regression.sh minimized.webp issue-123-riff-overflow \
  https://example.invalid/issues/123 CC0-1.0
```

The command copies the bytes into the regression corpus. Add a direct public
API test in `webp-rs/webp` for every promoted regression; assert rejection, or
the appropriate pixel, metadata, or `ReadInfo` golden expectation for valid
images.
