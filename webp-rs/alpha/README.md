| Encoder / iteration | Revision | Exact alpha | Whole-image median (3 x 10) ↓ | Throughput ↑ | Cost ↓ | Change from prior Rust | Time vs paired libwebp | Rust ALPH-only median ↓ | ALPH throughput ↑ | ALPH cost ↓ | ALPH change from prior |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| pinned libwebp | `733c91e` | 41/41 | **10029.278 ms** | 6.365 MPix/s | 157.110 ns/pixel | reference | reference | n/a: no public standalone ALPH encoder | n/a | n/a | n/a |
| Rust I1: latest-main code baseline | `a8a7371` (`5e54dd3` code) | 41/41 | 8058.452 ms | 7.922 MPix/s | 126.237 ns/pixel | baseline | -18.79% | 1786.003 ms | 35.742 MPix/s | 27.978 ns/pixel | baseline |
| Rust I2: batched LSB writer | `86ea22b` | 41/41 | 7122.629 ms | 8.962 MPix/s | 111.577 ns/pixel | **-11.61%** | -28.14% | 879.106 ms | 72.615 MPix/s | 13.771 ns/pixel | **-50.78%** |
| Rust I2f: ownership/filter/parser cleanup | pre-I3 checkpoint | 41/41 | 7022.180 ms | 9.091 MPix/s | 110.003 ns/pixel | -1.41% | -29.27% | 800.482 ms | 79.747 MPix/s | 12.540 ns/pixel | -8.94% |
| Rust I3: plane codes + indexed alpha | `b32d350` | **41/41** | **7019.944 ms** | **9.094 MPix/s** | **109.968 ns/pixel** | -0.03% | **-30.01%** | **796.203 ms** | **80.176 MPix/s** | **12.473 ns/pixel** | -0.53% |

| Encoder / iteration | ALPH bytes / suite ↓ | ALPH bpp ↓ | ALPH gap to libwebp | ALPH change from prior | Complete WebP bytes / suite ↓ | WebP gap to libwebp | WebP change from prior |
|---|---:|---:|---:|---:|---:|---:|---:|
| pinned libwebp | **4,098,325** | **5.1361** | reference | reference | **6,509,902** | reference | reference |
| Rust I1 | 4,135,772 | 5.1830 | +0.91% | baseline | 6,636,088 | +1.94% | baseline |
| Rust I2 | 4,135,772 | 5.1830 | +0.91% | 0.00% | 6,636,088 | +1.94% | 0.00% |
| Rust I2f | 4,135,741 | 5.1830 | +0.91% | -0.00% | 6,636,056 | +1.94% | -0.00% |
| Rust I3 | **4,118,622** | **5.1615** | **+0.50%** | -0.41% all files / **-10.98% structured** | **6,618,910** | **+1.67%** | -0.26% |

# ALPH encoder benchmark and optimization record

The opening tables are the decision ledger. Lower elapsed time, ns/pixel, and
byte counts are better; higher MPix/s is better. A standalone optimization is
called material only when it improves a primary metric by at least 10%.
Sub-10% compatible changes may be folded into an architectural iteration, but
are recorded as marginal rather than presented as wins. Regressions remain in
the table.

At the current operating point Rust uses **30.01% less whole-image time** than
libwebp, which is **42.87% higher throughput**. It is close to, but does not yet
claim, the 50% throughput target. Complete output is 1.67% larger and ALPH is
0.50% larger. There is no honest cross-library ALPH-only speed ratio because
libwebp does not expose a public standalone ALPH encoder; its public whole-image
API is the comparison boundary.

## ALPH 实验总账

这份总账以 `main` 上的本文件为唯一可见入口。独立对话和 worktree
负责隔离研究代码、原始数据和失败原型，但不能成为唯一记录位置。新实验创建后立即登记；
无论最终推广、仅保留 benchmark，还是完整回滚，都必须把分支、HEAD、基线、结果位置、
正式轮次和决定回填这里。顶部性能表只收录刷新纪录或形成明确 Pareto 的结果，失败和
未达 10% 门槛的实验仍永久留在本节。

根任务：`019f86e7-a515-7fc3-aa8a-bafb53daf279`。

