# P16 pre-screen same-binary fairness review

- Binary: `1828e721e2e4f1eb0fd72234e2a7d298af8ced367b13aec395cc66bab55fd84e`
  (2,195,264 bytes).
- Screen manifest: 41 rows, SHA-256
  `474587feabe3178268b2eab6f7a166501d8ecc3d637a76bd412f4233dfa7b913`.
- Control layouts call the unchanged latest-main/E37
  `spatial_writer::encode_profile_control_for_test` product path.
- Candidate layouts call the feature-private P16 public profile path.
- Both paths in this binary use the same `spatial_writer::Prepared` owner,
  validation, `collect_entropy_tokens`, cache-disabled fast tokenization,
  `SinglePlan`, strict single tie fallback, fast prefix, and single writer.
- Both spatial paths use the same `PackedTokenWriter`; P16 changes exact block
  frequency ownership, E/B/refined/split clustering, exact planning, and final
  selected spatial tables/map only.
- P16 serializes only the selected main token stream. Its clustering path
  retains scalar step attribution but never historical full plans; that small
  clustering-only overhead remains included in candidate screen timing/RSS.
- The benchmark runner preloads inputs, runs a warmup, records three
  forward/reverse alternating rounds, per-image samples, process CPU/RSS,
  medians, MAD, and 3xMAD flags.
- The same binary regenerated Phase A and matched 4,518 non-timing keyed fields
  against both superseded fairness runs before this screen was authorized.
