# Locked VP8L product benchmark summary

| operation | layout | median s | MAD s | vs baseline | paired median | wall outlier rounds |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| decode | single | 1.670383 | 0.010926 | — | — | [] |
| decode | compact | 1.696975 | 0.003381 | +1.592% | +1.794% | [] |
| decode | low-latency | 1.682369 | 0.007543 | +0.718% | +0.266% | [] |

Full CPU, RSS, per-image, paired-round, MAD, and 3×MAD data are in `summary.json`.