| ID | 假设 / 架构 | 分支 / HEAD | latest-main base | 独立 worktree / task | 当前状态与证据 | 推广决定 | `main` 总账动作 |
|---|---|---|---|---|---|---|---|
| A00 | benchmark v3、批量 bit writer、二维距离码与低基数 color-indexing | `codex/alpha-architecture@123961f`；核心提交 `d796657` / `86ea22b` / `b32d350` | 创建于 `5e54dd3`；推广前重放到 `a8a7371` | `/private/tmp/webp-alpha-arch-5e54dd3`；根任务 | 41 文件正式 3 x 10；Rust 整图吞吐比 libwebp 高 42.87%；ALPH-only 比基线少 55.42%；structured ALPH 少 10.98%；全门禁通过 | **已推广** | 已进入顶部表、迭代日志和 `main@123961f` |
| A01 | row/RLE parser + 三档 cost planner；采样后可直接选 parser，模糊区按完整 bitstream 成本择优 | `codex/alpha-cost-planner@909fc85`；latest-main 代码 `7039e8c` / `0613c88`；`6eb4d2a` 为 O(pixels) 反优化 | 创建于 `123961f`；最终重放到 `0e2ebb4db884893568470317cb922280baa2254f` | [`a2a2` worktree](</Users/lance/.codex/worktrees/a2a2/webp>)；task `019f8768-5da4-7622-952f-6958f53ecf71` | 41 文件正式 3 x 10：structured ALPH `138,762 -> 121,624`（**-12.35%**），距 libwebp `+0.92%`；整图 `7033.002 ms`（+0.09%），ALPH-only `747.279 ms`（-6.18%）；exact oracle、workspace、clippy、fmt、Bazel 全过；报告 `909fc85:reports/alpha-cost-planner/README.md` | **benchmark-only / 不推广**；A02 证明直接 RowRLE 快路径存在灾难性误选 | 不进入顶部表；保留 9 份 raw 日志、12.35% 局部收益和 `6eb4d2a` 反优化，作为 A03 的架构输入 |
| A02 | 在可追溯真实透明图与分层 synthetic 语料上验证 A01 泛化、长尾和资源成本 | `codex/alpha-row-parser-generalization@12444f0`；candidate `b6eb728 -> 142c242`；harness `24fabe0` | 创建于 `e72ed3b`；正式测量前重放到 `0e2ebb4db884893568470317cb922280baa2254f` | [`8cdc` worktree](</Users/lance/.codex/worktrees/8cdc/webp>)；task `019f877a-a92f-7f12-bd00-9c853e7a76d8` | 4 real + 11 synthetic，5 x 5 交错：real ALPH aggregate **+438.98%**、WebP **+224.88%**；23/24 平面直接 RowRLE、0 次 Compare；Metal/icon/shadow 最坏 `+579.69% / +1347.48% / +2232.03%`；30/30 pinned-dwebp exact；报告 `12444f0:tools/alpha-generalization/REPORT.md` | **reject / generalization failure**；不合并候选代码 | 顶部表不变；manifest、runner、raw timing/RSS、逐文件/分类汇总和失败结论固化于 `12444f0` |
| A03 | fallback-safe guarded planner：探针只生成候选，greedy 永远保底；RowRLE 仅凭最终字节成本获胜 | `codex/alpha-guarded-row-planner@4f99d8d`；代码 `21e0d15` | 创建于 `0e2ebb4`；正式测量前重放到 `ea346ff50fbc03f821eecfe8cce905419c75d070` | [`84c4` worktree](</Users/lance/.codex/worktrees/84c4/webp>)；task `019f8789-204c-7c41-8dda-e591b37c8ab8` | structured ALPH **-12.351%** 且三个 A01 反例 0% 回退；但 v3 ALPH-only **+15.153%**，4-real ALPH 仅 -3.414% 且 ALPH-only p50 **+39.089%**；全门禁通过；报告 `4f99d8d:reports/alpha-guarded-row-planner/README.md` | **reject / 安全但 CPU 成本过高** | 不进入顶部表；保留 exact-selection 不变量、全部 3 x 10 / 5 x 5 / RSS / oracle raw evidence，供 A04 降低候选执行数 |
| A04 | filter-first exact portfolio：先完成全部 greedy/filter baseline，只对 top-1/top-k shortlist 运行 RowRLE | `codex/alpha-filter-first-portfolio@15e4673`；oracle tooling `fa955f5` | 创建于 `ea346ff`；headline evidence 前重放到 `6627800d4786262651dd06e81022c7df2c3c84ab` | [`a11f` worktree](</Users/lance/.codex/worktrees/a11f/webp>)；task `019f87a6-663b-79a0-b644-c30407d4c28d` | top-1 完整保留 structured **-12.351%** 且零膨胀，但 41-file RowRLE 尝试只少 45.0%，real -50.0%、synthetic -33.33%、三集合合计 -41.86%；greedy token state 无安全 skip predicate；报告 `15e4673:reports/alpha-filter-first-portfolio/README.md` | **phase-A reject / 不实现**；未过额外解析数至少减半的 gate | 顶部表不变；保留全部 oracle candidate/rank/token ownership CSV、probe、输入哈希和 staged-check 失败记录 |
| A05 | fused / compact RowRLE exact candidate：直接降低每次必要 RowRLE walk、histogram、token-cache 和 prepare 成本 | `codex/alpha-fused-row-plan`；进行中 | 创建并核验于 `11f6f669215479848628c1bdcd438c2a891e96fb`；`main` 后续含 `fb17a98c` VP8L correctness fix，headline 前必须重放最新 `main` | [`ff4d` worktree](</Users/lance/.codex/worktrees/ff4d/webp>)；task `019f87bf-7827-7ca1-8487-1d4b4436b2e9` | **旧-base diagnostic**：RowRLE construction 占 A03 新增 CPU 的 99%–136%；safe 32-byte walk 实测 1.489x–1.739x，结合 top-1 后预测 A03-relative v3/real/synthetic `-8.92% / -18.19% / -9.37%`，v3/synthetic 未过 10%；因 main 前进，所有 timing 已作废并等待完整重跑 | 未决；旧数据倾向 phase-A reject，但只能由最新-main 重跑裁决 | 已登记；保留 runner 路径/time-l 失败，最终回填最新 base、phase CSV/micro、代码/证据 HEAD 和决定 |

