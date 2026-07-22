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
| A05 | fused / compact RowRLE exact candidate：直接降低每次必要 RowRLE walk、histogram、token-cache 和 prepare 成本 | `codex/alpha-fused-row-plan@3cfc2bff`；测量代码 `867df167` | 创建于 `11f6f669`；两次 main 前进后最终完整重放到 `8e3c29824151bae5697405f83ee81c2fe8335b7f` | [`ff4d` worktree](</Users/lance/.codex/worktrees/ff4d/webp>)；task `019f87bf-7827-7ca1-8487-1d4b4436b2e9` | RowRLE construction 占新增 CPU `111.0% / 99.89% / 132.52%`；safe chunks 实测 `1.735x / 1.504x / 1.617x`，结合 top-1 后 A03-relative 上限仅 `8.953% / 18.401% / 9.503%`；structured -12.350644%、最坏膨胀 0；报告 `3cfc2bff:reports/alpha-fused-row-plan/README.md` | **phase-A reject / 不实现**；v3、synthetic 未过 10% 现实上限 | 不进入顶部表；301 个 evidence 文件含最新 headline、两套 stale diagnostic、失败/中断日志、phase/micro CSV 与 SHA-256 |
| A06 | ALPH spatial entropy groups：不增加第二套 LZ parse，以标准 meta-Huffman map 聚类局部 green/distance 统计 | `codex/alpha-spatial-entropy-groups@103a277b`；analyzer `39d82d74` | 创建于 `39feb8dd`；正式测量前两次重放，最终基线 `f1ce6065cbbd11661561956c0d982e0a4cfddc27` | [`d454` worktree](</Users/lance/.codex/worktrees/d454/webp>)；task `019f87ea-a348-7062-833f-e32f04732803` | structured `138,762 -> 138,754`（**-0.005765%**），all-41 -0.080002%；real -22.120276% 但 99.3% 收益来自单图；56/56 project + pinned `dwebp` exact，零膨胀，最大 49 groups；报告 `103a277b:reports/alpha-spatial-entropy-groups/README.md` | **phase-A reject / 不实现**；完整流未达 10%，不进入 production/decode timing | 顶部表不变；保留 exact analyzer、逐 tile/component/per-file raw、SHA-256 与全部门禁日志 |
| A07 | ALPH exact color cache：复用既有 greedy token 流，对标准 cache bits 1..11 做逐像素状态模拟与完整 bitstream 计价 | `codex/alpha-exact-color-cache@2f731fa8`；analyzer `b9940ff8` | 创建于 `5362912a`；三次主线前进后最终完整重跑到 `d70b0cbe42467f3942e26eee11546cecdd60a39a` | [`40fa` worktree](</Users/lance/.codex/worktrees/40fa/webp>)；task `019f8825-6465-74c3-a880-ea07b1870c26` | structured/all-41/real 均 **0.000000%**、0 cache winner；仅 synthetic `shadow-soft` -6.393284%，synthetic aggregate -0.015598%；56/56 baseline/project/`dwebp` exact，零膨胀；报告 `2f731fa8:reports/alpha-exact-color-cache/README.md` | **phase-A reject / 不实现**；96.66% hit 仍反优化，未过 10% | 顶部表不变；保留逐 bits/hit/component/owner raw、三次 rebase 历史、22/22 SHA-256 清单 |
| A08 | optimal length-limited Huffman：量化当前超长树触发整树 `balanced_lengths` 的损失，以 package-merge 精确替代并按完整 payload 选择 | `codex/alpha-length-limited-huffman@5aa6a618`；analyzer `7dfa3ddc`；owner fixes `87595158` / `40822cbd` | 创建于 `8574b4ef`；登记和 A07 收口后最终完整重跑到 `e7891b27484ec0f66e86b46a2b1e9c8b981e77e5` | [`e754` worktree](</Users/lance/.codex/worktrees/e754/webp>)；task `019f8836-b206-7750-9b7e-8de995eeba06` | structured `138,762 -> 138,648`（**-0.082155%**）；real -19.621583%，但由 `metal-raytracing` 单图 -25.707018% 主导；56/56 baseline/project exact、112/112 `dwebp`、零膨胀；报告 `5aa6a618:reports/alpha-length-limited-huffman/README.md` | **phase-A reject / 不实现**；structured 未达 10%，不进入 production/decode timing | 顶部表不变；保留 package-merge/brute-force oracle、limit=15/7 owner waterfall、31 项 SHA 与 diagnostic 中断数据 |
| A09 | ALPH palette co-occurrence ordering：在 <=16 色 palette 上重新分配 index，精确量化 palette delta、packed-symbol table RLE、hash-collision parse 与 partial rows | `codex/alpha-palette-cooccurrence`；进行中 | worktree 创建目标为当时最新 `6f2e07fb0a82c2f52334825feb8a5d6ffc83184a`；正式数据前须重放到本登记提交后的最新 `main` | [`dda9` worktree](</Users/lance/.codex/worktrees/dda9/webp>)；task `019f8858-35db-7191-8d13-76cffe852420` | **Phase A**：先证明 permutation 双射下不变/可变 owner；<=8 色穷举实际 bytes，9..16 色以 Modified-Zeng 与固定有界实际字节搜索评估；分 40 structured/all-41/4 real/11 synthetic | 未决；structured 净 ALPH 必须 >=10%，56-file exact fallback 零膨胀；过门后才实现 | 已登记；不得把 co-occurrence proxy 或未穷举 9..16 候选称为全局最优 |

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

