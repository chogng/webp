# Superseded screen: missing generated control streams

`raw/screen-41-encode-valid` is a valid three-round encode measurement of test
binary `1828e721...`, but it is superseded and is not the final screen.

Review before decoder validation found that the feature-private `generate`
command wrote `default`, `single`, `compact`, and `low-latency` streams but did
not write `compact-control` and `low-latency-control`. That binary therefore
could not supply the complete same-binary control/candidate decoder screen.

The ignored product harness was extended to write both control layouts under the
P16 feature. No production path or clustering rule changed. Because that test
harness change changed the test binary, encode is repeated from the beginning
with the replacement binary before any final screen conclusion is recorded.