### A01 / A02 已完成结果明细

A01 的 41-file conformance 结果证明 row/RLE parser 本身有价值，但 A02
证明其“高结构即直接采用 RowRLE”的选择边界不安全。因此 A02 的泛化结论覆盖
A01 报告中原先的 promote 建议；A01 代码不进入 `main`，顶部 Pareto 表保持 I3。

| 指标 | 当前 Rust / A02 baseline | A01 bounded planner | 变化 | pinned libwebp / 目标 | 结论 |
|---|---:|---:|---:|---:|---|
| 41-file whole median，3 x 10 | 7026.367 ms | 7033.002 ms | +0.09% | 10037.038 ms | noise-level；Rust 吞吐高 42.71% |
| 41-file ALPH-only median，3 x 10 | 796.468 ms | 747.279 ms | -6.18% | 无公开 standalone API | 未达单项 10% 门槛 |
| 40 structured ALPH | 138,762 bytes | **121,624 bytes** | **-12.35%** | 120,521 bytes | 局部大小门槛通过，距 libwebp +0.92% |
| all-41 ALPH | 4,118,622 bytes | 4,101,484 bytes | -0.42% | 4,098,325 bytes | 随机压力图主导总量；距 libwebp +0.08% |
| all-41 complete WebP | 6,618,910 bytes | 6,601,768 bytes | -0.26% | 6,509,902 bytes | 距 libwebp +1.41% |
| A01 peak RSS | 未测 | 未测 | 未测 | 下一实验必测 | A01 报告明确保留缺口 |

| A02 泛化指标 | Baseline | Candidate | 变化 / 分布 | 决定 |
|---|---:|---:|---:|---|
| real ALPH，4 files | 105,175 bytes | 566,877 bytes | **+438.98% aggregate**；逐文件 p50 -5.24%，worst +579.69% | reject |
| real complete WebP | 205,314 bytes | 667,016 bytes | **+224.88% aggregate** | reject |
| real whole / ALPH-only p50 | 1088.662 / 165.430 ms | 1071.279 / 160.623 ms | -0.94% / -4.20% | 速度不足以抵消体积长尾 |
| synthetic ALPH，11 files | 1,269,394 bytes | 1,294,168 bytes | +1.95% aggregate；p50 -16.58%，worst **+2232.03%** | reject；不得与 real 混报 |
| synthetic whole / ALPH-only p50 | 3765.349 / 255.073 ms | 3739.977 / 208.630 ms | -0.35% / -18.06% | ALPH 速度正向，但体积不安全 |
| all-15 ALPH / WebP（诊断） | 1,374,569 / 4,193,334 bytes | 1,861,045 / 4,679,812 bytes | +35.39% / +11.60% | 不作为 real 泛化 headline |
| process peak RSS p50 | 130.08 MiB | 129.84 MiB | -0.18% | 进程级，非 allocator live bytes |
| exactness | 15/15 project；30/30 pinned `dwebp` | 全过 | random stress 正确 raw fallback | 正确性通过不等于压缩选择安全 |

### A03 已完成结果明细