### A05 阶段 A 结果明细

A05 证明热点归因成立，但现实可实现上限不成立。下面均来自最终
`main@8e3c2982` 的五轮同二进制 Phase A；两套旧-main 完整数据只留在 diagnostic
目录，不进入 headline。

| Set | Baseline ALPH-only p50 | A03 p50 | A03 delta | Row construction share | Safe-chunk walk speedup | Top-1 + chunks A05/A03 ceiling |
|---|---:|---:|---:|---:|---:|---:|
| v3 41 | 413.797 ms | 480.231 ms | +15.198% | **111.000%** | **1.735x** | **8.953%** |
| real 4 | 165.627 ms | 228.817 ms | +38.377% | **99.890%** | **1.504x** | **18.401%** |
| synthetic 11 | 263.711 ms | 304.092 ms | +15.407% | **132.524%** | **1.617x** | **9.503%** |

| A05 preserved output | Baseline | Exact-safe result | Delta / bound |
|---|---:|---:|---:|
| v3 structured ALPH | 138,762 bytes | 121,624 bytes | **-12.350644%** |
| v3 all-41 ALPH | 4,118,622 bytes | 4,101,484 bytes | -0.416110% |
| real ALPH | 105,175 bytes | 101,584 bytes | -3.414309% |
| synthetic ALPH | 1,269,394 bytes | 1,184,957 bytes | -6.651757% |
| worst per-file expansion | 0 bytes | 0 bytes | exact fallback retained |

Workspace tests、feature tests、clippy、fmt 和 fuzz check 通过。Phase A 已拒绝，
所以 production compact/span/scratch/filter-fusion、专门 release oracle、Bazel 与
Phase C 3 x 10 / 5 x 5 均明确未运行；不得引用 A03 的门禁冒充 A05 新结果。

### A06 阶段 A 结果明细

A06 在现有 greedy token 上逐比特复现 single-group 基线，并用真实 canonical
Huffman writer 枚举 4/8/16/32/64/128 像素 tile。每个完整候选均计入五棵表、
nested group-map、map tables/data、transform/palette prefix、length/distance extra
bits、padding 与 ALPH header；只有最终完整 payload 严格更小时才替换基线。

