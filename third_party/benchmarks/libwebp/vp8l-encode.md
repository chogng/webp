# VP8L encode benchmark baseline

This file contains the current accepted Rust result and the fixed pinned-libwebp
reference for one measurement contract. Raw rounds, binaries, and generated
streams are temporary and are deleted after each benchmark job.

## Measurement contract

- Contract: `vp8l-encode-e2e-preloaded-v1`
- Corpus: `9185c1492d47a129be4ce1b425f1cbfff7cc92b1c41fbd76fe7283b12e3f3e07` (41 files)
- Host: `LancedeMac-Studio.local`
- OS: `macOS-26.4.1-arm64-arm-64bit`
- CPU: `arm`
- Iterations per measured job: 5

## Fixed pinned-libwebp reference

- Commit: `733c91e461c18cf1127c9ed0a80dccbcfed599d3`
- Adapter: `6b6649d318524c8a668d325fe2838ee3338f4367f908a030155874fbceaf3e28`
- Compiler: `Apple clang version 21.0.0 (clang-2100.1.1.101)`
- Contract: lossless, exact transparent RGB, single-threaded, preloaded RGBA

| Level | Output bytes | Time / corpus |
|---:|---:|---:|
| 0 | 14,951,902 | 153.823 ms |
| 1 | 14,391,676 | 659.571 ms |
| 2 | 14,355,158 | 850.692 ms |
| 3 | 14,326,614 | 1053.801 ms |
| 4 | 14,326,574 | 1089.408 ms |
| 5 | 14,322,626 | 1353.263 ms |
| 6 | 14,321,674 | 1677.645 ms |
| 7 | 14,321,608 | 1735.002 ms |
| 8 | 14,321,962 | 3955.395 ms |
| 9 | 14,307,274 | 19664.488 ms |

## Current accepted Rust result

- Git commit: `b4fabfb3b87db5409b0c9637636f6ee0cb8cdc75`
- Source digest: `10b750c9941448b832d506a70225569633f872c42cded86a4e8c50b48099a93a`
- Dirty when measured: `true`
- Toolchain: `rustc 1.97.1 (8bab26f4f 2026-07-14)`
- Recorded: `2026-07-24T03:14:31.437741+00:00`

| Profile | Output bytes | Time / corpus |
|---|---:|---:|
| default | 17,908,560 | 210.528 ms |
| high-compression | 14,394,586 | 5005.026 ms |

## Current horizontal comparison

| Rust profile | libwebp reference | Size gap | Time gap |
|---|---:|---:|---:|
| default | level 6 | +25.045% | -87.451% |
| high-compression | level 9 | +0.610% | -74.548% |

## Machine-readable record