A03 消除了 A02 的安全性缺陷，但也量化了严格 exact portfolio 的 CPU
上限。它证明了“无误选”与“值得推广”是两个独立门槛；正确性和体积门槛通过，性能门槛失败。

| A03 指标 | Baseline | Guarded candidate | 变化 / 对比 | 决定 |
|---|---:|---:|---:|---|
| v3 structured ALPH | 138,762 bytes | 121,624 bytes | **-12.351%**；距 libwebp +0.915% | 体积门槛通过 |
| v3 all-41 ALPH | 4,118,622 bytes | 4,101,484 bytes | -0.416%；距 libwebp +0.077% | stress plane 主导总量 |
| v3 complete WebP | 6,618,910 bytes | 6,601,768 bytes | -0.259%；距 libwebp +1.411% | 正向但不 material |
| v3 whole p50 | 7127.238 ms | 7207.560 ms | +1.127%；libwebp 9910.045 ms | 整体仍比 libwebp 快 27.27% |
| v3 ALPH-only p50 | 828.553 ms | 954.101 ms | **+15.153%** | reject CPU trade-off |
| v3 process RSS p50 | 138.672 MiB | 139.500 MiB | +0.597% | 可接受但不抵消 CPU |
| real ALPH，4 files | current main | -3.414% aggregate | p5/p50/p95/worst = -19.194/-8.763/0/0% | 未达 10% real gate |
| real whole / ALPH-only p50 | reference | +5.905% / **+39.089%** | 无文件体积回退、CPU 明显回退 | reject |
| synthetic ALPH，11 files | current main | -6.652% aggregate | p50 -16.582%，worst 0% | 与 real 分开；不得作 real headline |
| selector | 24 planes | 1 GreedyOnly；23 exact compares；13 RowRLE wins | direct RowRLE = 0；misselection = 0 | 安全不变量成立 |
| gates | release oracle 2/2、41 exact + q0/70/99、workspace、clippy、fmt、Bazel 15/15、fuzz | 全过 | default build 不启用 benchmark feature | 正确性通过，仍不推广 |

### A04 阶段 A 结果明细

A04 先做离线 oracle，再决定是否实现。它验证了 greedy-best filter /
representation 与 RowRLE winner 的重合度很高，但单靠 top-k 调度仍不足以把
A03 的双解析成本可靠减半，因此按预设 gate 停在实现之前。

| A04 oracle set | A03 RowRLE attempts | Top-1 attempts | Reduction | A03 size gain captured | Worst expansion |
|---|---:|---:|---:|---:|---:|
| 40 structured / all 41 | 20 | 11 | **45.00%** | 100%；structured **-12.351%** | 0 bytes |
| A02 real，4 files | 8 | 4 | **50.00%** | 100%；ALPH -3.414% | 0 bytes |
| A02 synthetic，11 files | 15 | 10 | **33.33%** | 99.934%；仅漏 56-byte win | 0 bytes |
| combined | 43 | 25 | **41.86%** | 近完整 | 0 bytes |
| top-2 / top-4 | 与 A03 相同 | 与 A03 相同 | 0% | 100% | 0 bytes |

Greedy 已生成 token 中的 distance-1、previous-row、other-copy、literal 和
coverage 计数无法安全区分 RowRLE winner/loser；任何计数或比例 cutoff 都会成为
语料拟合阈值。证明另一种 segmentation 不可能获胜仍需实际 walking 和 pricing，
正是 A04 试图消除的工作。因此没有实现代码，也没有冒充 A04 的 3 x 10 / 5 x 5
性能结果；这些指标明确为未测。

### 总账更新规则

1. 创建实验前先记录最新 `main` 完整 SHA；工作树就绪后再次验证 `main`、`HEAD` 和祖先关系。
2. 每个实验使用唯一 `codex/<topic>` 分支，不复用已完成实验分支，不长期停留在 detached HEAD。
3. 实验任务只在独立 worktree 提交候选与报告；根任务负责读取结果、审查门槛，并将总账更新单独提交到 `main`。
4. 正式数据必须包含三次完整轮次、全部主指标、pinned libwebp 同场结果、正确性与资源成本；未测指标明确写“未测”。
5. 低于 10%、反优化、正确性失败或被其他结果支配的方案不进入顶部表，但必须保留总账行和结论，避免重复试错。
6. 若实验期间 `main` 前进，旧测量只保留为历史诊断；候选必须重放到新的最新 `main` 并重新通过正式 benchmark 和 oracle 才能推广。

每次完成实验时按以下字段回填：

