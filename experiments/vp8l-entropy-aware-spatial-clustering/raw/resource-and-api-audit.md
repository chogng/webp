# Resource, compatibility, and surface audit

- Safe/single-threaded: no `unsafe` addition and no thread creation in the P15
  diff.
- Dependencies/toolchain: no `Cargo.toml` or `Cargo.lock` change. All normal
  commands used stable Rust 1.97.1 on `aarch64-apple-darwin`; no non-host target
  was invoked or installed.
- API and behavior: no public API, `Default`, metadata, animation, or error
  surface change. Three-archive validation proves Default full-byte identity
  for 102/102 images; existing metadata/animation/error tests pass.
- Standard compatibility: project and pinned libwebp decoders each validated
  918/918 Default/Compact/LowLatency archive streams against complete RGBA.
- Maximum 16,384² counter storage is 34,373,632 bytes for Compact
  (`16,384 * 1,049 * sizeof(u16)`) and 17,186,816 bytes for LowLatency
  (`4,096 * 1,049 * sizeof(u32)`). Even a conservative peak including six
  retained plan copies, four sets of group code tables, prefixes, maps, and
  summaries remains below 40 MiB, hence below the +64 MiB static gate.
- Screen median RSS changed by +3,260,416 bytes/+0.558% for Compact and
  -11,206,656 bytes/-1.900% for LowLatency, passing both +64 MiB and +5%.
- E37 release rlib: 462,384 bytes; P15 release rlib: 574,488 bytes
  (+112,104/+24.245%). E37 release test binary: 1,523,552 bytes; final P15
  screen test binary: 1,573,600 bytes (+50,048/+3.285%). The test binary also
  contains both controls and research-only Phase A instrumentation.
