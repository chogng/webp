# Invalidated smoke: absent symbols treated as zero-bit codes

The first one-image Phase A smoke used checkpoint `d23c7a1e` before its
uncommitted symbol-support correction. The planner itself matched the writer,
but rate reassignment treated every `(code, width) == (0, 0)` entry as a free
symbol. In this encoder that representation means either the sole present
symbol (legal zero-bit code) or an absent symbol (not encodable). The draft
therefore collapsed the Compact proposal from 37/40 groups to one group and
reported a nonsensical refined size of 8,494,526 bytes versus B at 7,871,116.

This run is invalid for P15 because its model was not the initial groups'
actual adaptive Huffman code-length domain. The correction assigns an
infeasible cost when a block uses a symbol whose model frequency is zero,
while retaining width zero for the model's sole present symbol. A regression
test covers this distinction. The repeated smoke then retained 40 Compact
groups and improved B from 7,871,116 to 7,693,316 bytes; LowLatency retained
15 groups and improved E from 8,066,938 to 8,049,220 bytes. Neither smoke is a
headline corpus result.