```text
Date:
Task/thread id:
Hypothesis and owned invariant:
Latest main base SHA:
Branch / final HEAD:
Worktree:
Report and raw-data paths:
Corpus identity / host / toolchain:
All raw rounds and medians:
Pinned libwebp paired result:
ALPH bytes/bpp and complete WebP bytes:
40-file structured subtotal and all-41 total:
Peak RSS / working allocations / encode phases:
Exact oracle / workspace / clippy / Bazel results:
Rejected alternatives and regressions:
Result: promote / benchmark-only / reject and roll back
Top-table action: add / replace / none
```

## Benchmark contract

- Profile: lossy VP8 RGB at quality 75 plus lossless ALPH, fast alpha-filter
  selection, alpha quality 100. Alpha is exact; RGB bitstreams are
  encoder-specific and are not claimed to be identical.
- Corpus: 41 transparent upstream files pinned through
  `tools/corpus-lock.toml` at `libwebp-test-data` revision `06ddd96e`. The matrix
  spans 16x16 through 2048x2048, 1 through 256 alpha levels, all four source
  filter labels, color-cache and transform fixtures, natural structured alpha,
  and a 2048x2048 random-alpha stress image.
- Work per run: 41 files x 10 encodes = 410 encodes and 63,836,040 timed pixels.
  One untimed inspection encode per file and compilation are excluded. Each
  table duration is the median of three fresh process runs.
- Baseline: libwebp commit
  `733c91e461c18cf1127c9ed0a80dccbcfed599d3`, built as the repository's pinned
  scalar-canonical oracle. Both public APIs receive the same decoded RGBA.
- Host: Apple arm64, Darwin 25.4.0, Rust 1.97.1, Apple clang 21.0.0. Results
  from another host belong in a separate table.
- Size accounting: `ALPH bytes` includes the one-byte ALPH header. `WebP bytes`
  is the complete RIFF file. Sizes are deterministic and shown for one suite;
  timed logs report ten-suite totals.
- Runner output: machine-readable `metadata`, `case`, `measurement`, and
  `aggregate` records. Per case it reports shape, alpha cardinality,
  transparent/translucent counts, selected ALPH method/filter, output bytes,
  ALPH bytes, bpp, and raw ratio. Per measurement it reports elapsed time,
  MPix/s, and ns/pixel.

Run one measurement from the repository root:

```sh
./tools/benchmark-alpha-encode.sh 10
```

Run the formal three-process series:

```sh
for run_id in 1 2 3; do
  ./tools/benchmark-alpha-encode.sh 10 > "/tmp/alpha-v3-${run_id}.log"
done
```

An isolated worktree may reuse the pinned corpus and oracle:

```sh
WEBP_ALPHA_BENCH_CORPUS=/path/to/libwebp-test-data \
WEBP_ALPHA_BENCH_LIBWEBP=/path/to/libwebp \
./tools/benchmark-alpha-encode.sh 10
```

## Corpus-level size detail

The random stress image is 65.7% of suite pixels and more than 96.5% of each
encoder's ALPH bytes. It intentionally checks incompressible behavior, but it
hides transform gains on useful structured alpha. Therefore both the all-41 total
and the 40-file structured subtotal are mandatory; the latter is not a
replacement or a cherry-picked headline.

| Corpus group | Files | Pixels | Alpha levels | I1 ALPH | I3 ALPH | I3 vs I1 | libwebp ALPH | I3 gap |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| 128x128, 16 levels | 8 | 131,072 | 16 | 57,032 | 52,224 | -8.43% | 52,080 | +0.28% |
| 16x16 binary fixtures | 20 | 5,120 | 2 | 1,060 | 640 | **-39.62%** | 500 | +28.00% |
| 1-15-level structured | 6 | 999,536 | 1-15 | 33,862 | 26,898 | **-20.57%** | 19,064 | +41.09% |
| higher-cardinality structured | 6 | 1,053,572 | 64-256 | 63,946 | 59,000 | -7.73% | 48,877 | +20.71% |
| 2048x2048 random stress | 1 | 4,194,304 | 256 | 3,979,872 | 3,979,860 | -0.00% | 3,977,804 | +0.05% |
| **40 structured files** | **40** | **2,189,300** | **1-256** | **155,900** | **138,762** | **-10.99%** | **120,521** | **+15.14%** |
| **all 41 files** | **41** | **6,383,604** | **1-256** | **4,135,772** | **4,118,622** | **-0.41%** | **4,098,325** | **+0.50%** |

Representative files make the direction and remaining gaps visible. Repeated
fixtures are fully included in the group totals above rather than duplicated
in this table.

