# Superseded same-binary screen

This complete 41-image, three-round encode run used binary
`3061aac6b823aa0b521ec5b52876167a923d0b29b07e0eb010d1a8ac56dc4e72`.
It correctly established the LowLatency rate failure on image 008, but the
test-only generator in that binary did not materialize ordered-control streams
for the required decoder screen. Adding those layouts changed the test binary.
The encode run was therefore superseded before headline reporting so encode,
generation, Rust decode, and pinned-C decode can all use one final screen
binary. No samples or outliers were removed.