| A06 set | Files | Baseline ALPH | Exact fallback | Aggregate | Per-file p50 / p95 / worst | Grouped files / max groups |
|---|---:|---:|---:|---:|---:|---:|
| 40 structured | 40 | 138,762 B | 138,754 B | **-0.005765%** | 0 / 0 / 0% | 1 / 3 |
| all 41 | 41 | 4,118,622 B | 4,115,327 B | -0.080002% | 0 / 0 / 0% | 2 / 49 |
| A02 real | 4 | 105,175 B | 81,910 B | **-22.120276%** | -1.260954 / -0.028173 / 0% | 3 / 28 |
| A02 synthetic | 11 | 1,269,394 B | 1,234,432 B | -2.754228% | -13.755247 / 0 / 0% | 7 / 19 |

real 聚合看似显著，但其中 99.3% 的节省来自 `metal-raytracing.webp`
（80,196 -> 57,086，-28.816899%）；它不能替代 structured gate。all-41 的主要
额外收益来自 random 压力图，亦未混入 40-file 结论。完整 structured 诊断进一步
排除了聚类启发式偶然失误：

| Tile | Optimistic payload floor | Independent complete stream | Clustered complete stream | Exact fallback |
|---:|---:|---:|---:|---:|
| 4 px | -44.576055% | +226.902178% | +6.338911% | 0% |
| 8 px | -30.120098% | +112.943745% | +1.867947% | 0% |
| 16 px | -19.481378% | +48.790735% | +0.582292% | 0% |
| 32 px | -12.227951% | +19.380666% | +0.521757% | 0% |
| 64 px | -7.205863% | +6.738877% | +0.234935% | **-0.005765%** |
| 128 px | -4.425293% | +2.901371% | +0.224845% | 0% |

56/56 modeled baselines 与当前 encoder 字节完全一致；56/56 候选同时通过项目
decoder 和 pinned `dwebp`，baseline/candidate PAM 逐字节一致，最坏膨胀为 0。
Workspace、Clippy、fmt、Bazel 15/15、fuzz build 与 raw SHA-256 均通过。由于
structured 完整流只节省 8 字节，Phase B、默认 encoder 改动和 production decode
timing 按预设门禁未启动。

### A07 阶段 A 结果明细

A07 在既有 greedy segmentation 上只改写当时实际命中的 literal；copy span、
length/distance 与 extra bits 完全不变。标准 cache bits 1..11 均用完整
`0xff00GG00` 像素哈希，并实际写出 cache header、扩展 green alphabet、真实
code-length RLE、五表、token、padding、ALPH header 和 raw fallback。

| A07 set | Files | Baseline ALPH | Exact fallback | Aggregate | Per-file p50 / p95 / worst | Cache winners |
|---|---:|---:|---:|---:|---:|---:|
| 40 structured | 40 | 138,762 B | 138,762 B | **0.000000%** | 0 / 0 / 0% | 0 |
| all 41 | 41 | 4,118,622 B | 4,118,622 B | **0.000000%** | 0 / 0 / 0% | 0 |
| A02 real | 4 | 105,175 B | 105,175 B | **0.000000%** | 0 / 0 / 0% | 0 |
| A02 synthetic | 11 | 1,269,394 B | 1,269,196 B | -0.015598% | 0 / 0 / 0% | 1 |

唯一 winner 是 synthetic `shadow-soft.webp`：3,097 -> 2,899（-6.393284%，
bits=1，590/1,185 literal hits）。命名 fixture `alpha_color_cache.webp` 仍为
1,820 字节；其 hit rate 从 15.60% 升到约 89.93%，完整候选却为 1,823–2,057
字节。因此 hit rate 不是可靠的压缩收益代理。

| Structured cache bits | Candidate delta | Hits / literals | Hit rate |
|---:|---:|---:|---:|
| 1 | +0.665168% | 4,552 / 91,881 | 4.9542% |
| 4 | +2.902812% | 25,213 / 91,881 | 27.4409% |
| 6 | +3.980196% | 54,825 / 91,881 | 59.6696% |
| 8 | +2.565544% | 85,130 / 91,881 | 92.6525% |
| 10 | +3.226388% | 88,813 / 91,881 | **96.6609%** |
| 11 | +3.758954% | 88,813 / 91,881 | **96.6609%** |

