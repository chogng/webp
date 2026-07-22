# Locked VP8L product benchmark summary

| operation | layout | median s | MAD s | vs baseline | paired median | wall outlier rounds |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| decode | single | 3.942318 | 0.021666 | — | — | [5] |
| decode | compact | 4.023177 | 0.011836 | +2.051% | +1.506% | [1] |
| decode | low-latency | 4.000413 | 0.012302 | +1.474% | +1.001% | [] |
| encode | single | 6.430381 | 0.006007 | — | — | [1, 3] |
| encode | compact | 14.668471 | 0.026747 | +128.112% | +127.640% | [] |
| encode | low-latency | 14.253173 | 0.037536 | +121.654% | +121.277% | [4] |

Full CPU, RSS, per-image, paired-round, MAD, and 3×MAD data are in `summary.json`.
