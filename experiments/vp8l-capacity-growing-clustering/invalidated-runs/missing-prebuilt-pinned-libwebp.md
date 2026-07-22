# Retained pinned-library build miss

The first attempt to compile the pinned C compare and benchmark tools expected
`/Users/lance/Desktop/libwebp/build/libwebp.a`, following the older P15 replay
script. That checkout contained no build directory, so clang exited before
producing either tool.

The pinned checkout was confirmed at `733c91e461c18cf1127c9ed0a80dccbcfed599d3`.
It was then configured and built read-only from source into
`/private/tmp/p16-libwebp-build`; the successful artifact hashes are retained in
`raw/screen-artifacts.tsv`. No source in the pinned checkout was modified.