<!-- BEGIN VP8L ENCODE BENCHMARK DATA
{
  "contract": {
    "corpus_sha256": "9185c1492d47a129be4ce1b425f1cbfff7cc92b1c41fbd76fe7283b12e3f3e07",
    "cpu": "arm",
    "files": 41,
    "host": "LancedeMac-Studio.local",
    "iterations": 5,
    "machine": "arm64",
    "measurement_contract": "vp8l-encode-e2e-preloaded-v1",
    "os": "macOS-26.4.1-arm64-arm-64bit"
  },
  "libwebp": {
    "identity": {
      "adapter_sha256": "6b6649d318524c8a668d325fe2838ee3338f4367f908a030155874fbceaf3e28",
      "cc": "Apple clang version 21.0.0 (clang-2100.1.1.101)",
      "exact": true,
      "levels": [
        0,
        1,
        2,
        3,
        4,
        5,
        6,
        7,
        8,
        9
      ],
      "libwebp_commit": "733c91e461c18cf1127c9ed0a80dccbcfed599d3"
    },
    "recorded_at": "2026-07-24T03:13:13.758131+00:00",
    "results": [
      {
        "checksum": "74776320",
        "elapsed_ms_per_corpus": 153.82319999999999,
        "encoder": "libwebp",
        "encodes": 205,
        "exact": true,
        "files": 41,
        "iterations": 5,
        "level": 0,
        "output_bytes_per_corpus": 14951902,
        "profile": "lossless-level-0",
        "rgba_bytes_per_corpus": 22954432
      },
      {
        "checksum": "71975190",
        "elapsed_ms_per_corpus": 659.5706,
        "encoder": "libwebp",
        "encodes": 205,
        "exact": true,
        "files": 41,
        "iterations": 5,
        "level": 1,
        "output_bytes_per_corpus": 14391676,
        "profile": "lossless-level-1",
        "rgba_bytes_per_corpus": 22954432
      },
      {
        "checksum": "71792600",
        "elapsed_ms_per_corpus": 850.6918,
        "encoder": "libwebp",
        "encodes": 205,
        "exact": true,
        "files": 41,
        "iterations": 5,
        "level": 2,
        "output_bytes_per_corpus": 14355158,
        "profile": "lossless-level-2",
        "rgba_bytes_per_corpus": 22954432
      },
      {
        "checksum": "71649880",
        "elapsed_ms_per_corpus": 1053.8006,
        "encoder": "libwebp",
        "encodes": 205,
        "exact": true,
        "files": 41,
        "iterations": 5,
        "level": 3,
        "output_bytes_per_corpus": 14326614,
        "profile": "lossless-level-3",
        "rgba_bytes_per_corpus": 22954432
      },
      {
        "checksum": "71649680",
        "elapsed_ms_per_corpus": 1089.4082,
        "encoder": "libwebp",
        "encodes": 205,
        "exact": true,
        "files": 41,
        "iterations": 5,
        "level": 4,
        "output_bytes_per_corpus": 14326574,
        "profile": "lossless-level-4",
        "rgba_bytes_per_corpus": 22954432
      },
      {
        "checksum": "71629940",
        "elapsed_ms_per_corpus": 1353.2634,
        "encoder": "libwebp",
        "encodes": 205,
        "exact": true,
        "files": 41,
        "iterations": 5,
        "level": 5,
        "output_bytes_per_corpus": 14322626,
        "profile": "lossless-level-5",
        "rgba_bytes_per_corpus": 22954432
      },
      {
        "checksum": "71625180",
        "elapsed_ms_per_corpus": 1677.6448,
        "encoder": "libwebp",
        "encodes": 205,
        "exact": true,
        "files": 41,
        "iterations": 5,
        "level": 6,
        "output_bytes_per_corpus": 14321674,
        "profile": "lossless-level-6",
        "rgba_bytes_per_corpus": 22954432
      },
      {
        "checksum": "71624850",
        "elapsed_ms_per_corpus": 1735.0022000000001,
        "encoder": "libwebp",
        "encodes": 205,
        "exact": true,
        "files": 41,
        "iterations": 5,
        "level": 7,
        "output_bytes_per_corpus": 14321608,
        "profile": "lossless-level-7",
        "rgba_bytes_per_corpus": 22954432
      },
      {
        "checksum": "71626620",
        "elapsed_ms_per_corpus": 3955.3947999999996,
        "encoder": "libwebp",
        "encodes": 205,
        "exact": true,
        "files": 41,
        "iterations": 5,
        "level": 8,
        "output_bytes_per_corpus": 14321962,
        "profile": "lossless-level-8",
        "rgba_bytes_per_corpus": 22954432
      },
      {
        "checksum": "71553180",
        "elapsed_ms_per_corpus": 19664.4882,
        "encoder": "libwebp",
        "encodes": 205,
        "exact": true,
        "files": 41,
        "iterations": 5,
        "level": 9,
        "output_bytes_per_corpus": 14307274,
        "profile": "lossless-level-9",
        "rgba_bytes_per_corpus": 22954432
      }
    ]
  },
  "rust": {
    "identity": {
      "git_commit": "b4fabfb3b87db5409b0c9637636f6ee0cb8cdc75",
      "rustc": "rustc 1.97.1 (8bab26f4f 2026-07-14)",
      "source_sha256": "10b750c9941448b832d506a70225569633f872c42cded86a4e8c50b48099a93a",
      "worktree_dirty": true
    },
    "recorded_at": "2026-07-24T03:14:31.437741+00:00",
    "results": [
      {
        "checksum": "89559610",
        "elapsed_ms_per_corpus": 210.52759999999998,
        "encoder": "rust",
        "encodes": 205,
        "files": 41,
        "iterations": 5,
        "output_bytes_per_corpus": 17908560,
        "profile": "default",
        "rgba_bytes_per_corpus": 22954432
      },
      {
        "checksum": "71989740",
        "elapsed_ms_per_corpus": 5005.026400000001,
        "encoder": "rust",
        "encodes": 205,
        "files": 41,
        "iterations": 5,
        "output_bytes_per_corpus": 14394586,
        "profile": "high-compression",
        "rgba_bytes_per_corpus": 22954432
      }
    ]
  },
  "schema": 1
}
END VP8L ENCODE BENCHMARK DATA -->