Owner diagnosis 在去掉全部 header/table 成本后仍显示 structured symbol+extra
floor **+0.616421%**；恢复完整表后为 +0.650755%，exact fallback 才回到 0。
主要损失不是 5-bit 声明，而是把本来已拥有短码的低基数 green literal 频率
分散到多个 cache index；扩展 alphabet 与 table RLE 只是进一步恶化。

56/56 current baseline 逐字节一致，56/56 project decoder 与 pinned `dwebp`
完整 WebP/PAM exact，最坏膨胀 0。最大 cache 8 KiB，最大模型 working bound
24.97 MiB；analyzer RSS 为 v3/real/synthetic 163.42/82.75/52.66 MiB。
Feature tests 22/22、workspace、Clippy、fmt、Bazel 15/15、fuzz build 与
SHA-256 22/22 全过。Phase A 已拒绝，所以 production q0/70/99、3 x 10 /
5 x 5 和 cache/no-cache decode timing 明确未运行。

### A08 阶段 A 结果明细

A08 验证了当前实现的真实退化边界：unconstrained Huffman 只要有一个 leaf
超过主表 limit=15 或嵌套 code-length 表 limit=7，就丢弃整棵树并改用近均衡
长度。独立 package-merge solver 以 `4^6 = 4,096` 个小 alphabet 穷举成本、
Kraft equality、长度上限与确定性 tie-break 为 oracle，再实际写出完整 ALPH。

| A08 set | Baseline ALPH | Raw package | Exact fallback | Aggregate | p5 / p50 / p95 / worst | Winners |
|---|---:|---:|---:|---:|---:|---:|
| 40 structured | 138,762 B | 138,648 B | 138,648 B | **-0.082155%** | -0.148786 / 0 / 0 / 0% | 16 |
| all 41 | 4,118,622 B | 4,118,507 B | 4,118,507 B | -0.002792% | -0.148786 / 0 / 0 / 0% | 17 |
| A02 real | 105,175 B | 84,538 B | 84,538 B | **-19.621583%** | -21.886338 / -0.126619 / -0.010215 / -0.008944% | 4 |
| A02 synthetic | 1,269,394 B | 1,269,362 B | 1,269,361 B | -0.002600% | -0.796820 / 0 / 0 / 0% | 4 |

所有枚举路径中，over-limit owner 仅有：v3 的 9/86 个 main green
code-length(limit=7) 表、real 的 2/9 个 main green(limit=15) 与 2/9 个其
code-length 表、synthetic 的 1/24 个 code-length 表；distance、red、blue、
alpha 和 palette subimage 均无触发。

| Set | Selected triggers | Current -> package weighted | Header delta | Main-symbol delta | Exact ALPH |
|---|---:|---:|---:|---:|---:|
| 40 structured | 5 | -653 bits | -864 bits | 0 bits | -114 B |
| all 41 | 5 | -653 bits | -871 bits | 0 bits | -115 B |
| A02 real | 2 | -165,367 bits | +116 bits | -165,216 bits | -20,637 B |
| A02 synthetic | 1 | -173 bits | -274 bits | 0 bits | -33 B after fallback |

Structured 收益几乎全是 table-description 清理，main symbol bits 完全不变。
相反，`metal-raytracing.webp` 的 main green unconstrained depth=16；当前整树
balanced penalty 为 165,225 bits，而 package 解仅比 unconstrained lower bound
多 9 bits，使单文件 80,196 -> 59,580（-25.707018%）。它是有价值的真实 owner，
但不能替代预设 structured gate。

56/56 modeled baseline byte-exact、56/56 project exact、pinned `dwebp`
112/112；最坏膨胀 0。Package-merge 为 `O(nL)`，最大 280×15 scratch 保守低于
300 KiB。Feature tests 23/23、workspace、Clippy、fmt、Bazel 15/15、fuzz check
与 31 项 SHA 全过。Phase A 未过，因此默认 encoder、q0/70/99、3 x 10 /
5 x 5 与 decode-table timing 均未运行。

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

