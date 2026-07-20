# Fixture manifests

Each `*.toml` file describes exactly one fixture.  Paths are relative to this
directory; fixture bytes normally live under `../fixtures/{smoke,regressions}`.
Run `webp_testkit::FixtureRunner` with `tests/` as its corpus root.  A manifest
path is relative to its sidecar and may use `..` for the sibling fixture
directory; the runner canonicalizes the result and rejects any path that
escapes the corpus root.

Required keys are `id`, `file`, `sha256`, `class`, `source`, `license`, and
`codec`.  `MustAccept` and `CompatAccept` entries additionally require both
`expected_width` and `expected_height`.  Optional expected output hashes are
lower-case SHA-256 digests of canonical straight RGBA8 bytes.

Keep smoke files small and committed.  A discovered bug belongs in
`tests/fixtures/regressions/` with its own manifest, source/provenance, and
the smallest reproducer that still tests the failure.