| Representative input | Shape | Levels | I1 ALPH | I3 ALPH | Delta | libwebp | I3 gap |
|---|---:|---:|---:|---:|---:|---:|---:|
| `alpha_filter_0_method_0.webp` | 128x128 | 16 | 7,129 | 6,528 | -8.43% | 6,510 | +0.28% |
| `alpha_filter_1.webp` | 16x16 | 2 | 53 | 32 | **-39.62%** | 25 | +28.00% |
| `dual_transform.webp` | 100x30 | 2 | 381 | 189 | **-50.39%** | 184 | +2.72% |
| `lossless4.webp` | 256x256 | 15 | 3,801 | 3,161 | **-16.84%** | 2,648 | +19.37% |
| `lossy_alpha1.webp` | 1000x307 | 15 | 10,854 | 9,077 | **-16.37%** | 6,625 | +37.01% |
| `lossy_alpha2.webp` | 1000x307 | 10 | 10,388 | 8,545 | **-17.74%** | 6,016 | +42.04% |
| `lossy_alpha3.webp` | 1000x307 | 3 | 8,419 | 5,908 | **-29.83%** | 3,575 | +65.26% |
| `alpha_color_cache.webp` | 588x97 | 91 | 1,964 | 1,820 | -7.33% | 1,641 | +10.91% |
| `big_endian_bug_393.webp` | 256x256 | 256 | 16,801 | 16,187 | -3.65% | 16,185 | +0.01% |
| `lossless1.webp` | 1000x307 | 256 | 14,106 | 12,770 | -9.47% | 9,537 | +33.90% |
| `lossy_alpha4.webp` | 100x100 | 64 | 2,863 | 2,683 | -6.29% | 2,440 | +9.96% |
| `lossless_big_random_alpha.webp` | 2048x2048 | 256 | 3,979,872 | 3,979,860 | -0.00% | 3,977,804 | +0.05% |
| `one_color_no_palette.webp` | 100x100 | 1 | 19 | 18 | -5.26% | 16 | +12.50% |

## Iteration log

### I0 - complete literal ALPH encoder (`72c1309`)

Established validation, quality preprocessing, four filters, raw fallback,
headerless VP8L emission, RIFF integration, and pinned-`dwebp` exact decoding.
The historical nine-file v1 run emitted 3,348,150 ALPH bytes over 50 suites and
took 618.958 ms. Those numbers remain historical and are not mixed with v3.

### I1 - greedy LZ77 and adaptive Huffman (`22fb0ec`)

Added bounded greedy backward references, measured Huffman frequencies,
code-length RLE, and a bounded token cache. On the same historical v1 runner,
ALPH size fell 14.75%, while time regressed 12.95%. The size win was material
and the time trade remained explicit. This is the code baseline at `5e54dd3`
for the broader v3 table.

### Benchmark v3 - broader evidence (`d796657`)

Expanded the public comparison and exact external oracle from nine highly
duplicated inputs to all 41 transparent upstream files. Added machine metadata,
per-file content/size metrics, ns/pixel, and an isolated Rust ALPH profile. This
changes measurement coverage, not encoder output.

### I2 - batched LSB-first writes (`86ea22b`)

Replaced one-bit-at-a-time emission with bounded byte-window merges in the
shared core `BitWriter`. Output stayed byte-for-byte identical. Whole-image
time fell **11.61%** and ALPH-only time fell **50.78%**, so this is the material
speed iteration. The disproportionate ALPH result identifies bit emission as
the former alpha hot path.

### I2f - folded ownership, filtering, and parser cleanup

Borrowed quality-100 input instead of copying it, filtered by rows instead of
using per-pixel division/modulo, moved token ownership to a private bounded
LZ77 module, and sized its match table to the input. Relative to I2, whole time
fell 1.41% and ALPH-only time fell 8.94%. Neither clears the 10% rule, so these
are folded support changes rather than standalone wins.

### I3 - VP8L plane distance codes and color indexing (`b32d350`)

Added nearby two-dimensional distance codes and a row-packed VP8L
color-indexing transform for planes with at most 16 levels. Small inputs encode
both indexed and plain forms and retain the smaller result; larger low-cardinality
planes take the indexed path directly. The palette subimage and indexed entropy
stream use the existing adaptive Huffman machinery.

Against I2f, whole time improved 0.03% and ALPH-only time improved 0.53%, both
noise-level and below the threshold. Size is the accepted result: the 40-file
structured subtotal fell **10.98%**, with representative low-cardinality files
improving 16.37% to 50.39%. The all-41 ALPH total fell only 0.41% because the
incompressible random plane dominates it. All 41 outputs decoded to the exact
source alpha through pinned `dwebp`.