A05 measured that cost directly. Row construction owns essentially all of
A03's positive CPU delta, but safe 32-byte slice equality speeds the complete
walk by only 1.50x to 1.74x on real encoder planes. Even when combined with
A04 top-1 scheduling, the projected A03-relative gain is 8.95% on v3 and 9.50%
on synthetic, below the 10% gate. Compact spans, scratch reuse, and filter
fusion were therefore not implemented after the ceiling failed. The A03-A05
line now defines a measured Pareto bound: exact RowRLE closes the structured
gap but does not meet this project's CPU/materiality policy.

A06 then tested a separate standard density mechanism without changing the
greedy parse. Exact spatial entropy groups were safe and materially useful on
one large real screenshot mask, but the 40 structured files saved only eight
bytes. Independent per-tile streams still expanded 2.90% at 128 pixels and
6.74% at 64 pixels; finer tiles expanded much more. Thus local symbol
distributions exist, but five tables per group plus the nested group map cost
more than they save on the structured target. The mechanism is rejected for
ALPH rather than retuned with corpus-specific clustering thresholds.

A07 exhausted the remaining standard color-cache exponent space without a
second LZ parse. Structured cache hits reached 96.66%, but every complete
cache-bearing structured stream expanded; even the optimistic symbol-only
floor was 0.62% worse. Repeated low-cardinality green literals already have
short Huffman codes, while cache indices fragment their frequency mass. Exact
fallback preserved current bytes, but there is no hidden cache-density win to
productize or recover with a different hit-rate threshold.

A08 replaced the current whole-tree balanced fallback with an independently
verified optimal length-limited solution. It exposed a real 25.71% single-file
win where a green tree reaches depth 16, but structured only triggers five
limit-7 code-length fallbacks and saves 114 bytes. Package-merge is therefore a
valid future option if broader real evidence shows repeated deep main trees,
not a material owner for the current structured gate.

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
| fused/compact RowRLE ceiling | A05 five-round phase attribution + real-plane micro | Row owns 99.89%–132.52% of added CPU, but top-1 + safe chunks reaches only 8.95% v3 / 9.50% synthetic | reject at phase A; do not implement multi-surface production candidate |
| ALPH spatial entropy groups | A06 exact analyzer over 40 structured + random + 4 real + 11 synthetic | structured **-0.005765%** after fallback; independent complete streams expand +2.90% to +226.90%; 56/56 exact and zero expansion | reject at phase A; local Huffman gains cannot repay real group tables/map |
| ALPH exact color cache | A07 actual serialization for bits 1..11 over 40 structured + random + 4 real + 11 synthetic | structured/all-41/real **0%** after fallback; 96.66% hit still expands; only one synthetic file -6.39% | reject at phase A; cache symbols fragment already-cheap green literals |
| optimal length-limited Huffman | A08 package-merge + brute-force oracle over every real table owner | structured **-0.082155%**; real -19.621583% from one depth-16 green owner; 56/56 exact and zero expansion | reject at phase A; retain real-image owner evidence, no default solver |

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
   of 41-file RowRLE walks, and A05 shows that realistic per-walk acceleration
   still misses 10%. A06 has now ruled out spatial entropy groups on this
   structured target: even its exact independent complete streams cannot repay
   table/map cost. A07 has also ruled out the remaining standard color-cache
   mechanism: its optimistic payload loses before header cost even with 96.66%
   hits. A08 therefore moves below parser/format choices to the Huffman owner:
   it will measure whether the current whole-tree balanced fallback on an
   over-limit code discards material frequency information, then price optimal
   package-merge lengths with the real table RLE and complete payload. A08
   found only -0.082% structured, so the next independent standard-density
   target is palette-index assignment: current ALPH orders <=16 values only by
   numeric value, while a 4-neighbour co-occurrence ordering can change packed
   index bytes and LZ/Huffman density without changing the decoded palette.
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
