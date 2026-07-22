# Invalidated pinned-C invocation

The first compile command used the required source checkout
`/Users/lance/Desktop/libwebp@733c91e` but incorrectly assumed its static
archive was under that checkout's `build/` directory. Clang returned
`no such file or directory` before producing either helper. Because the shell
sequence continued to a final `wc`, its overall status was misleadingly zero;
the empty compare stdout/stderr files are retained here. No decoder ran and no
correctness or performance sample from this invocation is valid.

The corrected invocation uses the separately pinned oracle archive at
`/Users/lance/Desktop/webp/third_party/oracle/libwebp@733c91e/build/libwebp.a`.
