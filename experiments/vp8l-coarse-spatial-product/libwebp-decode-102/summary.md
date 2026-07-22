# Locked VP8L product benchmark summary

| operation | layout | median s | MAD s | vs baseline | paired median | wall outlier rounds |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| decode | libwebp-m6 | 5.965627 | 0.004489 | — | — | [4] |
| decode | default | 5.432180 | 0.007423 | -8.942% | -8.942% | [2] |
| decode | compact | 5.335206 | 0.001505 | -10.568% | -10.576% | [1, 2] |
| decode | low-latency | 5.279929 | 0.013647 | -11.494% | -11.308% | [] |

Full CPU, RSS, per-image, paired-round, MAD, and 3×MAD data are in `summary.json`.
