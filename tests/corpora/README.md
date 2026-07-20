# External corpora

The public `webp` crate tests consume downloaded corpus files directly. The
versioned smoke selection lists the inputs that a given test supports; the
fetch and verification scripts pin their source revision and file hashes before
CI invokes Rust tests. Keep fixture-specific expected results next to the
public API assertion that consumes the file.