From the latest-main I1 code baseline through I3, whole time is down **12.89%**,
ALPH-only time is down **55.42%**, complete size is down 0.26%, and ALPH size is
down 0.41% across all files.

### I4 research - row/RLE parser and guarded selection (`909fc85` / `12444f0`)

A01 added a parser specialized for distance-1 runs and exact previous-row
matches. Its first full-plane structural planner (`6eb4d2a`) preserved the
12.40% structured-size signal but regressed formal ALPH-only time 9.01%; this
is retained as the explicit O(pixels)-planning anti-pattern. Bounding the probe
to 4,096 samples removed that timing regression. On the 41-file gate the final
candidate cut structured ALPH 12.35%, reduced ALPH-only time 6.18%, and left
whole-image time within 0.09% of baseline.

A02 then isolated the selector from the parser on a fixed 4-real/11-synthetic
generalization corpus. The structural score sent 23 of 24 planes directly to
RowRLE and never entered its exact `Compare` path. This produced 5.80x ALPH on
the real Metal image and 14.47x / 23.32x ALPH on the synthetic icon/shadow
families. The candidate was therefore rejected despite the conformance win.
The architecture lesson is now a stable invariant for A03: sampling may decide
whether a parser candidate is worth constructing, but cannot by itself select
the winning bitstream.

A03 implemented that invariant with a reusable `EntropyPlan`: it builds greedy
as the fallback, abandons mathematically dominated RowRLE walks through a
monotonic Huffman lower bound, and exact-compares every surviving candidate
using the real table headers, code widths, and extra bits. This removed every
known size regression and retained the full 12.35% structured win. It also
made the remaining cost explicit: ALPH-only regressed 15.15% on v3 and 39.09%
at real-set p50. A03 is therefore retained as a correct architectural bound,
not promoted. A04 moves candidate scheduling above individual entropy planes
to test whether a greedy-first filter shortlist can retain the size win while
avoiding most second parses.

A04's exact oracle found that top-1 filter/representation selection preserves
every structured and real RowRLE win, but only reduces 41-file attempts from
20 to 11. The combined 56-file reduction is 41.86%, below the predeclared 50%
gate, and top-2 removes no work. Existing greedy token ownership cannot prove a
different RowRLE segmentation will lose. A04 therefore stopped before runtime
implementation; the next distinct question is the cost of constructing one
required RowRLE candidate, not another shortlist threshold.

## Rejected and non-material experiments

Diagnostic probes below used the same code base and corpus stated in each row,
but not all were three-process formal runs. They are decision evidence, not
primary headline measurements.

| Probe | Evidence | Result | Decision |
|---|---|---|---|
| plane distance codes alone | 41-file structured subtotal | -5.51% ALPH | useful only when grouped with a larger transform architecture |
| plane distance codes alone | historical 128x128 fixture | 7,129 to 7,150 bytes, +0.29% | explicit local regression; do not present alone |
| four hash candidates | historical nine-file probe | 7,129 to about 7,499 bytes on the 128x128 case; ALPH time about +30.5% | rejected |
| four candidates + plane codes | historical nine-file probe | 7,129 to about 7,527 bytes; ALPH time about +44.4% | rejected |
| one-step lazy parsing | candidate-parser probe | about -0.04% from the already worse candidate result | rejected as immaterial |
| alternate Huffman heap | nine-file timing probe | no size change and about +2.2% time | rejected |
| I2f cleanup as independent win | formal v3 | -1.41% whole / -8.94% ALPH-only | retained only as folded architecture support |
| unconditional greedy vs row/RLE | A01 v3 diagnostic | structured -12.40%, but ALPH-only about +33% and whole about +5.2% | reject execution policy; retain parser signal |
| full-plane three-way planner (`6eb4d2a`) | A01 formal 3 x 10 | structured -12.40%; whole +0.68%; ALPH-only **+9.01%** | reject O(pixels) planner scan |
| bounded planner (`0613c88`) on 41-file gate | A01 formal 3 x 10 | structured **-12.35%**; whole +0.09%; ALPH-only -6.18% | benchmark-only pending generalization; superseded by A02 failure |
| bounded planner on 4 real + 11 synthetic | A02 formal 5 x 5 | real ALPH **+438.98%**, real WebP **+224.88%**, worst synthetic ALPH **+2232.03%** | reject and do not merge; direct RowRLE selector is unsafe |
| exact guarded planner (`21e0d15`) | A03 formal v3 3 x 10 + generalization 5 x 5 | structured **-12.35%**, zero size regressions; v3 ALPH-only **+15.15%**, real p50 **+39.09%** | reject promotion; retain exact-selection boundary as A04 input |
| filter-first top-1 portfolio | A04 exact oracle over 41 + 4 real + 11 synthetic | retains structured **-12.35%** and zero expansion, but parse count only -45.0% on 41 / -41.86% combined | reject at phase A; do not implement or claim runtime speed |

