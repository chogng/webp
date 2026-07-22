# Locked VP8L product benchmark summary

| operation | layout | median s | MAD s | vs baseline | paired median | wall outlier rounds |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| decode | single | 3.993188 | 0.021705 | — | — | [2, 3] |
| decode | default | 5.002243 | 0.006319 | +25.269% | +25.438% | [] |
| decode | compact | 4.034269 | 0.015141 | +1.029% | +1.029% | [5] |
| decode | low-latency | 4.009531 | 0.019459 | +0.409% | -0.078% | [] |
| decode | libwebp-m6 | 5.938344 | 0.032261 | +48.712% | +48.251% | [] |

Full CPU, RSS, per-image, paired-round, MAD, and 3×MAD data are in `summary.json`.