## Research basis and next architecture targets

- The [WebP lossless bitstream specification](https://developers.google.com/speed/webp/docs/webp_lossless_bitstream_specification)
  defines LZ77, prefix coding, color indexing, the color cache, and optional
  spatial entropy groups.
- [RFC 9649](https://www.rfc-editor.org/rfc/rfc9649.html) specifies nearby
  two-dimensional distance codes 1 through 120 and the linear fallback.
- Google's [WebP lossless and alpha study](https://developers.google.com/speed/webp/docs/webp_lossless_alpha_study)
  reports that two-dimensional locality and color caching improve density on a
  much larger translucent-image population. The 41-file conformance corpus is
  still a gate, not a substitute for a real-image dataset.
- Larmore and Hirschberg's
  [Package-Merge paper](https://ics.uci.edu/~dhirschb/pubs/LenLimHuff.pdf)
  gives an optimal length-limited prefix-code construction. It should be tried
  only after diagnostics show Huffman lengths are a material owner.
- The pinned libwebp implementation uses quality-scaled hash chains, explicit
  previous-pixel/previous-row candidates, several reference strategies, lazy
  reach decisions, plane codes, and raw fallback. These are algorithmic
  references, not module boundaries to copy.

The next accepted architecture should target at least one measurable 10% gap:

1. **Structured ALPH density:** Rust is still 15.14% above libwebp on the
   40-file structured subtotal. A costed choice among palette, color cache,
   row/RLE, and bounded multi-candidate parses is the leading target. A01/A02
   show that neither more search nor sampled match density is sufficient:
   sampling may open a candidate set, but actual Huffman-table, prefix, length,
   and distance costs must govern the winner, with greedy fallback on ties or
   incomplete evidence. A04 shows that top-1 scheduling alone removes only 45%
   of 41-file RowRLE walks, so the next parser experiment must reduce the cost
   of each required exact candidate rather than relax the evidence boundary.
2. **Real-image evidence:** add a pinned, licensed translucent PNG/WebP corpus
   with PSNR/SSIM or exact-alpha gates, alpha-cardinality buckets, p50/p95
   latency, and peak RSS. No architecture should be tuned only to conformance
   fixtures.
3. **Whole-image 50% throughput target:** current throughput is 42.87% above
   libwebp. Reaching exactly 50% requires only another 4.8% Rust time reduction,
   which is below the project's standalone significance rule. It should be
   bundled with a >=10% density, p95, memory, or broader-dataset improvement.

## Resource behavior

- Match heads scale to twice the next power of two of input samples, clamped to
  256 through 65,536 `u32` entries (1 KiB through 256 KiB).
- Inputs through 4,194,304 samples may cache one packed `u32` token per sample;
  larger inputs use a second bounded parse instead of retaining unbounded token
  state.
- Quality-100 input is borrowed. Lower qualities own their quantized plane.
- Indexed planes retain a packed row buffer at 1/2, 1/4, or 1/8 of the source
  size depending on palette cardinality, plus at most 16 palette entries.
- Peak RSS is not yet emitted by v3 and remains a required metric for the next
  real-image benchmark revision.

## Correctness and acceptance gates

Every accepted iteration must pass:

- exact Rust round-trip for every ALPH compression/filter combination;
- exact pinned-`dwebp` decode for all 41 alpha-quality-100 files;
- Rust/`dwebp` agreement for the quality 0, 70, and 99 reduction matrix;
- workspace tests, clippy with warnings denied, formatting, and Bazel tests;
- three ten-iteration v3 runs on the same pinned corpus, oracle, host, and
  release profile;
- explicit size, speed, and regression reporting, including rejected results
  when no primary metric improves by at least 10%.

Before a benchmark worktree is created, record `git rev-parse main`, refresh
the local `main` reference when needed, create the worktree from that exact
revision, and verify that the recorded `main` commit is its ancestor. This
series was created from then-latest `main`
`5e54dd37c14cc0c810d5a2283b644161ddb2a9b2`. Before promotion, `main` advanced
by the documentation-only `a8a7371a76ac829b0cf73f62b99f8c22a04c5132`;
the branch was rebased onto it and the oracle plus all three I3 timing runs were
repeated. Stale worktree measurements are not eligible for this table.
