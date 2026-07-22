# VP8L 性能总账

| 纪录类别 | 实现 / profile | 图 / 流 | 解码线程 | 输入或容器 bytes | 输出 RGBA bytes | 中位时间 | 输入 MB/s | RGBA MB/s | MP/s | 相对 pinned libwebp | 相对 m6 体积 | 正确性 | 可追溯位置 |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | --- |
| libwebp 基准 | pinned libwebp m0 | 102 / 102 | 1 | 290,266,556 | 1,007,432,548 | 4,776 ms | 60.8 | 210.9 | 52.7 | 基准 | +9.526% | 102/102 | `733c91e`；[quality gates](../../docs/quality-gates.md) |
| libwebp 基准 | pinned libwebp m3 | 102 / 102 | 1 | 267,917,268 | 1,007,432,548 | 4,881 ms | 54.9 | 206.4 | 51.6 | 基准 | +1.093% | 102/102 | `733c91e`；[quality gates](../../docs/quality-gates.md) |
| libwebp 基准 | pinned libwebp m6 | 102 / 102 | 1 | 265,020,980 | 1,007,432,548 | 4,777 ms | 55.5 | 210.9 | 52.7 | 基准 | 基准 | 102/102 | `733c91e`；[quality gates](../../docs/quality-gates.md) |
| libwebp 基准 | pinned libwebp m0+m3+m6 | 102 / 306 | 1 | 823,204,804 | 3,022,297,644 | 14,363 ms | 57.3 | 210.4 | 52.6 | 基准 | 三种标准流合计 | 306/306 | `733c91e`；[backend record](</Users/lance/.codex/worktrees/4c95/webp/tools/vp8l-backend-bakeoff/RESULTS.md>) |
| 标准 VP8L 纪录 | 当前 Rust，m0+m3+m6 | 102 / 306 | 1 | 823,204,804 | 3,022,297,644 | 14,009 ms | 58.8 | 215.7 | 53.9 | **快 2.5%**，同输入同轮次 | 相同 | 306/306 | main lineage；最初记录于 `eca32b4` |
| 标准 VP8L 自编码流纪录 | Rust `fast_no_cache` | 102 / 102 | 1 | 724,306,686 | 1,007,432,548 | 2,613 ms | 277.2 | 385.5 | 96.4 | 约快 45.3%†，相对 m6 C 基准 | +173.302% | 102/102，两套 decoder | `codex/vp8l-fast-decode-profile@232a32c`；[report](</Users/lance/.codex/worktrees/c68f/webp/docs/vp8l-fast-decode-research.md>) |
| 标准 VP8L 自编码 Pareto | Rust `FastDecodeCompact` | 102 / 102 | 1 | 617,958,802 | 1,007,432,548 | 4,034 ms | 153.2 | 249.7 | 62.4 | 同流比 pinned C 快 24.4%†；相对 m6 C 快 32.4%† | +133.174% | E37 产品流 306/306 byte identity；E33 两套 decoder 408/408；Default 不变 | decode `9776da40`；encode `b3b96fdc`；[decode report](../../experiments/vp8l-coarse-spatial-product/REPORT.md)；[encode report](../../experiments/vp8l-packed-writer-product/REPORT.md) |
| 标准 VP8L 自编码 Pareto | Rust `FastDecodeLowLatency` | 102 / 102 | 1 | 625,321,072 | 1,007,432,548 | 4,010 ms | 156.0 | 251.3 | 62.8 | 同流比 pinned C 快 24.1%†；相对 m6 C 快 32.8%† | +135.952% | E37 产品流 306/306 byte identity；E33 两套 decoder 408/408；Default 不变 | decode `9776da40`；encode `b3b96fdc`；[decode report](../../experiments/vp8l-coarse-spatial-product/REPORT.md)；[encode report](../../experiments/vp8l-packed-writer-product/REPORT.md) |
| 私有兼容表示实用档纪录 | FDEC Zstd-1 / RGB / Row-Sub，融合输出 | 102 / 102 | 1 | 663,622,132 | 1,007,432,548 | 923.689 ms | 718.4 | 1,090.7 | 272.7 | 约快 80.7%†；同轮 Rust m6 快 81.8% | +150.404% | 102/102；libwebp fallback 102/102 | `codex/fdec-hot-path-migration@ba4b530`；[report](</Users/lance/.codex/worktrees/a386/webp/docs/fdec-hot-path-migration.md>) |
| 私有兼容表示极速档纪录 | FDEC LZ4 / RGB / none，融合输出 | 102 / 102 | 1 | 935,997,910 | 1,007,432,548 | **416.581 ms** | 2,246.9 | 2,418.3 | 604.6 | 约快 91.3%†；同轮 Rust m6 快 91.8% | +253.179% | 102/102；libwebp fallback 102/102 | `codex/fdec-hot-path-migration@ba4b530`；[report](</Users/lance/.codex/worktrees/a386/webp/docs/fdec-hot-path-migration.md>) |
| 单图流水线纪录 | entropy producer + transform consumer | 102 / 306 | 2 | 823,204,804 | 3,022,297,644 | 9,375 ms | 87.8 | 322.4 | 80.6 | 快 34.7% | 相同 | 306/306 | `codex/vp8l-single-image-pipeline@66356c6` |
| 批量吞吐纪录 | 当前 Rust，jobs=12 | 102 / 306 | 12 | 823,204,804 | 3,022,297,644 | **2,842.808 ms** | 289.6 | 1,063.1 | 265.8 | 快 80.2%；但不是单图 latency | 相同 | 306/306 | `codex/vp8l-batch-parallel-ab@664d142`；[report](</Users/lance/.codex/worktrees/ffb9/webp/docs/vp8l-batch-parallel-benchmark.md>) |

顶部表只保留 pinned 基准，以及在自己的可比类别中刷新时间纪录或形成明确速度/体积 Pareto 的结果。被后续结果完全支配、仅改善内存但降低速度、未通过正确性、或只产生诊断信息的实验只进入下方实验账本。

`MB/s` 使用十进制 MB，所有主解码时间均排除文件读取和进程启动，输入先载入内存，输出 RGBA 完整分配、写出并参与校验。`MP/s` 按 RGBA 像素数计算。标有 `†` 的 Rust/C 比较使用不同语言的独立锁定 runner，不能称为同一 binary A/B：旧 FDEC/`fast_no_cache` 行使用历史 pinned C 固定参考；两个 coarse 产品档使用本次同语料、同规则、五轮的 pinned C 实测。FDEC 的 `306` 结果若出现，是同一 102 图 profile 重复三次的等价投影，不应写成 306 个不同码流。

### 纪录的资源与产品成本

| Profile | Encode / append | 标准 fallback | 私有 payload | 最大 decode working peak | 已观测进程峰值 / 增量 | 依赖或二进制成本 | 完整附加 I/O break-even | Alpha 加速覆盖 |
| --- | ---: | ---: | ---: | ---: | --- | --- | ---: | ---: |
| pinned libwebp m6 | 未测 | n/a | n/a | 未分离 | aggregate live-allocation 下界 835,656,644 B | pinned C static library | 基准 | 由标准 VP8L 覆盖 |
| Rust `fast_no_cache` | 未保留可比 encode 计时 | n/a | n/a | 未分离 | 799,277,056 B RSS | 默认 safe Rust workspace | 未实测 | 本轮 CLIC 为 opaque |
| `FastDecodeCompact` | **7,874.026 ms**；相对 same-binary latest-main writer control -27.005% | 680,790,322 B single；精确计价后逐图严格回退 | n/a；普通标准 VP8L | 未分离 | 正式 encode 进程 1,143.25 MiB；control 1,215.27 MiB | safe Rust；无新增依赖/线程/unsafe；release rlib +17,504 B / +4.011%；main `b3b96fdc` | n/a | 标准 VP8L；latest-main/E36 各 306/306 byte identity；双 decoder exact |
| `FastDecodeLowLatency` | **7,638.855 ms**；相对 same-binary latest-main writer control -26.561% | 680,790,322 B single；精确计价后逐图严格回退 | n/a；普通标准 VP8L | 未分离 | 正式 encode 进程 1,153.55 MiB；control 1,215.53 MiB | safe Rust；无新增依赖/线程/unsafe；release rlib +17,504 B / +4.011%；main `b3b96fdc` | n/a | 标准 VP8L；latest-main/E36 各 306/306 byte identity；双 decoder exact |
| FDEC Zstd-Sub fused | 2,176.514 ms | 265,020,980 B | 398,596,613 B compressed；398,601,152 B complete chunk | 21,790,720 B | 旧 harness RSS 718,323,712 B | `zstd-sys` C/FFI；整个 research feature 令最新 release binary +280,768 B / +44.54% | 136.9 MB/s | promoted RGB：0/28 alpha；RGBA screen 待新协议 |
| FDEC LZ4 fused | 1,210.423 ms | 265,020,980 B | 670,972,393 B compressed；670,976,930 B complete chunk | **13,238,272 B** | 旧 harness RSS 988,921,856 B | `lz4_flex` pure safe Rust；与 Zstd 合并 feature 的 binary 成本同上 | 162.3 MB/s | promoted RGB：0/28 alpha；RGBA screen 待新协议 |
| 单图流水线 | n/a | n/a | n/a | 原 residual history + 最多约 792 KiB | 未保留统一 RSS | safe Rust；固定 1 个 consumer thread | n/a | 标准 decoder 覆盖 |
| batch jobs=12 | n/a | n/a | n/a | 每个 worker 正常单图工作集 | 1.50 GiB；比 jobs=1 多约 0.66 GiB | 最多 11 个额外 worker；CPU time 14.42 -> 16.73 s | n/a | 标准 decoder 覆盖 |

## Pinned libwebp 基准身份

| 指标 | 固定值 / 规则 |
| --- | --- |
| Oracle commit | `733c91e461c18cf1127c9ed0a80dccbcfed599d3` |
| API | 静态链接 pinned `libwebp.a`，调用 `WebPDecodeRGBA` |
| Corpus | `tfds:clic:1.0.0` validation，102 张源 PNG |
| Manifest SHA-256 | `6faf7f5eef4235c69de45a292dc6c68fc0831830b7e4e4516b5f058a6037f13a` |
| 标准流 | pinned `cwebp -lossless -exact` 的 method 0、3、6，共 306 个 VP8L |
| m0 / m3 / m6 bytes | 290,266,556 / 267,917,268 / 265,020,980 |
| 总压缩输入 | 823,204,804 bytes |
| 每个 method 输出 | 251,858,137 pixels；1,007,432,548 RGBA bytes；checksum `332352` |
| Aggregate 输出 | 755,574,411 pixels；3,022,297,644 RGBA bytes；checksum `997056` |
| libwebp m0 / m3 / m6 | 4.776 / 4.881 / 4.777 s |
| libwebp aggregate | 14.363 s；约 210.4 MB RGBA/s；52.6 MP/s |
| 同轮 Rust aggregate | 14.009 s；比 libwebp 快 2.5%；比最初 Rust 20.863 s 快 32.9% |
| 主机 | Apple M2 Max，arm64，12 cores，32 GiB，macOS 26.4.1 |
| Rust 工具链 | stable Rust 1.97.1；普通验证不使用 nightly |
| 测量规则 | release、单线程、输入预载、完整 RGBA materialization、三次完整 corpus 中位数 |
| Aggregate live-allocation 下界 | 835,656,644 bytes：823,204,804 bytes 预载输入 + 12,451,840 bytes 最大 RGBA 输出；不含后端私有 scratch 与 allocator overhead |
| 标准命令 | `bash tools/benchmark-vp8l-clic.sh 1 4`；以后扩展脚本时仍须保留可比的 serial 输出 |

基准有两组容易混淆的数字：`14.363 s` 是 pinned C libwebp decoder 的真实 aggregate；`13.822 s` 是 decoder-aware encoder 实验中，Rust decoder 对三组 libwebp 生成流的逐图中位数求和。任何“相对 libwebp”的结论必须指明它比较的是 C decoder，还是 libwebp 生成的码流。

## Coarse 产品跨实现五轮对照

这组补测来自最终产品二进制和 pinned `libwebp@733c91e`。两个 runner 都预载输入、完整 materialize RGBA、做全字节 checksum/memcmp、正反交替五轮并记录进程 CPU/RSS/MAD；Rust 与 C 是两次独立锁定运行，因此跨实现百分比标 `†`，但每个实现内部的四种流比较是同 binary。

| Decoder | 标准流 | 输入 bytes | 五轮中位 | MAD | CPU 中位 | RSS 中位 | 输入 MB/s | RGBA MB/s | MP/s |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Rust | public Default | 661,692,326 | 5.002243 s | 0.006319 s | 5.085617 s | 703.25 MiB | 132.3 | 201.4 | 50.3 |
| Rust | fast-no-cache single | 680,790,322 | 3.993188 s | 0.021705 s | 4.081290 s | 720.97 MiB | 170.5 | 252.3 | 63.1 |
| Rust | `FastDecodeCompact` | 617,958,802 | 4.034269 s | 0.015141 s | 4.115655 s | 662.00 MiB | 153.2 | 249.7 | 62.4 |
| Rust | `FastDecodeLowLatency` | 625,321,072 | 4.009531 s | 0.019459 s | 4.084358 s | 669.20 MiB | 156.0 | 251.3 | 62.8 |
| Rust | pinned libwebp m6 生成流 | 265,020,980 | 5.938344 s | 0.032261 s | 5.963286 s | 328.03 MiB | 44.6 | 169.6 | 42.4 |
| pinned C | public Default | 661,692,326 | 5.432180 s | 0.007423 s | 5.685760 s | 1,678.09 MiB | 121.8 | 185.5 | 46.4 |
| pinned C | `FastDecodeCompact` | 617,958,802 | 5.335206 s | 0.001505 s | 5.570309 s | 1,636.69 MiB | 115.8 | 188.8 | 47.2 |
| pinned C | `FastDecodeLowLatency` | 625,321,072 | 5.279929 s | 0.013647 s | 5.531357 s | 1,643.20 MiB | 118.4 | 190.8 | 47.7 |
| pinned C | pinned libwebp m6 生成流 | 265,020,980 | 5.965627 s | 0.004489 s | 6.126984 s | 1,301.30 MiB | 44.4 | 168.9 | 42.2 |

产品结论必须同时保留两面：相对 public Default，Compact/LowLatency 体积小 6.609%/5.497%，Rust 解码快 19.351%/19.845%；相对 pinned m6，它们体积大 133.174%/135.952%，但 Rust 解码快 32.064%/32.481%，pinned C 解码也快 10.568%/11.494%。因此这是跨 decoder 成立的速度/体积 Pareto，不是 m6 的压缩率替代品。

## 顶部纪录准入规则

新结果只有同时满足以下条件，才可以加入或替换顶部纪录：

1. 在相同类别中时间严格更强，或形成不能被现有结果同时按时间和体积支配的新 Pareto 点。
2. 完整输出逐字节一致；标准码流由项目 decoder 和 pinned libwebp 验证，私有表示还必须验证标准 fallback。
3. 单线程、单图流水线、批量并行必须分栏；不得把吞吐提升写成单图 latency 或算法提升。
4. 必须记录完整轮次原始样本、中位数、输入 bytes、RGBA bytes、像素数、线程数、CPU time、RSS/working peak、encode/append 时间和 phase 拆分。没有测到的指标写“未测”，不能省略。
5. 必须给同一最终二进制中的旧/新交错 A/B，并同时运行 pinned libwebp C decoder。跨 session 的固定参考只能标 `†`。
6. 输入必须在计时前载入；输出必须在计时内完整 materialize、black-box/checksum，并在下一张图前释放。不得用 lazy output、缓存结果或漏算转换/CRC 获益。
7. 速度更慢但体积、内存或代码复杂度更好的结果写在实验账本，不进入顶部性能表。

## 工作树与分支强制规则

1. **每次新工作树必须从当时最新的 `main` 创建。** 创建前先快进本地 `main`，记录完整 base SHA；禁止从旧实验分支继续派生。历史 `232a32c`/`eca32b4` 工作树只作为证据源，后续方案必须把最小行为重新迁移到最新 main。
2. **每个工作树必须立即挂在可识别分支上。** 默认使用 `codex/<topic>`，禁止长期 detached HEAD。任务开始时记录 `branch -> worktree -> base SHA`，最终记录 HEAD/commit；未达门槛也保留分支名与结果位置。
3. **每个分支结果必须在本 README 指定位置。** 至少登记任务 ID、分支、HEAD、base、当前工作树绝对路径、报告/原始数据路径和 promotion 决定。工作树被删除后，分支与 commit 仍是永久定位键。
4. 一个实验只负责一个假设。不得把另一个实验的未提交代码、无关 rustfmt、语料缓存或主工作树改动带入提交。
5. 新架构先做独立 feature-private A/B；达到门槛后再迁移到最新 main，最后才讨论稳定 API 或默认输出。
6. 未达门槛只意味着回滚候选实现；报告、复现命令或 runner、原始统计仍必须提交到原实验分支。禁止把唯一证据留在 untracked 文件或任务回复中，账本必须登记证据 commit SHA。

推荐的创建与核验顺序：

```sh
git -C /Users/lance/Desktop/webp fetch origin
git -C /Users/lance/Desktop/webp switch main
git -C /Users/lance/Desktop/webp merge --ff-only origin/main
git -C /Users/lance/Desktop/webp worktree add \
  -b codex/your-topic /Users/lance/.codex/worktrees/your-slot/webp main
git -C /Users/lance/.codex/worktrees/your-slot/webp branch --show-current
git -C /Users/lance/.codex/worktrees/your-slot/webp rev-parse HEAD
```

若主工作树有用户未提交修改，不得为创建实验而 stash、reset 或覆盖；应只快进可安全更新的 refs，再从已确认的最新 `main` commit 建树。

## 与本任务关联的工作树索引

根任务：`019f8321-035e-7211-8f53-987e18891c8c`。下表覆盖该任务已经收口的 47 个 VP8L/FDEC 实验、验证与产品迁移任务；更早的 `vp8l-huffman-paper-feasibility` 属于另一根任务，未混入这份计数。一个假设若因系统中断、创建期间 main 前进或通过实验 gate 后另建 latest-main 产品迁移树，各棵树分别登记，避免把失效 preflight 或诊断提交误认成产品 HEAD。

| ID | 实验 | 分支 / HEAD | 实验 base | 当前工作树与结果 | 决定 |
| --- | --- | --- | --- | --- | --- |
| E01 | 单线程解码架构扫描 | `codex/vp8l-architecture-experiments@eca32b4` | `eca32b4` | [9f3a worktree](</Users/lance/.codex/worktrees/9f3a/webp>)；task `019f85f5-4740-7073-83c1-2e69905d906d`；最终回复已汇总于下文 | 拒绝；无 commit |
| E02 | 批量并行吞吐 | `codex/vp8l-batch-parallel-ab@664d142` | `eca32b4` | [report](</Users/lance/.codex/worktrees/ffb9/webp/docs/vp8l-batch-parallel-benchmark.md>)；[ffb9](</Users/lance/.codex/worktrees/ffb9/webp>)；task `019f85f7-2bac-7a42-a56d-bb15adbf8bd6` | benchmark-only commit |
| E03 | target-cpu / PGO | `codex/vp8l-clic-native-pgo@eca32b4` | `eca32b4` | [4d4d worktree](</Users/lance/.codex/worktrees/4d4d/webp>)；task `019f85f8-e494-7a51-9688-39498d8af2ac` | 拒绝；无 commit |
| E04 | 单图双阶段流水线 | `codex/vp8l-single-image-pipeline@66356c6` | `eca32b4` | [8f5b worktree](</Users/lance/.codex/worktrees/8f5b/webp>)；task `019f85f9-97d7-76a1-a756-117191120bee` | benchmark-only commit |
| E05 | LZ77 overlap copy | `codex/vp8l-lz77-overlap-ab@eca32b4` | `eca32b4` | [f9c4 worktree](</Users/lance/.codex/worktrees/f9c4/webp>)；task `019f85fb-4fb1-7c03-a705-0895d1ed858e` | 回滚；无 commit |
| E06 | 替代 decoder backend | `codex/vp8l-backend-bakeoff@ce4acf6` | `eca32b4` | [RESULTS](</Users/lance/.codex/worktrees/4c95/webp/tools/vp8l-backend-bakeoff/RESULTS.md>)；[4c95](</Users/lance/.codex/worktrees/4c95/webp>)；task `019f85fb-8fc6-71e2-b8b8-082891e84a96` | 工具/报告 commit；不采用 backend |
| E07 | decoder-aware VP8L encoder | `codex/vp8l-fast-decode-profile@232a32c` | `eca32b4` | [report](</Users/lance/.codex/worktrees/c68f/webp/docs/vp8l-fast-decode-research.md>)；[c68f](</Users/lance/.codex/worktrees/c68f/webp>)；task `019f8677-3a61-71c1-88b9-cfbe8f2059d4` | research commit；未产品化 |
| E08 | mode-11 SWAR/重排 | `codex/experiment-vp8l-select-swar@5e8dd93` | `eca32b4` | [report](</Users/lance/.codex/worktrees/896e/webp/docs/vp8l-select-predictor-experiment.md>)；[896e](</Users/lance/.codex/worktrees/896e/webp>)；task `019f8678-4bfe-7731-a4bc-ca3d3d6cf00b` | 回滚；报告/benchmark commit |
| E09 | phase-aware pair Huffman | `codex/vp8l-phase-aware-pair-huffman@b1ad6e1` | `eca32b4` | [report](</Users/lance/.codex/worktrees/2e99/webp/docs/vp8l-pair-huffman-experiment.md>)；[2e99](</Users/lance/.codex/worktrees/2e99/webp>)；task `019f8678-4bfd-7611-ad80-d46702c1483d` | 回滚；报告/工具 commit |
| E10 | 通用 LZ77 + 感知 parse | `codex/vp8l-lz77-aware-parse@d93b670` | `232a32c` | [report](</Users/lance/.codex/worktrees/c48b/webp/docs/vp8l-fast-decode-research.md>)；[c48b](</Users/lance/.codex/worktrees/c48b/webp>)；task `019f869f-8cda-7b83-874f-4aca08937909` | 回滚；仅报告 commit |
| E11 | 块级廉价 predictor | `codex/vp8l-block-predictor-search@90c3fde` | `232a32c` | [report](</Users/lance/.codex/worktrees/fb44/webp/docs/vp8l-block-predictor-research.md>)；[fb44](</Users/lance/.codex/worktrees/fb44/webp>)；task `019f869f-8cda-7b83-874f-4aa876fc1b4e` | 回滚；报告 commit |
| E12 | 长度受限 Huffman | `codex/vp8l-huffman-lengths@d4a8084` | `232a32c` | [report](</Users/lance/.codex/worktrees/69bc/webp/docs/vp8l-huffman-length-research.md>)；[69bc](</Users/lance/.codex/worktrees/69bc/webp>)；task `019f86b7-c666-7220-8685-0e82700ad38f` | 回滚；报告 commit |
| E13 | block32 predictor + max10 | `codex/vp8l-joint-predictor-huffman@a1fab27` | `232a32c` | [report](</Users/lance/.codex/worktrees/cd33/webp/docs/vp8l-joint-predictor-huffman-research.md>)；[cd33](</Users/lance/.codex/worktrees/cd33/webp>)；task `019f86c5-2e55-7a12-8310-c72fed321181` | 回滚；报告 commit |
| E14 | FastDecode 专用 decoder | `codex/vp8l-fast-stream-decoder@f2c03f5` | `232a32c` | [report](</Users/lance/.codex/worktrees/ae81/webp/docs/vp8l-fast-stream-decoder-research.md>)；[ae81](</Users/lance/.codex/worktrees/ae81/webp>)；task `019f86da-dd36-7a42-9776-18d8398eebb8` | 回滚；报告 commit |
| E15 | FDEC codec/transform bake-off | `codex/fdec-codec-bakeoff@4c6d7b0` | `232a32c` | [report](</Users/lance/.codex/worktrees/b58e/webp/docs/fdec-codec-bakeoff.md>)；[b58e](</Users/lance/.codex/worktrees/b58e/webp>)；task `019f86f4-fa7d-7d20-bea2-e63def908702` | **promotion commit** |
| E16 | FDEC 最新-main 迁移与热路径融合 | `codex/fdec-hot-path-migration@ba4b530` | `5e54dd3` | [report](</Users/lance/.codex/worktrees/a386/webp/docs/fdec-hot-path-migration.md>)；[a386](</Users/lance/.codex/worktrees/a386/webp>)；task `019f871c-f1bd-74c3-bdd2-70e784208713` | **3 commits，保留** |
| E17 | FDEC 229 图泛化与生态验证 | `codex/fdec-generalization-validation@db14bc4` | `5e54dd3` | [report](</Users/lance/.codex/worktrees/cb21/webp/docs/fdec-generalization-report.md>)；[raw CSV](</Users/lance/.codex/worktrees/cb21/webp/docs/fdec-generalization-results.csv>)；[cb21](</Users/lance/.codex/worktrees/cb21/webp>)；task `019f871c-f1b8-71f1-a337-cc4e3e371bd2` | **evidence commit** |
| E18 | mode-11 Select 小块投机 | `codex/vp8l-select-speculative-simd@b1245ce`；证据 `9cb5d3a` | `0e2ebb4` | [report](</Users/lance/.codex/worktrees/f501/webp/docs/vp8l-select-speculative-simd-report.md>)；[raw CSV](</Users/lance/.codex/worktrees/f501/webp/docs/vp8l-select-speculative-statistics.csv>)；[f501](</Users/lance/.codex/worktrees/f501/webp>)；task `019f877e-b169-7eb1-922e-6a3799419140` | 模型 gate 拒绝；无生产代码/依赖 |
| E19 | 单线程行流式 transform 融合 | `codex/vp8l-row-stream-fusion@dde1f39`；候选 `6fd5a9a`；证据 `dd36c40` | `e72ed3b` | [report](</Users/lance/.codex/worktrees/ee22/webp/docs/vp8l-row-stream-fusion-report.md>)；[raw](</Users/lance/.codex/worktrees/ee22/webp/docs/vp8l-row-stream-fusion/raw>)；[ee22](</Users/lance/.codex/worktrees/ee22/webp>)；task `019f877c-8d2c-7000-b447-f7ac59d56171` | 回滚；aggregate 慢 0.54% |
| E20 | 空间 meta-Huffman feasibility | `codex/vp8l-spatial-entropy-groups@422abb6` | **误建于 `5e54dd3`；当时 local main 为 `0e2ebb4`** | [summary](</Users/lance/.codex/worktrees/77fa/webp/experiments/vp8l-spatial-entropy-groups/phase-a-full-102.md>)；[raw TSV](</Users/lance/.codex/worktrees/77fa/webp/experiments/vp8l-spatial-entropy-groups/phase-a-full-102.tsv>)；[77fa](</Users/lance/.codex/worktrees/77fa/webp>)；task `019f8784-b5cf-7733-8f85-ce02e800240a` | 仅条件性证据；基线无效，不进入产品 B |
| E21 | safe-SIMD predictor register microkernel | `codex/vp8l-safe-simd-predictor@935f7d0`；证据 `cb05785` | `0e2ebb4` | [report](</Users/lance/.codex/worktrees/b25c/webp/docs/vp8l-safe-simd-predictor-experiment.md>)；[raw rounds](</Users/lance/.codex/worktrees/b25c/webp/docs/vp8l-safe-simd-predictor-rounds.csv>)；[b25c](</Users/lance/.codex/worktrees/b25c/webp>)；task `019f8788-9e1f-71d0-b7ef-f7d6db22d16e` | 回滚；41 图 aggregate 仅快 2.335% |
| E22 | mode 7/13 空间块 recurrence | `codex/vp8l-recurrence-block-scan@2a1fbf1`；证据 `b070002` | `0e2ebb4` | [report](</Users/lance/.codex/worktrees/4049/webp/docs/vp8l-recurrence-block-scan-report.md>)；[raw cost](</Users/lance/.codex/worktrees/4049/webp/docs/vp8l-recurrence-block-scan-cost.csv>)；[4049](</Users/lance/.codex/worktrees/4049/webp>)；task `019f878d-41cf-7141-afcc-5c3044218ded` | 模型 gate 拒绝；无性能路径/依赖 |
| E23 | 空间 meta-Huffman 产品 v2 | `codex/vp8l-spatial-entropy-product-v2@00354df` | `ea346ff` | [report](</Users/lance/.codex/worktrees/bf0b/webp/experiments/vp8l-spatial-entropy-product-v2/REPORT.md>)；[raw/formal](</Users/lance/.codex/worktrees/bf0b/webp/experiments/vp8l-spatial-entropy-product-v2>)；[bf0b](</Users/lance/.codex/worktrees/bf0b/webp>)；task `019f879d-a62a-7d13-8ee7-b668e2831055` | 回滚；体积 -12.016%，解码慢 11.423% |
| E24 | WorkBudget 最坏界预授权 | `codex/vp8l-preauthorized-work-budget@3b8b6d7`；候选 `64fcc13`；回滚 `7389e51` | `ea346ff` | [report](</Users/lance/.codex/worktrees/b934/webp/docs/vp8l-preauthorized-work-budget-report.md>)；[raw](</Users/lance/.codex/worktrees/b934/webp/docs/vp8l-preauthorized-work-budget/raw>)；[b934](</Users/lance/.codex/worktrees/b934/webp>)；task `019f87a3-87a1-7712-b99b-df1e5ea6aad4` | 回滚；41 图仅快 1.740%，m6 慢 0.065% |
| E25 | Huffman group layout / fixed root | `codex/vp8l-huffman-layout-specialization@495b0f4` | `ea346ff` | [report](</Users/lance/.codex/worktrees/842d/webp/docs/experiments/vp8l-huffman-layout-specialization/report.md>)；[raw/ASM](</Users/lance/.codex/worktrees/842d/webp/docs/experiments/vp8l-huffman-layout-specialization>)；[842d](</Users/lance/.codex/worktrees/842d/webp>)；task `019f87a5-3f17-7a41-b19c-5a0ac0229af0` | 回滚；A/B/C 分别慢 5.16%/47.48%/47.84% |
| E26 | 每块局部 cross-color transform | `codex/vp8l-local-color-transform@ceff122` | `ea346ff` | [report](</Users/lance/.codex/worktrees/5d5c/webp/reports/vp8l-local-color-transform/README.md>)；[raw](</Users/lance/.codex/worktrees/5d5c/webp/reports/vp8l-local-color-transform>)；[5d5c](</Users/lance/.codex/worktrees/5d5c/webp>)；task `019f87a8-2922-7792-90a8-d0154c94d16f` | 阶段 A 拒绝；所有 local 档均比有效 no-transform 基线更大 |
| E27 | 纯标量 predictor outlining | `codex/vp8l-scalar-predictor-outlining@a59feed` | `ea346ff` | [report](</Users/lance/.codex/worktrees/5e03/webp/docs/vp8l-scalar-predictor-outlining-experiment.md>)；[raw](</Users/lance/.codex/worktrees/5e03/webp/docs/vp8l-scalar-predictor-outlining-rounds.csv>)；[5e03](</Users/lance/.codex/worktrees/5e03/webp>)；task `019f87aa-9ca2-7eb1-9fc6-274c7d1820a6` | 回滚；端到端仅快 0.326% |
| E28 | 64-bit 两像素 literal bundle | `codex/vp8l-two-literal-bundle@f2ca4ed`；候选 `18a10f1`；runner `e3c0b14`；回滚 `567bbc8` | `6627800` | [report](</Users/lance/.codex/worktrees/b657/webp/reports/vp8l-two-literal-bundle/REPORT.md>)；[raw](</Users/lance/.codex/worktrees/b657/webp/reports/vp8l-two-literal-bundle/raw>)；[b657](</Users/lance/.codex/worktrees/b657/webp>)；task `019f87b9-aaaa-7041-8216-7a14e31eeb3e` | 回滚；41 图快 3.576%，低于 5% |
| E29 | color-transform wire 根因验证 | `codex/vp8l-color-transform-validity-fix@9fa7f5c` | `6627800` | [0d48](</Users/lance/.codex/worktrees/0d48/webp>)；task `019f87b4-6454-7a83-abb5-a5e948469dc2`；诊断任务多次 system-error，完整证据由 E30 重建 | 最小修复确认；不直接作为产品 HEAD |
| E30 | color-transform latest-main 产品迁移 | `codex/vp8l-color-transform-fix-product@e8066a3`；代码 `fb17a98` | `11f6f66` | [report](../../experiments/vp8l-color-transform-fix-product/REPORT.md)；[raw](../../experiments/vp8l-color-transform-fix-product)；[689c](</Users/lance/.codex/worktrees/689c/webp>)；task `019f87c8-58a0-7692-8e92-597b782957b0` | **已快进 main**；失败 101/102 -> 0/102，306/306 防回归通过 |
| E31 | 流量感知可变宽 pair transducer | `codex/vp8l-adaptive-pair-transducer@95dfa3d`；候选 `26b9c21`；证据 `4f8f34d` | `11f6f66` | [report](</Users/lance/.codex/worktrees/a784/webp/docs/vp8l-adaptive-pair-transducer-experiment.md>)；[raw](</Users/lance/.codex/worktrees/a784/webp/docs/raw>)；[patch](</Users/lance/.codex/worktrees/a784/webp/docs/patches/0001-perf-vp8l-prototype-adaptive-pair-transducers.patch>)；[a784](</Users/lance/.codex/worktrees/a784/webp>)；task `019f87c2-b758-7012-9e5b-1b4ace778b2d` | 回滚；A-only 仅快 1.075%，A+B 全部变慢 |
| E32 | coarse spatial meta-Huffman Pareto | `codex/vp8l-coarse-spatial-entropy@0240db2`；候选 `72409d7` | `11f6f66` | [report](</Users/lance/.codex/worktrees/6d6b/webp/experiments/vp8l-coarse-spatial-entropy/REPORT.md>)；[raw/reproducer](</Users/lance/.codex/worktrees/6d6b/webp/experiments/vp8l-coarse-spatial-entropy>)；[6d6b](</Users/lance/.codex/worktrees/6d6b/webp>)；task `019f87ca-6fbe-7d53-b10d-a265031b50aa` | **通过实验 gate**；128/64 体积 -9.229%、解码 +1.939%，转 P08 产品迁移 |
| E33 | coarse spatial stable profiles 产品迁移 | `codex/vp8l-coarse-spatial-product@a489d0b`；代码 `fb869383` | `52c6b8fc` | [report](../../experiments/vp8l-coarse-spatial-product/REPORT.md)；[raw/reproducer](../../experiments/vp8l-coarse-spatial-product)；[070b](</Users/lance/.codex/worktrees/070b/webp>)；task `019f87f5-d9a0-7281-a319-5d6e4a1fc510` | **已线性迁入 main**：代码 `9776da40`、证据 `00f2f587`、raw whitespace policy `e35a00db`；两档 gate、正确性与 pinned C 泛化均通过 |
| E34 | exact-cost single-write 实验 | `codex/vp8l-exact-cost-single-write@a8570f47`；候选 `a89e0f73`；证据 `c0b6544e` | `5362912a` | [report](</Users/lance/.codex/worktrees/b99f/webp/experiments/vp8l-exact-cost-single-write/REPORT.md>)；[raw/reproducer](</Users/lance/.codex/worktrees/b99f/webp/experiments/vp8l-exact-cost-single-write>)；[b99f](</Users/lance/.codex/worktrees/b99f/webp>)；task `019f8825-e240-7f42-a04c-c1fa77b80476` | **通过实验 gate**：Compact/LowLatency -28.823%/-29.110%，306/306 byte identity，转 E35 latest-main 产品迁移 |
| E35 | exact-cost single-write 产品迁移 | `codex/vp8l-exact-cost-product@4803b2d`；代码 `6ed10e55`；证据 `6369ddcd` | `130aa1f3` | [report](../../experiments/vp8l-exact-cost-product/REPORT.md)；[raw/reproducer](../../experiments/vp8l-exact-cost-product)；[6368](</Users/lance/.codex/worktrees/6368/webp>)；task `019f885a-c777-70c2-83c1-f622b78e3363` | **已线性迁入 main**：代码 `97d6f1f4`、证据 `00f02468`、whitespace policy `61aa5899`；两档 -28.389%/-28.966%，306/306 byte identity 与双 decoder exact |
| E36 | packed token writer 实验 | `codex/vp8l-packed-token-writer@6000af0a`；候选 `dfc0cf6f`；证据 `1f8635c1` | `7eca2b83` | [report](</Users/lance/.codex/worktrees/b8f0/webp/experiments/vp8l-packed-token-writer/REPORT.md>)；[raw/reproducer](</Users/lance/.codex/worktrees/b8f0/webp/experiments/vp8l-packed-token-writer>)；[b8f0](</Users/lance/.codex/worktrees/b8f0/webp>)；task `019f8890-c433-7013-b862-00f8c5f4221a` | **通过实验 gate**：最终 binary 上 Compact/LowLatency -27.657%/-28.119%，306/306 byte identity 与双 decoder exact，转 latest-main 产品迁移 |
| E37 | packed token writer 产品迁移 | `codex/vp8l-packed-writer-product@a7cde726`；代码 `9435fbd0`；证据 `a7cde726` | `0ee428dc` | [report](../../experiments/vp8l-packed-writer-product/REPORT.md)；[raw/reproducer](../../experiments/vp8l-packed-writer-product)；[5e00](</Users/lance/.codex/worktrees/5e00/webp>)；task `019f88d1-ed7a-7573-8898-d78525870e70` | **已线性迁入 main**：代码 `b3b96fdc`、证据 `80113c1e`、whitespace policy `fabcbf9c`；两档 -27.005%/-26.561%，latest-main/E36 各 306/306 byte identity 与双 decoder exact |
| E38 | 流式 tokenization + spatial sufficient statistics | `codex/vp8l-streaming-spatial-plan@d2207f45`；S+C `daadb6f1`；F `f5e5bee5`；修正 `815df546`；诊断 `292c1d74`；证据 `a2295c3d` | `cec68762` | [report](../../experiments/vp8l-streaming-spatial-plan/REPORT.md)；[raw/reproducer](../../experiments/vp8l-streaming-spatial-plan)；[25a6](</Users/lance/.codex/worktrees/25a6/webp>)；task `019f8915-45d9-7a90-a843-4d0062ade22b` | **拒绝，不迁移代码**：修正版 S+C+F 两档 -2.658%/-3.191%，最强 materialized C+F -5.899%/-3.520%，均未过双档 10%/零回退 gate；306/306 byte identity、918/918 pinned C exact；完整负证据归档 main |
| E39 | frequency-owned spatial clustering | `codex/vp8l-frequency-owned-clustering@3468fcff`；E `c38e98aa`；对称 A/B `6703a163`；B `2d529c33`；报告 `bb7002e9`；checksum `3468fcff` | `3474599d` | [report](../../experiments/vp8l-frequency-owned-clustering/REPORT.md)；[raw/reproducer](../../experiments/vp8l-frequency-owned-clustering)；[6d5d](</Users/lance/.codex/worktrees/6d5d/webp>)；task `019f8960-1a51-75a3-aec4-f99a1e7fb5de` | **拒绝，不迁移代码**：E 两档 encode -34.540%/-36.188% 但 aggregate bytes +0.423%/+0.388%、各 8/41 超 +2%；B 的 102 图 rate 仍 +0.389%/+0.419%；918/918 双 decoder exact，未过 rate gate，故不跑 formal |
| E40 | exact-cost multi-proposal + one-pass entropy-aware refinement | `codex/vp8l-entropy-aware-spatial-clustering@76762d10`；实现 `eacad8bf`；Phase A `f78ca14e`；fair screen `7d14b835`；证据 `a52a3cce`；复现 `76762d10` | `0e91e379` | [report](../../experiments/vp8l-entropy-aware-spatial-clustering/REPORT.md)；[raw/reproducer](../../experiments/vp8l-entropy-aware-spatial-clustering)；[3cd9](</Users/lance/.codex/worktrees/3cd9/webp>)；task `019f899d-1871-7453-8450-630ffe00ecd1` | **拒绝，不迁移代码**：screen encode -26.605%/-29.481%，aggregate bytes -3.482%/-1.738%，但 LowLatency 图 008 稳定 +4.338% 超过 +2% 硬门；formal 按规则未跑，完整负证据归档 main |
| E41 | capacity-growing exact-cost split/refine clustering | `codex/vp8l-capacity-growing-clustering@13c0f2a1`；规则 `7641d33a`；实现 `31595aa3`；Phase A `0ba25f17`；共享准备 `58327b09`；screen `e1b6c851`；checksum `13c0f2a1` | `ec7fbaf6` | [report](../../experiments/vp8l-capacity-growing-clustering/REPORT.md)；[raw/reproducer](../../experiments/vp8l-capacity-growing-clustering)；[5d9b](</Users/lance/.codex/worktrees/5d9b/webp>)；task `019f89e8-4f41-7b12-b14d-4da149d07b3a` | **拒绝整套双档方案，不迁移代码**：Phase A 两档 rate -11.410%/-3.825%、0/102 超 +2%；screen Compact +79.086%、40/41 回退而失败，LowLatency -47.145%、0/41 回退并单档全过；246/246 双 decoder exact，formal 未跑 |
| E42 | multi-resolution exact-cost spatial portfolio | `codex/vp8l-multires-spatial-portfolio@9a8b7d23`；实现 `41c24db5`；归因修正 `bdb709ea`；Phase A `c151f06b`；checksum `9a8b7d23` | `ec7fbaf6` | [report](../../experiments/vp8l-multires-spatial-portfolio/REPORT.md)；[raw/reproducer](../../experiments/vp8l-multires-spatial-portfolio)；[dfbc](</Users/lance/.codex/worktrees/dfbc/webp>)；task `019f89e8-dcd1-7a43-ba7b-a8406d10740e` | **Phase A 拒绝，不迁移代码**：Compact 精确复现 E40；LowLatency aggregate -4.182%，但图 074 +4.993% 超 +2%；99/102 选择 128，产品区分度弱，screen/formal 未跑 |
| E43 | profile-specialized exact-cost hybrid | `codex/vp8l-profile-hybrid-clustering@c04bed7b`；设计 `3dea69cc`；实现 `a0606a83`；归因 `36ad7acd`；最终证据 `c04bed7b` | `58f7b8dd` | [report](../../experiments/vp8l-profile-hybrid-clustering/REPORT.md)；[durable evidence/reproducer](../../experiments/vp8l-profile-hybrid-clustering)；[7d78](</Users/lance/.codex/worktrees/7d78/webp>)；task `019f8a34-8286-70d2-84ba-461fbb4117d5` | **通过全部研究 gate，转独立产品迁移**：formal Compact/LowLatency -52.388%/-51.401%，两档 0/102 回退；rate -3.004%/-3.825%，0/102 超 +2%；612/612 双 decoder exact，Default 102/102 byte identity；研究代码不直接合入 |
| E44 | profile hybrid 产品迁移 preflight（P19） | 无分支；detached `f4c4ae0b` | 请求 base `f4c4ae0b`；启动时 local main `f0b5fd4d` | [失效说明](../../experiments/vp8l-profile-hybrid-product/invalidated-runs/p19-base-race.md)；[4365](</Users/lance/.codex/worktrees/4365/webp>)；task `019f8a82-b5c3-7291-8406-883fdb7cdbdf` | **创建身份失效，零修改停止**：HEAD/merge-base 为 `f4c4ae0b`，但任务开始核验时 main 已前进到 `f0b5fd4d`；未建分支、未实现、未测量，另建 latest-main 产品树 |
| E45 | profile hybrid 最小产品迁移（P20） | `codex/vp8l-profile-hybrid-product@cebc0981`；设计 `09863f08`；产品 `67bd0427`；最终 binary `9aa8fa08…a29f8` | `66c15f11` | [report](../../experiments/vp8l-profile-hybrid-product/REPORT.md)；[durable evidence/reproducer](../../experiments/vp8l-profile-hybrid-product)；[5020](</Users/lance/.codex/worktrees/5020/webp>)；task `019f8a85-c530-79d2-af1f-2b54105574be` | **screen 拒绝，不迁移代码**：Compact -50.482% 通过；LowLatency -48.190%，仅差 1.810pp 而失败 ≥50% 硬门；两档 0/41 回退、rate/双 decoder/RSS 全过，formal 按规则未跑 |
| E46 | zero-eliding sparse histogram merge recovery（P21） | `codex/vp8l-sparse-histogram-merge@6f82035d`；设计 `c57e7eac`；dense product `1746c7bd`；机制 `60dc7c99`；测量 `a07b3d21` | `8485fc05` | [report](../../experiments/vp8l-sparse-histogram-merge/REPORT.md)；[durable evidence/reproducer](../../experiments/vp8l-sparse-histogram-merge)；[1841](</Users/lance/.codex/worktrees/1841/webp>)；task `019f8aba-a8d0-73e3-b1b8-434634e9eea6` | **recovery 拒绝，不迁移代码**：72.292% slot 为零但 B 未加速；Compact +0.034%，LowLatency +0.430% 且 23/41 回退；204/204 A/B/P18 exact，产品 Phase A/screen/formal 未跑 |
| E47 | metric-only search / final-plan materialization（P22） | `codex/vp8l-metric-only-plan-search@4b80999f`；设计 `479a5149`；机制 `60719703`；测量 `688452ec` | `4280a59a` | [report](../../experiments/vp8l-metric-only-plan-search/REPORT.md)；[durable evidence/reproducer](../../experiments/vp8l-metric-only-plan-search)；[c5fc](</Users/lance/.codex/worktrees/c5fc/webp>)；task `019f8add-4346-70a2-a831-530db819cb8f` | **recovery 拒绝，不迁移代码**：B 将峰值 full plan 3→1，但 Compact +0.918%、LowLatency +0.081%，分别 34/41、31/41 回退；204/204 A/B/P18 exact，产品阶段未跑 |

### latest-main 迁移链

E31/E32 均从各自创建时最新的本地 `main@11f6f669215479848628c1bdcd438c2a891e96fb` 建树；E32 通过后没有直接合入，而是按规则从届时最新 `main@52c6b8fc64cd86b4fccd0f30fb996d825a6dd2ec` 新建 P08，最终作为 E33 线性迁入 main。P09/E34 又从创建时最新 `main@5362912a23a39175758796e07f45af3ee79143b1` 独立建树；通过 25% gate 后，没有直接把研究树合入，而是从届时最新 `main@130aa1f347ae1193463f35205b5bd98b4031bc7c` 新建 E35，重新理解并迁移最小产品实现。E35 最终作为 `97d6f1f4`/`00f02468`/`61aa5899` 线性进入 main。P11/E36 则从创建时最新 `main@7eca2b83c2b9338ab4f15a58755e6e0acc970bf0` 独立建树；研究树已证明 packed token writer 的端到端收益和 wire identity，但没有把 census/phase instrumentation 带进产品。P12/E37 随后从创建时最新 `main@0ee428dc0bee9c035f051b4ccaa846dabe394ca8` 新建独立产品树，重建最小 packet sink；产品代码、完整证据和 raw whitespace policy 已分别作为 `b3b96fdc`/`80113c1e`/`fabcbf9c` 线性迁入 main。P13/E38 从创建时最新 `main@cec68762e5ab6184bce275aeff5720ba3e6f96c7` 独立建树；它通过完整 screen 和复现证明“只融合 pass”不足以过 10% gate，因此没有 rebase、没有产品迁移，只把报告、raw 与复现器归档进 main。P14/E39 从创建时最新 `main@3474599d89804cb91357788e967826544903011c` 独立建树；后续 main 前进时保留原 base 和完整实验链。它证明 exact-frequency ownership 可删除主导 census 成本，但 E/B 都未过 rate gate，因此同样不 rebase、不迁移研究代码，只把报告、raw、失效运行和复现器归档进 main。P15/E40 从创建时最新 `main@0e91e379aef2cfac1189472a3dd0627060f892b8` 独立建树；后续登记提交 `cef04c68` 没有倒灌或 rebase 到研究树。它证明一次 exact-cost reassignment 可同时保留约 27–30% screen encode 收益并改善 aggregate rate，但 LowLatency 的稳定单图长尾仍违反产品门槛，因此不迁移任何代码，只归档 `76762d10` 的报告、raw、失效运行与一键复现器。P16/E41 与 P17/E42 都从创建时最新的 `main@ec7fbaf69f423bfd7251a121d2e629cfa776cb79` 独立建树；登记提交 `cb89e317` 只作为 post-creation provenance，两棵研究树均未 rebase/merge。E41 证明容量增长对 LowLatency 同时具备大幅速度、rate 与 tail 收益，但 Compact 的近四千次 split 搜索不适用；E42 证明简单 128/256 exact portfolio 无法消除 074 长尾且几乎退化成 Compact。P18/E43 从创建时最新的 `main@58f7b8dd047cad1733bc2766a797d8f2e4b5ff3c` 建树，登记提交 `7f5cd83c` 只作为 post-creation provenance，没有 merge/rebase；它在同一最终 binary 中证明 Compact/E40 与 LowLatency/E41 的固定 profile 分工同时通过全部研究 gate，最终研究 HEAD 为 `c04bed7b`。E43 仍不直接合入。首次产品树 P19/E44 请求从归档提交 `f4c4ae0b` 创建，但在队列完成、任务开始核验前，main 已由独立增量解码工作前进到 `f0b5fd4d`；P19 因此保持 detached、零修改停止。P20/E45 随后从 `main@66c15f11c0cd63a7e5ad80ffbe7553e6f68ec569` 独立建树，登记提交 `c8e29225` 没有倒灌；它手工复建的 204 个产品流与 P18 全部字节一致，但 LowLatency 只因 48.190% 未过主动收紧的 50% screen 门，故未 rebase/merge、未跑 formal，只归档 `cebc0981` 的耐久负证据。P21/E46 从 `main@8485fc0593bf6e29715350ea72b15a9dabf4c80b` 建树，登记提交 `2234932d` 没有倒灌；它选择性复用 P20 产品控制，只改变 zero-eliding merge，并证明 72.292% 可跳过的零槽仍不足以抵消每槽分支成本，故在 recovery gate 立即拒绝、未进入产品验证。P22/E47 从创建时最新的 `main@4280a59a1a7a22d1e312b9de131b46873688c008` 建树并立即挂到 `codex/vp8l-metric-only-plan-search`；登记提交 `29e7d6ef` 没有倒灌。它证明 full-plan 峰值从 3 降到 1 仍无法恢复速度，最终重建抵消了释放 prefix/tables 的收益，因此不迁移 P20 或 P22 代码，只归档 `4b80999f` 的耐久负证据。基线始终以创建瞬间的本地 committed `main` 为准，不以可能落后的 `origin/main` 替代。

### 进行中的 latest-main 编码优化

P22/E47 已收口并拒绝；下一独立树只验证解析式 exact-cost planning 是否能删除每个候选的 group-map/table 临时序列化，而不是继续优化已证明无收益的 full-plan lifetime。工作树、分支与任务 ID 将在创建并通过 latest-main 身份门禁后登记。

## 每次优化的结果与结论

### E01：单线程架构扫描

优化点：统计 literal/LZ/cache、entropy 与 predictor 占比，评估 packed multi-symbol Huffman、predictor 分派、短行 pipeline 与安全 SIMD 可行性。

- CLIC 固定 Rust 基准约 14.009 s，目标 7.000 s；复测受主机调度影响为 15.133/15.286 s。
- entropy 约占 63%，predictor 约占 28%；只完全消除 predictor 的理论加速上限约 39%。
- 8-bit 多符号表因跨 codebook 可覆盖 literal 极少而失败；当前 row 融合已消除明显的整帧中间 pass。
- 标准 VP8L、safe、单线程范围内没有发现可信 2x 方案，工作树清理为无改动。

### E02：批量并行吞吐

优化点：保持单图 decoder 不变，在 benchmark 层按输入批次使用 scoped threads，并逐文件验证串并行完整结果一致。

- jobs 1/2/4/12：15.447 / 8.758 / 4.388 / 2.843 s；jobs=12 为 5.43x，parallel efficiency 45%。
- CPU time 14.42 -> 16.73 s；峰值 RSS 0.84 -> 1.50 GiB。
- 这是独立图片批量吞吐纪录，不是单图 latency 或 codec 算法突破。

### E03：target-cpu=native 与 PGO

优化点：独立冷 target 构建，比较 release、native、PGO、native+PGO；PGO 用 method 0/3/6 平衡训练集。

- 全量中位：release 14.002 s；native 14.218 s；PGO 14.398 s；native+PGO 15.122 s。
- 留出集 PGO 约 1.3% 的收益未在全量复现；没有继续 fat LTO，也没有保留配置。

### E04：单图双阶段流水线

优化点：调用线程生产 entropy/LZ77 residual，固定 consumer 按行执行 color/predictor/subtract/RGBA；32 行、队列深度 2 最优。

- 306 流 9.375 s，相对 14.009 s 快约 33%，但使用两核且未达到 7.000 s。
- producer 单独约 8.841 s，已构成理论下限；额外峰值最多约 792 KiB。
- 保留 benchmark-only 原型，不进入单线程纪录或稳定 API。

### E05：LZ77 overlap copy

优化点：将 overlap copy 调用次数从 `ceil(length/distance)` 降为 `1 + ceil(log2(length/distance))`，distance=1 使用单次 resize。

- aggregate 反而慢 0.8%；m0/m3/m6 分别慢 6.3%、慢 0.9%、快 4.1%。
- method0 overlap 仅 0.16%，分支与额外判断抵消收益；实现全部回滚。

### E06：替代 decoder backend bake-off

优化点：对当前 Rust、`image-webp 0.2.4`、`oxideav-webp 0.2.3` scalar/SIMD 使用同一 306 流 runner。

- 当前 Rust 14.058 s；image-webp 16.919 s；OxideAV scalar/SIMD 24.511/24.375 s。
- image-webp 306/306 exact 但慢 20.8%；OxideAV 仅 290/306 exact。
- 不引入生产依赖，只提交可复现工具和报告。

### E07：decoder-aware 标准 VP8L encoder

优化点：在标准 VP8L 内按图调整 predictor、color/subtract-green、palette、cache、distance-1 LZ77，并拟合编码时可用的 decode-cost model。

- Pareto：`no_color` 619.3 MB/3.621 s；`no_pred` 645.9 MB/3.095 s；`fast` 671.9 MB/2.928 s；`fast_no_cache` 724.3 MB/2.613 s。
- `fast_no_cache` 相对 libwebp m6 生成流快 44.493%，但体积大 173.302%；306 等价 7.839 s，未达 50%。
- held-out 排序准确率 82.6%，26 图中选中 25 个实测最快候选；证明 encoder/decoder 协同有效，但压缩率缺口阻止产品化。

### E08：mode-11 安全 SWAR/循环重排

优化点：重排 Select predictor 数据并观察编译器 NEON；比较安全 packed-u32/SWAR 与自动向量化标量 pass。

- aggregate 仅快 0.57%，m0 慢 0.04%；predictor phase 在 m0/m3/m6 慢 16.9%/3.6%/4.3%。
- 编译器生成 NEON 能让独立 pass 快 7.35x，但准备/重排成本令完整路径仍慢；性能实现回滚。

### E09：phase-aware pair Huffman

优化点：为绿+红、蓝+alpha 建紧凑 pair 表，控制表宽、构表时间、root/secondary 覆盖和 cache footprint。

- 最终 41 图 aggregate 5,796.117 -> 5,530.195 ms，仅快 4.809%，低于 5% gate。
- method0/3/6 分别快 7.623%/3.403%/5.497%；最佳 10-bit/64 KiB 原型仍无法形成稳定全量收益。
- 性能路径回滚，只保留实验报告与工具。

### E10：通用 LZ77 与 rate/decode-aware parse

优化点：16-bit hash-chain、32 candidates、最长 4096、overlap、greedy/两轮 Size/FastDecode parse，并补齐 VP8L backward-distance 逆映射。

- `no_pred` 体积小 3.321%，但解码慢 4.641%；`fast_no_cache` 体积小 7.211%，但慢 4.599%。
- 306 等价从 7.839 退到 8.200 s，仍比 m6 大 153.595%；生产行为回滚。

### E11：块级廉价 predictor

优化点：4/8/16/32 block 的廉价 predictor mode 搜索，比较 Size 与 FastDecode 目标，避免昂贵 mode 11。

- 最佳 32x32 FastDecode 为 610.24 MB，比 `no_pred` 小 5.53%，但解码慢 14.39%。
- 没达到“体积至少小 10% 且时间退化不超过 5%”，实现回滚。

### E12：长度受限 Huffman

优化点：确定性 package-merge 风格 max10/max15 code length、canonical code 和 VP8L header，替换原平衡树。

- `no_pred` max15 645.9 -> 609.4 MB，减 5.65%。
- `fast_no_cache` max10 724.3 -> 677.1 MB，减 6.52%，数据 symbol 全 root hit；306 等价 7.964 s，未达到 7.000 s。
- 证明 Huffman 解释一部分 rate gap，但不是主要缺口；实现回滚。

### E13：block32 predictor 与 max10 联合优化

优化点：用 max10 实际码长重新评分 predictor，而不是机械叠加两个隔离实验。

- 联合流相对 `no_pred+max10` 小 16.190%，但解码慢 11.591%，超过 10% gate。
- 相对 balanced 的 rate 改善 20.667%，比两个单项简单相加多约 9.8 个百分点，说明非线性协同真实存在。
- 相对 m6 仍大 93.358%，306 等价 10.523 s；下一瓶颈是 predictor phase，代码回滚。

### E14：FastDecode 专用 decoder

优化点：自动识别单 group/no cache/no transform 标准流，测试 10-16 bit pair transducer 与 direct RGBA output。

- 基线 2.614 s；pair-14 2.695 s；direct RGBA 3.228 s；组合 3.375 s。
- direct RGBA 把 RSS 83.9 MB 降至 48.1 MB，但逐 literal 字节写入导致 23.52% 时间退化。
- 16-bit pair 覆盖 99.673%，但每图 512 KiB 随机表伤害 cache；全部性能代码回滚。

### E15：FDEC codec/transform bake-off

优化点：保留原样 m6 VP8L fallback，在 RIFF 尾部加入可忽略 FDEC；对 RGB/RGBA、QOI-like、LZ4、Zstd-1 与 none/decorrelate/Sub/Paeth/byte-plane 共 30 点筛选。

- 初始保留点：Zstd-Sub 663,622,132 bytes / 1.743 s；LZ4-none 935,997,910 bytes / 0.586 s。
- QOI RGBA 2.069 s 被 Zstd-Sub 同时按速度和体积支配；Zstd-Paeth 4.237 s，逆 Paeth 单独 3.368 s。
- 两条保留路径项目与 pinned libwebp 各 102/102 exact；未知、损坏、超限 FDEC 安全回退。
- 这是第一个在单线程、非 SIMD、非并发条件下跨过 50% 目标的架构结果。

### E16：FDEC 最新 main 迁移与热路径融合

优化点：在 `main@5e54dd3` 重新实现 feature-private FDEC v1；按 contract/payload/pixels/orchestration 拆分，并融合 Row-Sub、RGB->RGBA 和 CRC。

- Zstd 1,643.038 -> 933.680 ms，二次提升 43.17%；最终同轮为 923.689 ms。
- LZ4 506.782 -> 417.977 ms，二次提升 17.52%；最终同轮为 416.581 ms。
- Zstd 将 inverse Sub + conversion + CRC 的 1,108.144 ms 替换为 389.168 ms 融合 pass。
- LZ4 使用 768 KiB 有界 scratch、逆序块搬移、正序 RGBA 展开和 CRC combine；working peak 21,790,720 -> 13,238,272 bytes，减 39.25%。
- Zstd streaming 最佳 948.688 ms，比 bulk-fused 慢 2.03%，按 gate 回滚。

### E17：FDEC 泛化、透明与生态验证

优化点：在另一棵 `main@5e54dd3` 工作树上运行 229 图、6,235 条五轮记录；覆盖原 CLIC 102、固定哈希 disjoint train/test 64、upstream VP8L 43、确定性 UI/纹理/噪声/透明 20，以及工具链与边界行为。

- 未融合实现的 CLIC 306 投影：Zstd 5.256 s，快 62.0%；LZ4 1.589 s，快 88.5%。
- disjoint train：m6 1.529 s，Zstd 0.565 s，LZ4 0.178 s；disjoint test：1.655/0.645/0.217 s。
- 代价：CLIC Zstd 体积 +148% 到 +163%，LZ4 +248% 到 +271%；229 图在 10/25/50% size cap 下覆盖均为 0。
- promoted RGB 对 28 张 alpha 图加速覆盖为 0；RGBA screen 候选快，但体积增加 121.3%/134.5%，需要新协议。
- 完整附加的内存/存储 break-even 约为 Zstd 136.9 MB/s、LZ4 162.3 MB/s；低带宽输入会输给标准 m6。
- `webpmux` metadata 修改保留 FDEC，`dwebp -> cwebp` 重编码移除它；显示兼容不等于加速层可持续。

### E18：mode-11 Select 小块投机

优化点：在完整 102 图、306 个固定流的真实 residual 上统计 mode-11 的 top/left 决策、run 和 2/4/8/16-pixel tile 命中，再用偏乐观的 safe-SIMD 成本上界决定是否值得实现。

- top/left 占 55.729%/44.271%，但选择 run 中位仅 2/1 pixels；tile4 all-top 和首选延续的完整命中率仅 18.099%/28.822%。
- 即使给予理想 4-lane 算术、免费 dispatch/mask/失配定位和最小 fallback，最佳 aggregate 也只预计快 4.010%；按 E08 时间权重为 3.813%。
- 未实现 decoder kernel、未添加依赖，也没有 formal wall/RSS A/B；生产内存、binary 和依赖成本均为零。

### E19：单线程行流式 transform 融合

优化点：entropy 每完成一行就立即执行 color、predictor、subtract-green 与 RGBA 输出，同时保留标准 LZ77 所需的完整 residual history；修正 predictor top 必须使用 subtract-green 之前的中间状态，并对不支持的 transform 顺序显式 fallback。

- 306/306 完整 RGBA 逐字节一致；5 轮正式 A/B 中 m0 快 4.79%，m3 慢 4.68%，m6 慢 0.50%，aggregate 14,238.881 -> 14,315.435 ms，慢 0.54%。
- 候选相对同轮 pinned libwebp aggregate 仍快 1.75%，但丢失当前 Rust 的部分优势；峰值 RSS 946,733,056 B，包括 823,204,804 B 预载输入。
- 主流 CLIC transform shape 已 306/306 命中仍无 aggregate 收益，color indexing 泛化还需 packed-row expansion；候选在 `6fd5a9a` 可复现，最终活动 crate 已回滚。

### E20：空间 meta-Huffman feasibility（错误基线）

优化点：用当前真实 tokenization 精确计入每组五张表、payload extra bits、标准嵌套 group-map 的 header/data，比较独立 tile 与最多 64 组的确定性 clustering。

- 102 图 auto 的 `<=64` 组结果为 661,692,326 -> 606,218,418 B，仅减 8.384%；`fast_no_cache` 为 680,790,322 -> 601,911,782 B，减 11.586%；`no_pred` 相对自身仅减 8.405%。
- 绝对最小的 no_pred clustered 相对 current auto 减 10.578%，但其中一部分来自关闭 predictor，不能全部归因于空间分组。每图相对 auto 的 p00/p50/p100 为 -3.016%/10.118%/24.988%，产品实现必须保留逐图 size fallback。
- 该树误从旧 `origin/main@5e54dd3` 建分支，而创建时本地最新 main 是 `0e2ebb4`；因此只保留条件性 feasibility，不实施阶段 B、不进入顶部表。P01 已从正确的最新 main 独立重跑。

### E21：safe-SIMD predictor register microkernel

优化点：用 `wide 0.7.33` 在安全 Rust 中为 modes 7/11/12/13 构建四像素寄存器 kernel，不落地整行 scratch；同时加入相同函数边界的纯标量对照以分离 SIMD 与代码布局收益。

- 41 图锁内 aggregate 6,592.373 -> 6,438.441 ms，只快 2.335%；predictor phase 的 m0/m3/m6 分别快 2.64%/12.79%/8.77%，均未达到要求。
- mode 7/13 的独立 replay 有真实局部收益，但 mode 11 仍弱；更重要的是 outlined scalar phase 为 593.302/454.790/419.164 ms，明显快于 SIMD 的 799.514/517.579/480.415 ms。
- 这把后续方向从 SIMD 改为 P05 的纯标量 hot/cold outlining。`wide`、feature dispatch 和 runtime kernel 已全部移除；保留 306 流真实 residual benchmark、报告与 raw 数据。

### E22：mode 7/13 空间块 recurrence

优化点：把同一长 mode run 内四个不同空间块作为 SIMD lanes，以各块上一行 top-left 为错误初值先行推进；真实 left 可用后从块首标量 repair，状态首次相等后由确定性 recurrence 接受候选后缀。

- 真实 CLIC residual 的 K=4/8/16 块末收敛率：mode 7 为 72.541%/95.021%/99.215%，mode 13 为 30.253%/63.263%/93.947%；mode 12 仅 3.302%/4.487%/7.293%。
- K=16 的平均 repair 为 mode 7 约 3.69 pixels、mode 13 约 7.42 pixels，但完整四块组覆盖与尾部损失明显。
- 计入非零 load、构造、scratch store、repair、validation、branch 和 tail 后，aggregate 只预计快 3.067%；K=8 预计慢 0.946%。只有把必需成本全部设为零的诊断上界才到 5.368%，因此在实现前拒绝。

### E23：空间 meta-Huffman 产品 v2

优化点：在正确的 latest-main 基线上独立实现四像素 meta-prefix map、最多 64 个聚类 group、标准嵌套 group image 和真实 VP8L writer，并对同一 `fast_no_cache` profile 做 size/latency 双门禁。

- 102 图真实文件从 680,790,322 降至 598,985,852 bytes，同 profile 减 12.016%；项目与 pinned libwebp 对 204 条 A/B 流全部逐字节一致。
- 五轮正式 decode 中位从 4.023 增至 4.482 s，慢 11.423%；配对轮次中位慢 10.263%，超过 5%/8% 两条门禁。
- 根因不是 group 数，而是四像素 map 仍产生 62,977,090 个横向 run boundary。活动实现已回滚；128px 粗块的下一假设转入 P07。

### E24：WorkBudget 最坏界预授权

优化点：用 `5 * remaining_pixels + 4` 的可证明保守界一次预授权热循环，减少约 4.8 亿次逐 symbol `consume` 检查；嵌套图仍独立受限。

- 41 图 aggregate 8,169.231 -> 8,027.065 ms，仅快 1.740%；m0/m3/m6 为快 3.72%/快 0.85%/慢 0.065%。
- 每个 cutoff、truncation 与 306/306 完整流 exact 均通过，但未达到 5% screen gate，未运行正式五轮。
- 候选 `64fcc13` 已由 `7389e51` 显式回滚，`3b8b6d7` 只保存报告和 raw。

### E25：Huffman group layout 与 fixed root

优化点：在 group 构建时识别 `P10/P10/P10/S`，把 enum/layout 分派外提到 run，并测试定长 1024-entry root 对 bounds check 和 secondary path 的影响。

- 主签名覆盖 m0/m3/m6 literal 的 99.937%/95.528%/96.912%，所以覆盖 gate 充分。
- 清理 instrumentation 后的 41 图结果仍为：只外提布局慢 5.16%，fixed root 慢 47.48%，组合慢 47.84%。
- 汇编证明分支确实被外提/消除，但构表时间增加 7.85%，fixed-root 版本 RSS spot 增 2.00%；生产路径全部回滚。

### E26：每块局部 cross-color transform

优化点：比较 16/32/64/128px block 的局部 multiplier 搜索，精确计入 transform image、main entropy 和空间系数变化；阶段 A 不依赖非法 current-auto 流做 promotion。

- 有效 no-transform CLIC 基线为 619,331,782 bytes；b4/b5/b6/b7 分别膨胀 10.584%/9.715%/8.491%/7.650%。
- 逐图 oracle 在四档中选优仍膨胀 7.456%，102 图没有一张获得净收益；440 个 test-only local 流仅用于项目 decoder exact。
- 另行确认当时 current-auto 的 101/102 流非法，因此相对它的数字只保留为 conditional；正确性根因由 E29/E30 修复。

### E27：纯标量 predictor outlining

优化点：独立复测“把 recurrent modes 移出大 dispatcher”的代码布局信号，比较全部 recurrent outlining、仅 cold modes 和 match ordering，不引入 SIMD 或新依赖。

- 最佳 predictor phase 仅快 3.855%/2.441%/2.082%，未复现旧混合实验中的 20%–28% 信号。
- 正式 aggregate 只快 0.326%；m0/m3/m6 反而慢 0.578%/0.277%/1.025%。候选增加 396 B text，RSS 单样本增加 196,608 B。
- 306/306 exact 与独立 14-mode reference 通过；性能代码回滚，只保留更强的 test-only differential 覆盖。

### E28：64-bit 两像素 literal bundle

优化点：从同一个 63-bit snapshot 投机解出连续两个完整 literal pixel 的 8 个 symbol，成功时只推进一次 cursor、合并 WorkBudget 结算并一次追加两个 ARGB。

- Census 显示成功 bundle 覆盖 86.862% entropy pixels、98.89% literal pixels；10 个小流和 127,848 个逐 bit-prefix/truncation case 完全一致。
- 41 图 m0/m3/m6 分别快 0.638%/3.940%/6.067%，aggregate 只快 3.576%；独立 VP8L payload phase spot 快 4.150%。
- 候选二进制只增加 96 B，但仍低于 5% gate；`18a10f1` 已由 `567bbc8` 回滚，没有运行被 gate 禁止的全量五轮。

### E29：color-transform wire 根因验证

优化点：审计 public lossless encoder 的 transform descriptor，确认 `COLOR_TRANSFORM_BLOCK_BITS=7` 表示实际 exponent，而三位 wire 字段必须写 `7 - 2 = 5`。

- 修复前 102 张 public encoder 流有 101 张同时被项目 decoder 与 pinned dwebp 拒绝；首个失败是 `clic-validation-000`。
- 原 wire 写 `111`，decoder 再加 2 后误解为 exponent 9/512px；writer 实际按 exponent 7/128px 写 coefficient image，首图 parser 预期 12 个 coefficient pixels、writer 实写 192 个，随后 nested bit boundary 错位。
- 诊断任务提交最小修复 `9fa7f5c55be869ca852badf7effd9f598bf1f5c6`，但多次后台 system-error；没有把这棵树直接当产品分支。

### E30：color-transform latest-main 产品迁移

优化点：从当时最新 `main@11f6f669` 重放 E29 的四文件最小 diff，重新生成 before/after 证据，并把正确性修复作为独立产品提交迁移。

- Before 为项目/dwebp 各 101/102 失败；after 两套 decoder 均 102/102 exact。101 个 transform 流 hash 改变，唯一 no-transform 流不变。
- 每项和总输出长度都不变，总计 661,692,326 bytes；127/128/129、511/512/513、负系数、透明和 no-transform 边界全部通过。
- 标准 m0/m3/m6 防回归为 306/306 exact；workspace debug/release、fmt、Clippy `-D warnings` 和 diff-check 通过。`fb17a98` 与证据 `e8066a3` 已快进合入 main。

### E31：流量感知可变宽 pair transducer

优化点：不再给每组固定分配 10-bit A+B 表，而是按真实 group traffic 在 none、A-only、B-only、A/B 7/8/9/10-bit 中做 64/128/256 KiB 有界选择；完全相同的 Huffman layout 经逐表相等验证后共享紧凑 transducer。

- Stage-1 模型准确重放 E09：m3/m6 的 A/all-literal 覆盖由约 43.1%/47.2% 提升到 64 KiB 下的 78.049%/82.346%，any/all-literal 达 97.845%/98.891%，因此实现门槛成立。
- 真实 41 图 screen 否定模型到运行时的转化：A-only 64 KiB 仅快 1.075%；B-only 慢 19.851%；A+B 64/128/256 KiB 分别慢 1.811%/0.849%/1.467%。64 KiB 的 m0/m3/m6 为快 3.365%、慢 3.525%、慢 1.471%。
- 候选增加 55,360 B release binary；combined 64 KiB 对 306 条固定流输出 3,022,297,644 RGBA bytes，完整 checksum 与 control 一致。短尾、miss、nonliteral、work-budget 与 malformed/meta-group 测试均通过。
- 由于所有组合档为负且唯一正向档远低于 5%，按预定 gate 不运行正式五轮。候选 `26b9c21`、证据 `4f8f34d` 均保留，`95dfa3d` 已显式回滚生产代码；负报告、raw、runner 和 patch 可独立复现。

### E32：coarse spatial meta-Huffman Pareto

优化点：把 E23 的四像素 meta-prefix block 扩大到 64/128/256px，并在 16/32/48/64 个 group frontier 中联合控制局部熵收益和 decoder group-run 成本；生成普通标准 VP8L，并在 candidate 不严格更小时回退同 profile single RIFF。

- 正式保留两个不互相支配的点：128px/64 groups 为 617,958,802 B，相对 fast-no-cache single 的 680,790,322 B 减 9.229%，五轮 decode 中位 4.030125 -> 4.108264 s（慢 1.939%，配对中位 1.184%）；256px/16 groups 为 625,321,072 B（减 8.148%），decode 4.091907 s（慢 1.533%，配对中位 0.558%）。
- single 加十个过模型 gate 的 coarse layout 共 1,122 条标准流；项目 decoder 与 pinned `WebPDecodeRGBA` 都是 1,122/1,122 完整 RGBA exact。模型与真实 RIFF bytes、prefix/cache/table/map/main/extra bit 分区全部 0 mismatch。
- 128/64 与 256/16 的结构 row-run 上界分别为 1,997,970 与 1,007,545，相对旧四像素的 62,977,090 减 31.52x/62.51x。该数字只是 `height * ceil(width/block)` 的结构上界；copy 可跨 `run_end`，因此 row/group/token switch 均未冒充精确 decoder dispatch，产品影响由锁内实测证明。
- 当前编码会共享一次 tokenization，但仍完整序列化 single 和 candidate 后比较；正式中位因此分别慢 131.713%/125.862%。这不影响本轮 size/decode gate，却是产品化后的首要编码瓶颈：下一独立实验将验证精确 bit-cost 先决策、只写胜出流。
- 候选 `72409d7` 与证据/最终 `0240db2` 已提交，默认 encoder 的 `__text` 与 base 逐字节一致。P08 已从更新后的 `main@52c6b8fc` 新建产品迁移树，不直接把旧实验树合入 main。

### E33：coarse spatial stable profiles 产品迁移

优化点：把 E32 的两个 Pareto 点重新迁移到 latest main，并收敛为显式稳定 options；默认编码、metadata 默认路径和 animation 继续使用原 profile，不把研究特性静默变成默认行为。

- 新增 `LosslessEncodeProfile::{Default, FastDecodeCompact, FastDecodeLowLatency}`、`LosslessEncodeOptions` 以及两个 options 入口；enum/options 均为 `#[non_exhaustive]`。Compact 为 128px/64 groups，LowLatency 为 256px/16 groups，输出仍是普通标准 VP8L。
- encoder 只 tokenization 一次，但本版仍完整序列化 single 与 candidate；只有 candidate 的完整 padded RIFF 严格更小时才采用它。102 图五轮中，single/Compact/LowLatency 编码分别为 6.430381/14.668471/14.253173 s，后两者慢 128.112%/121.654%，因此产品 API 已稳定但编码架构尚未收口。
- Compact/LowLatency 分别为 617,958,802/625,321,072 B，相对同 profile single 小 9.229%/8.148%；Rust 解码为 4.034269/4.009531 s。相对 public Default，它们体积小 6.609%/5.497%，Rust 解码快 19.351%/19.845%。
- pinned `WebPDecodeRGBA` 对 Compact/LowLatency 的五轮解码为 5.335206/5.279929 s，相对 m6 生成流的 5.965627 s 快 10.568%/11.494%；这证明 coarse 布局收益不依赖本项目 decoder，但候选体积仍比 m6 大 133.174%/135.952%。
- 当前 generation 的 Default/single/Compact/LowLatency 共 408 条流，项目 decoder 与 pinned C 均 408/408 完整 RGBA exact；产品流与 E32 306/306 长度、RGBA 和 stream hash 相同。默认 before/after 102 条 TSV 逐字节一致，metadata、透明、tiny、127/128/129、255/256/257 与跨 block copy 都有覆盖。
- workspace debug/release/all-targets、Clippy `-D warnings`、fmt、rustdoc/doctest、WASM、Windows GNU/MSVC 和 C/Python/shell helper 均通过。产品分支代码 `fb869383`、证据 `a489d0b`；main 对应代码 `9776da40`、证据 `00f2f587`，raw TSV 属性修正为 `e35a00db`。

### E34：exact-cost single-write 实验

优化点：把同 profile single 流的 canonical Huffman table 与完整 padded RIFF 精确成本先规划出来；先且只序列化 spatial candidate，候选胜出时不再写必输的 single main payload，single 胜出或 byte tie 时才用缓存 plan 写出与旧实现逐字节相同的回退流。

- Phase A 对 102 图的中位归因显示：validate/tokenize 为 1.628–1.635 s，single table plan 仅 0.004 s，必输的 single main 写出为 4.222–4.236 s，spatial cluster 为 2.837–2.870 s，candidate main 写出为 5.300–5.549 s，wrap/padding/compare 为 0.124–0.129 s。删除 single main 写出的理论可回收量为 Compact 29.210%、LowLatency 28.963%，先通过模型 gate 再实现产品路径。
- 正式五轮中 Compact 14.961090 -> 10.648917 s（-28.823%，配对 -28.754%），LowLatency 14.672747 -> 10.401437 s（-29.110%，配对 -29.110%）；全部样本和 3×MAD outlier 均保留，每张图都没有回退。
- 102/102 独立 single 流的 meaningful bits、rounded payload 和完整 RIFF 精确相等；204/204 profile 决策均 exact、candidate win、0 losing-single write、0 estimator fallback。Default/Compact/LowLatency 共 306/306 与 base 完整 byte identity，项目 decoder 与 pinned `WebPDecodeRGBA` 均 306/306 RGBA exact。
- 分支 `codex/vp8l-exact-cost-single-write`：base `5362912a`，候选 `a89e0f73`，证据 `c0b6544e`，最终/卫生提交 `a8570f47`；工作树 [b99f](</Users/lance/.codex/worktrees/b99f/webp>)，task `019f8825-e240-7f42-a04c-c1fa77b80476`，[完整报告](</Users/lance/.codex/worktrees/b99f/webp/experiments/vp8l-exact-cost-single-write/REPORT.md>)。

### E35：exact-cost latest-main 产品迁移

优化点：不 cherry-pick E34 的研究提交，而从创建时 latest local `main@130aa1f3` 重建最小产品版本；保留私有 `SinglePlan` 的精确成本与 strict fallback invariant，删除 phase instrumentation、candidate-only research layout、生产 `payload_bytes` 和宽泛 dead-code 豁免。

- 正式五轮中 Compact 14.243412 -> **10.199847 s**（-28.389%，配对 -28.433%），LowLatency 13.944728 -> **9.905461 s**（-28.966%，配对 -29.054%）。正式 encoder 每图分位全为改善，两个 profile 都没有单图回退；41 图 screen 也分别改善 28.130%/28.741%。
- Compact 产品进程 wall/CPU/RSS 为 15.096795/15.087664 s/1,216.22 MiB，对照为 19.160958/19.151521 s/1,292.97 MiB；LowLatency 产品为 14.808693/14.798409 s/1,216.41 MiB，对照为 18.850454/18.840279 s/1,289.41 MiB。
- 102/102 single 精确计价；204/204 profile 均 candidate win、0 losing-single write、0 fallback。latest-main/product 以及 product/E34 各 306/306 完整 byte identity；项目与 pinned C decoder 各 306/306 RGBA exact。Default、metadata、animation、wire syntax、依赖、安全与线程模型均不变。
- 产品代码生产模块为 179 行 `single_plan.rs` 与 242 行 `spatial_writer.rs`（另 89 行仅测试）；release rlib +25,856 B / +6.302%，release test binary +20,512 B / +1.404%。host stable workspace debug/release、all-targets、Clippy、fmt、rustdoc/doctest 与证据检查全部通过；当前工具链只安装 host target，因此没有改动全局工具链，跨目标沿用 E33 已通过的产品证据。
- 分支 `codex/vp8l-exact-cost-product`：base `130aa1f3`，代码 `6ed10e55`，证据 `6369ddcd`，最终/卫生提交 `4803b2d`；工作树 [6368](</Users/lance/.codex/worktrees/6368/webp>)，task `019f885a-c777-70c2-83c1-f622b78e3363`，[完整报告](../../experiments/vp8l-exact-cost-product/REPORT.md)。对应 main 线性提交为代码 `97d6f1f4`、证据 `00f02468`、raw whitespace policy `61aa5899`。

### E36：packed token writer 实验

优化点：把每个 Literal 的 green/red/blue/alpha Huffman code，以及 Copy 的 length/extra/distance/extra，按现有 LSB-first wire 顺序预组装为单个 `TokenPacket`；再由私有 `BufferedPacketWriter` 用 64-bit accumulator 和 32-bit little-endian bulk flush 写出，避免每个字段重复进入 `BitWriter::write_bits`。

- 合法极限已由代码与差分测试固定：Literal 最多 60 bit，Copy 最多 58 bit，Cache 最多 15 bit；当前 adaptive table 最长 9 bit，产品语料实际 packet 为 6–25 bit。实现只在中间组包使用 `u128`，最终 sink 保持 safe Rust，并以 `tokens * 8 + 1` 做可失败的预留和显式容量检查。
- 每个 profile 的 102 图共有 244,018,874 个 token：242,507,972 literal、1,510,902 copy、cache=0。原路径调用 `write_bits` 732,056,622 次，新路径只追加 244,018,874 个 packet，调用数精确减少 66.667%。
- Phase A 在 102 图三轮中将收益分离：Compact 的 original/packet-via-old-write/direct-byte-OR/packet+accumulator 为 5.244629/4.371400/4.634385/**2.405114 s**，最终机制改善 54.141%；LowLatency 为 4.875939/3.920810/4.157897/**2.169415 s**，改善 55.508%。这证明主要收益来自“组包 + accumulator”的合作，不是某一个表面 helper 或并发。
- 最终候选 binary SHA-256 为 `260c297d4448a40e361fad5c62cbd6a9c0d00e36256943b2fcd69ac8b980fd73`。同 binary 41 图 screen：Compact 4.545191 -> 3.310370 s（-27.168%），LowLatency 4.420475 -> 3.193324 s（-27.761%），两档均 0/41 逐图回退。
- 102 图五轮正式：Compact 10.769393 -> **7.790943 s**（独立中位 -27.657%，配对 -27.739%），LowLatency 10.515524 -> **7.558638 s**（独立 -28.119%，配对 -27.991%），两档均 0/102 逐图中位回退。LowLatency 候选第一轮 9.037979 s 是保留的 3×MAD outlier，其余轮为 7.532–7.609 s，未因异常轮删数据。
- 正式资源中位的 process wall/CPU/RSS：Compact control 16.044915/15.993002 s/1,215.25 MiB，candidate 13.056734/13.011449 s/1,141.11 MiB；LowLatency control 15.726706/15.674722 s/1,216.39 MiB，candidate 12.819073/12.772517 s/1,152.73 MiB。release rlib 仍为 446,024 B（净增 0 B），test binary +35,184 B/+2.375% 主要来自研究 hooks。
- Default/Compact/LowLatency 共 306/306 与 base 在长度、SHA 和全字节上一致；项目 decoder 306/306 完整 RGBA exact，pinned libwebp `733c91e` 也是 306/306 exact。workspace debug/release all-targets、Clippy `-D warnings`、fmt、rustdoc/doctest 全部通过；未修改工具链或安装新 target。
- 分支 `codex/vp8l-packed-token-writer`：base `7eca2b83`，候选 `dfc0cf6f`，证据 `1f8635c1`，最终报告/HEAD `6000af0a`；工作树 [b8f0](</Users/lance/.codex/worktrees/b8f0/webp>)，task `019f8890-c433-7013-b862-00f8c5f4221a`，[完整报告](</Users/lance/.codex/worktrees/b8f0/webp/experiments/vp8l-packed-token-writer/REPORT.md>)。候选 amend 前旧 binary `7c2ba1f0…` 的数据只保留为 preliminary，顶层结论只使用上述最终 binary 重跑结果。

### E37：packed token writer latest-main 产品迁移

优化点：不整体 cherry-pick E36 的研究实现，而从创建时 latest local `main@0ee428dc` 重建最小产品路径。私有 `spatial_packet_writer` 独占 LSB-first packet、64-bit accumulator、checked reserve/capacity 与错误语义；`spatial_writer` 只保留 token/table orchestration。census、phase variants、benchmark hooks 和宽泛 dead-code 豁免均未进入生产代码。

- 最终完整仓库 archive release test binary SHA-256 为 `247305b53187841383afb7a39a872f1292728e7a114b0d5541547b101da524fe`。41 图三轮 screen 中 Compact 4.503202 -> 3.347753 s（-25.658%），LowLatency 4.390787 -> 3.264446 s（-25.652%），两档均 0/41 回退。
- 102 图五轮正式：Compact 10.787120 -> **7.874026 s**（独立中位 -27.005%，配对 -26.828%），LowLatency 10.401583 -> **7.638855 s**（独立 -26.561%，配对 -26.249%），两档均 0/102 逐图中位回退。Compact round 5 的 7.929905 s 是保留的 3×MAD outlier；LowLatency 无 3×MAD outlier，没有删除任何样本。
- 正式 process wall/CPU/RSS：Compact control 16.046536/16.000232 s/1,215.27 MiB，product 13.139456/13.085052 s/1,143.25 MiB；LowLatency control 15.719175/15.628885 s/1,215.53 MiB，product 12.922736/12.876665 s/1,153.55 MiB。RSS 分别降低 72.02/61.98 MiB。
- release rlib 436,344 -> 453,848 B（+17,504 B/+4.011%）；包含 test-only same-binary control 的 release test binary 1,481,328 -> 1,501,056 B（+19,728 B/+1.332%）。实现保持 safe Rust，无新增依赖、线程、unsafe、API、profile、Default、metadata、animation 或 wire 变化。
- latest-main/product 与 product/E36 各自都是 Default/Compact/LowLatency 共 306/306 长度、SHA 和完整字节一致；项目 decoder 各 306/306 RGBA exact，产品 pinned libwebp `733c91e` 也是 306/306 exact。完整仓库 archive 的 debug/release all-targets、Clippy `-D warnings`、fmt、rustdoc/doctest 全部通过。
- 只把最终完整仓库 archive binary 的结果写入 headline。错误 cwd archive、zsh 特殊 `path` 导致的 non-run、pre-manifest partial、workspace-subtree archive 的 screen/formal/identity、缺 root fixtures 的 validation 与覆盖日志重建均在 [invalidated-runs](../../experiments/vp8l-packed-writer-product/invalidated-runs) 保留原因与影响；逐次 raw 输出由复现器写入外部结果目录，不进入 Git。
- 分支 `codex/vp8l-packed-writer-product`：base `0ee428dc`，代码 `9435fbd0`，证据/最终 HEAD `a7cde726`；工作树 [5e00](</Users/lance/.codex/worktrees/5e00/webp>)，task `019f88d1-ed7a-7573-8898-d78525870e70`，[完整报告](../../experiments/vp8l-packed-writer-product/REPORT.md)。对应 main 线性提交为代码 `b3b96fdc`、证据 `80113c1e`、raw whitespace policy `fabcbf9c`。

### E38：流式 tokenization 与空间统计融合

优化点：把 residual 生成、tokenization、block census 与 group frequencies 拆成可独立对照的 S/C/F 变体，验证“让 token producer 同步拥有空间规划统计、删除中间 materialization 和后续全 token 扫描”能否形成新的高收益数据流架构。

- Phase A 对两档各 102 图、251,858,137 pixels、244,018,874 tokens 做完整归因。Compact/LowLatency 的 residual 为 0.627/0.622 s、tokenization 为 0.913/0.902 s、ordered census 为 2.504/2.530 s、group-frequency pass 为 0.773/0.745 s、packed writer 为 3.276/3.180 s；名义可删除阶段合计约 3.905/3.897 s，但这只是忽略同步更新与 merge 成本的乐观上界。
- 初版 S 循环重复计算 lookahead residual，作为有效失败变体保留：S、S+C、S+C+F 在两档全部未过 gate。`815df546` 修正为每像素只计算一次后，最强 S+C+F 的 41 图结果也只有 Compact 3.818069 -> 3.716571 s（-2.658%，1/41 回退）和 LowLatency 3.772696 -> 3.652312 s（-3.191%，0/41 回退）。
- 预声明的 materialized residual + C+F 诊断隔离掉 S：Compact 4.015890 -> 3.778998 s（独立 -5.899%、配对 -3.515%、1/41 回退），LowLatency 3.787675 -> 3.654337 s（独立 -3.520%、配对 -3.479%、0/41 回退）。三轮与 3×MAD 标记全部保留；因为所有 screen 都未达到双档至少 10% 且零回退，正式 102×5 按 gate 主动跳过。
- F 使用每 block 1,049 个 cache-0 counter；16384² 最坏额外存储为 Compact 32.781 MiB、LowLatency 16.391 MiB，加 C 后约 33.406/16.547 MiB。真实最大图仅约 0.392/0.194 MiB，实测诊断 RSS 只增加 0.484/0.141 MiB；内存通过，性能失败。
- base/control/candidate 的 Default/Compact/LowLatency 共 306/306 在长度、SHA 与完整字节上相同；项目 decoder 全部 exact，pinned libwebp `733c91e` 为 918/918 RGBA exact。stable debug/release、all-targets、Clippy `-D warnings`、fmt、rustdoc/doctest 七项全通过；一键复现脚本已从仓库根实际运行并 exit 0，新生成的相对 SHA 清单全部通过。
- E37 相对 E33 已改善 46.320%/46.406%；叠加修正版 S+C+F 只投影到 47.747%/48.116%，叠加最强诊断也只到 49.487%/48.293%，都不能声称超过 50%。结论是“删除 pass 本身不够，必须删除或根本改变统计更新成本”；不建产品迁移树，不把研究 hooks 合入生产。
- 分支 `codex/vp8l-streaming-spatial-plan`：base `cec68762`，S+C `daadb6f1`，F `f5e5bee5`，修正 `815df546`，最终诊断 `292c1d74`，证据 `a2295c3d`，最终 HEAD `d2207f45`；工作树 [25a6](</Users/lance/.codex/worktrees/25a6/webp>)，task `019f8915-45d9-7a90-a843-4d0062ade22b`，[完整报告与复现器](../../experiments/vp8l-streaming-spatial-plan/REPORT.md)。

### E39：frequency-owned spatial clustering

优化点：让每个 spatial block 直接拥有最终 entropy group 所需的 1,049 个 exact counter，从计数派生类簇签名并 merge group frequencies，从而删除四路分支密集的 ordered Boyer–Moore census 和第二次 group-frequency token scan。Compact 用 `u16`，LowLatency 用 `u32`，都覆盖其 block 内最大 token-start 数。

- Phase A 的 102 图对称 A/B 路径先修正了一个重要偏置：ordered control 和 exact-frequency candidate 共享 prepare、exact-cost `SinglePlan`、strict fallback、candidate writer 和 packed writer，只改变 `SpatialPlan` 构建。修正后 Compact/LowLatency 的 ordered product 为 7.693/7.534 s，E/exact-symbol 为 4.742/4.548 s，改善 **38.357%/39.630%**；counter update 只占 0.690/0.687 s，证明新统计所有权模型成立。
- E 的 41 图同 binary 交错 screen：Compact 3.541526 -> **2.318275 s**（-34.540%），LowLatency 3.501564 -> **2.234421 s**（-36.188%），两档都是 0/41 编码回退。但 aggregate bytes 增加 0.423%/0.388%，最差单图 +5.841%/+5.058%，两档各有 8/41 超过 +2%，因而在高速度收益下仍硬性失败 rate gate。
- 唯一允许的 B 检查点不再搜参：将每通道 256 个 exact count 固定汇总成 8 个 32-symbol bin，取最大质量 bin。102 图 rate 预检查仍为 Compact +0.389%、LowLatency +0.419%，分别 15/102 和 14/102 超 +2%，最差 +6.422%/+7.503%。因此 B 不跑 screen，不允许第三种 signature，两个候选均未进 102×5 formal。
- Default 在 base/E37/P14-B 三个 archive 之间 102/102 全字节一致；Default/Compact/LowLatency 共 918/918 通过项目 decoder 与 pinned libwebp `733c91e` 完整 RGBA exact。七项 stable 质量门坎全通过；release rlib +37,736 B，研究 test binary +55,856 B。最坏 16384² 的 exact counters 为 32.781/16.391 MiB，screen RSS 为 +0.118%/-1.965%，内存不是拒绝原因。
- 两个失效运行的说明也被保留：早期 verifier 错误要求 fast-profile byte identity，以及一次 rustdoc shell 引号错误。它们都未开始或未正确表达目标验证，不影响 codec 正确性结论。一键复现已实际 exit 0，并在外部输出目录生成和校验该次运行自己的 `SHA256SUMS`。
- E 若只用 screen 比例叠加 E37，会得到相对 E33 的 64.861%/65.801% 投影；它不是 formal 测量，且候选未过 rate gate，所以不进顶部性能表、不声称产品突破。产品决策是拒绝 E/B 代码，仅保留“exact-frequency ownership 足够快，但 assignment objective 必须改变”这一架构结论。
- 分支 `codex/vp8l-frequency-owned-clustering`：base `3474599d`，E `c38e98aa`，对称 A/B `6703a163`，Phase A `9832274c`，B `2d529c33`，报告 `bb7002e9`，最终 checksum/HEAD `3468fcff`；工作树 [6d5d](</Users/lance/.codex/worktrees/6d5d/webp>)，task `019f8960-1a51-75a3-aec4-f99a1e7fb5de`，[完整报告与复现器](../../experiments/vp8l-frequency-owned-clustering/REPORT.md)。

### E40：exact-cost multi-proposal 与一次 entropy-aware refinement

优化点：沿用 E39 的 exact block-frequency ownership，同时让 E/B 两个 proposal 共用一个逐位精确的 `SpatialCostPlan`；从完整 RIFF 成本较低者出发，以当前 group Huffman code lengths 对每个 block 做一次确定性重分配，再精确比较 E、B、refined 与 single，只序列化最终胜者。

- Phase A 在锁定 102 图上得到 204/204 planner/writer bit、byte、RIFF 全一致，204/204 E/B selector 与实际较小流一致，204/204 public output 与最终选择一致。E39 离线 oracle 也精确复现：Compact 的 E/B 按图最小值比 ordered 小 0.099788%，LowLatency 小 0.049311%。
- 一次 refinement 将 Compact aggregate 从 E37 的 617,958,802 B 降到 **599,398,064 B**（-3.003556%），LowLatency 从 625,321,072 B 降到 **617,047,520 B**（-1.323089%）。Compact 0/102 超 +2%，LowLatency 仍有 3/102：`008`、`066`、`074`，最差 +7.007527%。
- 41 图同 binary screen 的 Compact encode 为 3.341012 -> **2.452132 s**（-26.605%，配对 -27.689%），aggregate bytes -3.482%，0/41 编码回退、0/41 超 +2%；LowLatency 为 3.177514 -> **2.240757 s**（-29.481%，配对 -28.773%），aggregate bytes -1.738%，0/41 编码回退，但图 008 稳定增长 **+4.338207%**，触发硬性 rate gate。
- 原始 screen 的 Rust decode 为 -1.700%/+1.179%，LowLatency 还略超 +1% 门槛；完整一键复现得到 -0.399%，说明该次 decode 回退不稳定。图 008 的 +4.338207% 在复现中完全相同，所以它是充分且可复现的最小拒绝原因；formal 102x5 按预声明规则未运行。
- screen 两套 decoder 各 246/246 RGBA exact；更宽的 Default archive 在 base/E37/P15 间 102/102 byte identity，项目 decoder 与 pinned libwebp `733c91e` 各 918/918 exact。七项 stable 质量门槛全部通过，无依赖、unsafe、线程、公开 API、Default、metadata、animation 或错误语义变化。
- exact counter update 在两档各约 0.61 s；proposal/cost/reassignment/rebuild 的归因全部保留。最大维度 counter 为 32.781/16.391 MiB，保守 research peak 小于 40 MiB；screen RSS 为 +3,260,416/-11,206,656 B，内存不是拒绝原因。release rlib 相对 E37 增长 112,104 B/+24.245%，研究 test binary 增长 50,048 B/+3.285%。
- 失败的 absent-symbol smoke、错误 libwebp 静态库路径、错误 cwd validation 和被公平 generator 取代的旧 screen 均具名保存在 `invalidated-runs`。仓库根一键复现已 exit 0，128 文件输出清单 hash 为 `70b7f41a…93e62`；研究目录 187 条相对 SHA-256 全部通过。
- 三个 LowLatency 长尾样本的 refined group 数仅为 11/7/5，而 profile 上限为 16；当前算法只能在已有组间移动，无法补回 proposal 过早折叠掉的 entropy capacity。这是下一架构要验证的机制性解释，不把它伪装成 P15 已证明的因果结论。
- 分支 `codex/vp8l-entropy-aware-spatial-clustering`：base `0e91e379`，实现 `eacad8bf`，Phase A `f78ca14e`，最终 same-binary control `7d14b835`，失败证据 `a52a3cce`，复现/最终 HEAD `76762d10`；工作树 [3cd9](</Users/lance/.codex/worktrees/3cd9/webp>)，task `019f899d-1871-7453-8450-630ffe00ecd1`，[完整报告与复现器](../../experiments/vp8l-entropy-aware-spatial-clustering/REPORT.md)。产品决定是拒绝，不迁移研究代码，顶部纪录表不变。

### E41：capacity-growing exact-cost split/refine clustering

优化点：从 E40 的 E/B/refined 完整 RIFF 胜者出发，用 support-safe 的 exact combined-histogram merge penalty 选择第二 seed；固定拆分最大 coding-regret group，分区后做一次全局 code-length reassignment。每个候选完整计入 nested group-map、五张 Huffman header、payload、extra bits、padding 与 RIFF，只接受严格减小的候选，首个不改善即停止。

- 锁定 102 图 Phase A 中，Compact 从 E37 的 617,958,802 B 降到 **547,448,078 B**（-11.410263%，相对 E40 -8.667026%）；LowLatency 从 625,321,072 B 降到 **601,400,998 B**（-3.825247%，相对 E40 -2.535708%）。两档最差逐图为 -1.468297%/+1.483745%，都是 0/102 超 +2%。Compact 102/102 由 split 胜出；LowLatency 的 E/B/refined/split/single 为 1/2/20/79/0。
- E40 的三个 LowLatency tail 都被容量增长修复：008 为 11→16 groups、-2.019478% vs E37；066 为 7→16、-3.129497%；074 为 5→16、-10.793940%。这支持“现有容量不足”机制，但仍不把 group 数写成一般因果规律。
- 全部 816 个 E/B/refined/split plan 和 204 个 single plan 的 predicted bits/bytes/RIFF 与 writer 一致；204/204 E/B selector、final selector 与 public output 都精确。每个已接受 split 严格降低完整 RIFF bytes。
- 最终 same-binary 41 图 screen 给出截然不同的 profile 结论：Compact 4.939605→8.846161 s，**+79.086400%** 且 40/41 回退；LowLatency 4.843859→2.560232 s，**-47.144794%** 且 0/41 回退。对应 aggregate rate 为 -13.092110%/-5.093032%，最差逐图 -3.169255%/-1.128722%。Rust decode +0.482386%/-1.444946%，pinned C +0.682189%/-0.830949%，RSS -35.476%/-35.638%；两套 decoder 都是 246/246 RGBA exact。
- Compact 执行 3,978 次 growth attempt、接受 3,967 次，扣除共享 counter update 后的增长归因约 15.34 s/102；LowLatency 336/336，约 0.49 s/102。差异来自 profile group cap 下的搜索规模，不是并发。16384² 的 counter/self-cost 加四个最大 plan 保守低于 40 MiB；release rlib +231,240 B/+11.000%，test binary +101,696 B/+4.800%。
- 因为预声明要求两档同时过 screen，完整 P16 方案在 Compact 硬失败后拒绝，102×5 formal 明确未跑。端到端复现再次得到 Compact +81.41%、LowLatency -48.00%，结论方向稳定；feature 278 tests、default 273 tests、两套 Clippy、fmt 与 215 项相对 SHA-256 均通过。失效的旧 test filter、缺 control 流、旧 binary、公平性修正、pinned 静态库路径与测试隔离运行全部保留。
- 分支 `codex/vp8l-capacity-growing-clustering`：base `ec7fbaf6`，预声明 `7641d33a`，实现 `31595aa3`，Phase A `0ba25f17`，scalar trace `cea589b5`，共享 E37 prepare `58327b09`，screen/证据 `e1b6c851`，最终 checksum/HEAD `13c0f2a1`；工作树 [5d9b](</Users/lance/.codex/worktrees/5d9b/webp>)，task `019f89e8-4f41-7b12-b14d-4da149d07b3a`，[完整报告与复现器](../../experiments/vp8l-capacity-growing-clustering/REPORT.md)。只归档负结果，不迁移研究代码；顶部纪录表不变。

### E42：multi-resolution exact-cost spatial portfolio

优化点：Compact 只构建 P15 的 128-block exact winner；LowLatency 从同一 prepared tokens 顺序构建 128/256 两个 exact winner、释放大 counter 后再建下一分辨率，并按完整 RIFF bytes 只写较小者。tie 固定选 256，single 在完整 RIFF tie 时仍严格胜出；没有 classifier、第三 block size、第二次 refinement 或参数搜索。

- 204 个 resolution row 的 planned/written bits、payload bytes 与 RIFF bytes 全一致；E/B/refined/winner 共 816 次逐流核验通过，102/102 resolution selector、102/102 Compact public output 与 102/102 LowLatency public output 都精确。
- Compact 精确复现 E40：599,398,064 B，较 E37 -3.003556%，0/102 超 +2%。LowLatency portfolio 为 **599,169,200 B**，较 E37 -4.182151%、较 E40 -2.897398%，但图 074 仍为 **+4.992654%**，因此 1/102 超 +2% 并在 Phase A 硬失败。
- LowLatency 仅 005/040/068 三图选择 256，99/102 选择 128；这 99 个输出与 Compact 逐字节一致。方案不是别名，但 97.1% 的选择退化为 Compact，同时仍支付两次 resolution plan 成本，缺乏稳定的 LowLatency 产品区分度。
- 共享 prepare 为 1.580540 s；128/256 exact counter update 为 0.609374/0.603181 s。顺序 counter ownership 把 16384² 保守峰值维持在 40 MiB 内；release rlib +176,976 B/+8.175%，test binary +99,952 B/+4.721%。由于 Phase A rate tail 失败，41 图 screen、双 decoder 性能与 102×5 formal 都未运行，不对速度作 headline 声明。
- 被错误 attribution、错误完整 SHA、跨 target-path binary identity 假设和缺 candidate rlib 影响的运行均具名保留。一键复现 exit 0，复现了两个 aggregate、074 失败、全部 exactness denominator 与 99/3 选择分布；最终 42 项相对 checksum 全部通过。
- 分支 `codex/vp8l-multires-spatial-portfolio`：base `ec7fbaf6`，实现 `41c24db5`，归因修正 `bdb709ea`，Phase A/报告 `c151f06b`，最终 checksum/HEAD `9a8b7d23`；工作树 [dfbc](</Users/lance/.codex/worktrees/dfbc/webp>)，task `019f89e8-dcd1-7a43-ba7b-a8406d10740e`，[完整报告与复现器](../../experiments/vp8l-multires-spatial-portfolio/REPORT.md)。产品决定是拒绝，不迁移研究代码，顶部纪录表不变。

### E43：profile-specialized exact-cost hybrid

优化点：把 E40 与 E41 证明出的适用边界变成固定的 profile 架构，而不是运行时分类器。Compact 只执行 exact E/B、一次 entropy-aware reassignment 与 exact winner selection，完全不构建 growth state；LowLatency 从 exact winner 出发执行 deterministic capacity growth。两档共享 prepare/tokenization、exact block-frequency ownership、逐位精确的 `SpatialCostPlan`、strict single fallback 与 selected-only packed writer。实现保持 safe Rust、单线程，无图 ID、语料阈值、参数搜索、新依赖或公开 API/Default 行为变化。

- 锁定 102 图 Phase A 精确复现预声明机制：Compact **599,398,064 B**，较 E37 -3.003556%，最差 +1.490531%，0/102 超 +2%，growth 0/0；LowLatency **601,400,998 B**，较 E37 -3.825247%，最差 +1.483745%，0/102 超 +2%，growth 336/336。714 个 spatial planner/writer row、204 个 single plan、204 个 E/B selector、204 个 final selector 与 204 个公开输出全部精确。
- 同一最终 binary `05b8421c…64c9` 的 41 图预载、warmup+3 交错 screen 中，Compact 5.096220→**2.453193 s**（-51.862488%），LowLatency 4.945387→**2.432862 s**（-50.805420%），两档都是 0/41 编码回退。aggregate rate 为 -3.482487%/-5.093032%，0/41 超 +2%；Rust decode +0.893292%/+0.407526%，pinned C -0.768430%/-0.438525%，均过 1% gate；RSS 分别下降 35.246%/35.604%。项目 decoder 与 pinned C 各 246/246 RGBA exact。
- 锁定 102×5 formal 仍使用该 binary：Compact candidate 中位 **5.721840 s**、较 control -52.387558%；LowLatency **5.821272 s**、-51.400923%。两档均 0/102 逐图中位回退，并低于 7.1/6.9 s 绝对上限；没有删除样本或离群值。
- 最终六布局归档中，项目 decoder 与 pinned libwebp `733c91e` 各 **612/612 exact**；same-source no-feature control 408/408 exact，feature/no-feature Default **102/102 byte-identical**。default/feature workspace all-target tests、两套 Clippy `-D warnings`、fmt、rustdoc 与 doctest 全过。最大尺寸的 counters/caches/plans 保守低于 40 MiB；生产模块均低于 500 行，无依赖、unsafe、线程、metadata、animation 或错误语义变化。
- 隔离 target 的完整复现脚本 exit 0，Phase A、screen、formal、双 decoder、Default identity 与九项 stable quality 命令全部重跑；因 Rust binary hash 含 target-path 元数据，复建 SHA 为 `2e5f7b11…e570`，但该 SHA 在复现的每个阶段保持一致，rate 与输出流逐字节重现。外部复现输出的 185 项 checksum 与研究分支证据的 242 项 checksum 均通过；仓库只归档报告、摘要、provenance、复现器和失效运行说明，按当前政策忽略可再生 raw 输出与 checksum manifest。
- 分支 `codex/vp8l-profile-hybrid-clustering`：base `58f7b8dd`，设计 `3dea69cc`，实现 `a0606a83`，完整归因 `36ad7acd`，Phase A `6230c4a0`，screen `483ad4da`，formal `9f97bf12`，产品 gate `5c1f7b00`，复现修正 `4bc28f4a`，最终证据/HEAD `c04bed7b`；工作树 [7d78](</Users/lance/.codex/worktrees/7d78/webp>)，task `019f8a34-8286-70d2-84ba-461fbb4117d5`，[完整报告与耐久证据](../../experiments/vp8l-profile-hybrid-clustering/REPORT.md)。结论是通过研究 gate，但不直接合入研究代码；顶部产品纪录表要等独立 latest-main 产品迁移复现后才更新。

### E45：profile hybrid 最小产品迁移

优化点：从创建时 latest `main` 手工重建 E43 的最小生产职责，不 merge/rebase/cherry-pick P18。私有模块分别拥有 exact block counters、histogram cost、proposal clustering、spatial refinement、complete-RIFF plan 与 profile orchestration；Compact 类型不含 growth/self-cost storage，LowLatency 才拥有对应状态。research feature、timer、trace、census、公开 hook 和新依赖均未进入 release 路径。

- 最终 product binary `9aa8fa08…a29f8` 的 102 图 Phase A 完全通过：Compact **599,398,064 B**、较 control -3.003556%、0/102 超 +2%、growth 0/0；LowLatency **601,400,998 B**、-3.825247%、0/102 超 +2%、growth 336/336。两档各 102/102 与 P18 candidate 字节一致；Compact/Low spatial planner-writer 306/306、408/408，single、E/B selector、final selector、public selected stream、strict fallback 与 P18 identity 均 204/204。
- 锁定 41 图同 binary screen 中，Compact control/product 中位 5.025098/2.488329 s，改善 **50.481976%** 并通过；LowLatency 为 4.932338/2.555452 s，只改善 **48.189859%**，比预声明的 ≥50% 硬门少 1.810141pp，因此产品方案拒绝。两档仍是 0/41 编码回退，aggregate rate -3.482487%/-5.093032%，0/41 超 +2%，RSS -35.331%/-35.605%。
- 项目 decoder 与 pinned libwebp `733c91e` 各 246/246 RGBA exact，所有 benchmark stderr 为空。Rust decode -0.769314%/-0.021578%，pinned C -0.379723%/-1.018441%；LowLatency 除 aggregate encode threshold 外的所有 screen gate 均通过。三轮样本全部保留，包含被 3×MAD 标记的第三个 LowLatency candidate 样本 2.657632 s，没有删样本或补考。
- 独立 postmortem 将 P18 与 P20 的同 corpus screen 分开比较：LowLatency control 只变化 -0.263854%，但 candidate 从 2.432862 增至 2.555452 s，慢 **5.038897%**，缺口在 candidate 侧而不是更快的基线。源码差异还显示 P18 的 sparse histogram merge 先判断 counter 非零再做 checked add，而 P20 对每个 1,049-slot histogram 无条件执行 checked add；LowLatency 的 336 次 growth/rebuild 会放大该差异。这是下一实验的固定因果假设，当前尚未通过 A/B，不能写成已证明原因。
- 产品静态审计未发现 dependency、feature、公开 API、unsafe、thread、classifier 或 Default routing 变化；release rlib +238,080 B/+9.95594%，锁定 test/reproducer binary +84,368 B/+3.83499%。library tests 293 passed/4 ignored、all-target Clippy `-D warnings` 与 fmt 通过。formal 102×5、最终 102 图全布局、Default identity、完整 workspace/docs 与 root replay 按 stop rule 均未运行，不作相应通过声明。
- 分支 `codex/vp8l-profile-hybrid-product`：base `66c15f11`，设计 `09863f08`，产品实现 `67bd0427`，audit `efa186cc`，最终布局 `9ad4afbe`，Phase A `2cdd2293`，screen runner `4d0069a2`，失效 filter 修正 `f09122b1`，负报告/HEAD `cebc0981`；工作树 [5020](</Users/lance/.codex/worktrees/5020/webp>)，task `019f8a85-c530-79d2-af1f-2b54105574be`，[完整报告与耐久证据](../../experiments/vp8l-profile-hybrid-product/REPORT.md)。不迁移产品代码，顶部纪录表不变。

### E46：zero-eliding sparse histogram merge recovery

优化点：选择性复用 E45 的 dense product control，只改变 exact block histogram 合并中的一个规则。A 对 1,049 个 source slots 全部执行 checked add；B 在 source counter 为零时跳过 add。A/B 分派每个 histogram 只做一次，slot 内没有 variant switch；census 是 thread-local test-only 且在计时时关闭，release 路径始终保持 dense A。

- Dense A 在引入 B 前先通过基线：锁定 102 图 Compact/LowLatency 为 **599,398,064 / 601,400,998 B**，204/204 profile streams 的 size 与 stream hash 都匹配重建 P18 oracle，stderr 为空。第一次直接 `cmp` 指向了不保留 stream 文件的历史 replay 路径，被明确归档为 path-resolution invalidation，不计为 mismatch。
- 机制 census 证明数据高度稀疏：两档共访问 105,647,937 slots，其中 76,374,722 为零，理论 elision **72.291731%**。Compact 为 50,676,141 visits、74.920198% 零；LowLatency 为 54,971,796、69.868660% 零，其中 growth 独占 37,281,460 visits、71.202954% 零。
- empty/sparse/dense/max/overflow-adjacent、literal/copy、`u16`/`u32` 与 deterministic plan 的 A/B 单元/属性矩阵通过。锁定 102 图上 A/B、public、P18 identity 各 204/204；Compact/Low spatial planner-writer 306/306、408/408，E/B/final selector 与 strict fallback 各 204/204，growth 0/0 与 336/336，所有 stderr 为空。
- 唯一合法的 41 图 recovery screen 预载后运行 warmup+F/R/F，保留全部样本且未补考。Compact A/B 中位 2.504255/2.505112 s，B **慢 0.034190%**，仍在允许的 +1% aggregate 内；LowLatency 2.580750/2.591843 s，B **慢 0.429849%**，相对要求的 ≥3% 改善反向失败，并有 23/41 逐图中位回退。82/82 A/B 输出字节一致，stderr 为空。
- 结论不是“稀疏度不足”，而是本标量布局中每槽可预测 zero branch 未能抵消 dense checked-add loop 的流水线优势。预声明 stop rule 随即生效，B 没有进入 release route，产品 Phase A、latest-main product screen、formal、最终 correctness/quality/resource/replay 全部未跑，不作通过声明。
- 分支 `codex/vp8l-sparse-histogram-merge`：base `8485fc05`，设计 `c57e7eac`，授权 P20 transplant `1746c7bd`，dense identity `52ccccad`，机制 `60dc7c99`，锁定 runner/measurement `a07b3d21`，负报告/HEAD `6f82035d`；工作树 [1841](</Users/lance/.codex/worktrees/1841/webp>)，task `019f8aba-a8d0-73e3-b1b8-434634e9eea6`，[完整报告与耐久证据](../../experiments/vp8l-sparse-histogram-merge/REPORT.md)。不迁移产品或 test-only oracle，顶部纪录表不变。

### E47：metric-only search / final-plan materialization recovery

优化点：A 保持 E45 的 retained-full-plan 搜索；B 对每个成功构建的 `SpatialCostPlan` 立即移动出 `PlanParts + PlanMetric` 并释放 encoded prefix 与 `Vec<EntropyTables>`，最终空间流胜出后只重建一次完整 plan。A/B 之外的 proposal、exact cost、growth、tie、fallback 和 writer 均不变，release 始终保持 A。

- 引入 B 前，A 精确复现 P18：Compact/LowLatency 为 **599,398,064 / 601,400,998 B**，204/204 profile stream 的 size 与 hash 一致。Phase R 中 A/B、public/A、P18、E/B selector、final selector 与 strict fallback 均 204/204；planner/writer 为 Compact 306/306、LowLatency 408/408，growth 0/0 与 336/336，stderr 全空。
- 生命周期机制确实生效：Compact 峰值 full plan/table/prefix/估算 heap 从 3/141/84,372 B/725,604 B 降为 1/49/29,292 B/252,124 B；LowLatency 从 3/48/29,844 B/248,100 B 降为 1/16/9,948 B/82,700 B。所有 `PlanParts` clone 为 0，但 204 个最终空间胜者都需要额外 materialization。
- 唯一合法 recovery screen 保留 warmup+3 轮 F/R/F。Compact A/B 中位样本为 2.493447/2.493782/2.479023 与 2.518911/2.516327/2.511623 s，B **慢 0.917607%**、34/41 回退；LowLatency 为 2.654492/2.561734/2.576800 与 2.611756/2.578878/2.574910 s，B **慢 0.080641%**、31/41 回退，反向失败 ≥3% 与 0/41 两项 gate。82/82 输出逐字节一致，未补考。
- 结果否定的是“保留对象造成 5% 缺口”这一因果假设：单独缩短生命周期没有端到端收益，额外最终重建抵消了释放收益。产品 Phase A/screen/formal、最终 correctness/Default/resource/replay 按 stop rule 全部禁止；允许的 library tests 299 passed/4 ignored、all-target Clippy `-D warnings` 与 fmt 通过。
- 分支 `codex/vp8l-metric-only-plan-search`：base `4280a59a`，设计 `479a5149`，P20 control `5d44c41d`，A 证据 `37f1f563`，机制 `60719703`，锁定 runner/measurement `688452ec`，负报告/HEAD `4b80999f`；工作树 [c5fc](</Users/lance/.codex/worktrees/c5fc/webp>)，task `019f8add-4346-70a2-a831-530db819cb8f`，[完整报告与耐久证据](../../experiments/vp8l-metric-only-plan-search/REPORT.md)。不迁移控制或实验代码，顶部纪录表不变。

## 下一阶段：优先寻找更强且更通用的新架构

标准 VP8L 的局部优化已经给出一致信号：Huffman、predictor、LZ77、PGO、单个 copy kernel 各自只有个位数收益或以明显 rate/latency 回退换取收益。后续优先级应从“继续打磨一个旧循环”转向能够同时改变表示、依赖图和输出流水线的架构方案。

### 第一优先：analytic exact-cost / selected-only materialization

- E47 证明只减少 full-plan 同时存活数量没有收益；真正仍被每个 E/B/R/growth 候选重复支付的是 `SpatialCostPlan::build` 本身。当前 exact-cost 为了得到 bit/RIFF 长度，会把 nested group map 和每组五张 adaptive Huffman 表真实写入临时 `BitWriter`，保存 prefix 与全部 `EntropyTables`，随后绝大多数候选被丢弃。LowLatency 的 336 次 growth 会重复这段与最终 wire 等价、但不产生产品输出的序列化工作。
- 下一独立树固定 A 为 E45 完整 plan costing；B 使用解析式 exact metric：nested map 仍按同一 tokenization/频率/表生成规则计算，table header 与 token payload 逐位计数，group tables 逐组构建、计数并立即释放，不生成 candidate prefix bytes、不保留 candidate table vector。只有最终空间胜者调用一次既有完整 builder/writer。禁止改变 proposal、Huffman code、cost、tie、growth、rate 或 wire。
- 首先证明 analytic metric 与真实 builder 对 E/B/R/每个 Split 的 payload bits/bytes/RIFF/group count 全相等，并证明最终 204 个公开流与 A/P18 逐字节一致；census 必须量化候选临时写入 bits、table builds、分配与最终 materialization。若解析式计数不能覆盖相同 overflow/长度边界，或出现一处 metric/selection/wire 漂移，立即拒绝。
- 41 图同 binary recovery 仍只允许一轮 warmup+3 F/R/F；要求 LowLatency 相对 A 至少快 5%，Compact 不回退且两档 0/41 逐图中位回退。通过后固化 B，再从头跑双档 ≥50% product screen、102×5 formal、双 decoder、Default identity、stable quality、资源/size 与隔离 replay；失败则归档，不在同一树混入 histogram layout、参数或并发。

### 第二优先：统一的 Fast Representation v2

下一版不应只是给 v1 增加更多 enum，而应拥有明确的数据所有权和可扩展 framing：

- 以 row-group/tile 为独立压缩单元，目录记录 offset、compressed length、decoded length、transform、codec 和 checksum。
- Zstd-Sub 作为实用默认候选，LZ4 作为极速候选；允许按图或按 tile 选择，而不是强制全图一种 codec。
- 原生支持 RGB、RGBA，或 RGB + 独立 alpha plane；transparent、tiny、极宽/极高必须进入相同协议与 limit 模型。
- 解压、逆变换、RGBA 写出、校验继续融合；tile framing 同时提供有界 scratch、真 streaming、随机跳过和未来并行能力，但单线程必须先达标。
- 标准 VP8L fallback 继续完整保留；私有表示失效必须回退，不能改变公开 decoder 的默认容错。
- v1 的 CRC 只证明私有 payload 自洽，不能证明它与 fallback 像素相同。对外部不可信文件，必须采用可信 ingest 时双解码校验并缓存认证结果，或建立可验证的签名/manifest 信任模型；仅在同一不可信文件里增加另一个 hash 不能解决恶意双表示问题。

### 第三优先：可部署的自动选择器

泛化实验中的 oracle selector 不能进入产品。下一步应只用编码时可得特征预测：

- m6 bytes、raw bytes、alpha class、颜色/梯度/重复统计、候选压缩前缀采样、预计工作量与目标输入带宽。
- 目标函数必须是 `decode_time + extra_bytes / bandwidth`，并同时受单文件和整个 asset pack 的体积预算约束。
- 先在 train 拟合，在固定 disjoint test 决策；最终结果不能通过实际解码候选来“预测”自己。
- 网络/冷盘、内存缓存和本地预载包必须是不同 product policy。LZ4 不应因内存成绩好而自动进入网络分发档。

### 第四优先：统一 latest-main FDEC 证据

当前最重要的缺口不是再解释旧数据，而是创建一棵新的、挂分支的 latest-main 工作树，将 E16 的最终融合实现与 E17 的 229 图 harness 合并后重跑：

1. 同一最终二进制交错测 pinned C libwebp、标准 Rust m0/m3/m6、Zstd、LZ4。
2. 同时报告完整 229 图与 CLIC validation/disjoint/category 分布。
3. 补 x86-64 Linux、Windows、WASM build；性能至少覆盖 arm64 与 x86-64。
4. 将 raw CSV、manifest/hash、命令、branch、commit 和 worktree 位置回填本 README；只有刷新纪录的结果进入顶部表。

## 每次新实验的登记模板

```text
Date:
Task/thread id:
Hypothesis and owned invariant:
Latest main base SHA:
Branch:
Worktree:
Final HEAD / commits:
Report and raw-data paths:
Corpus identity and manifest hash:
Host / OS / CPU / toolchain:
Format/profile and compatibility class:
Images / streams / compressed bytes / decoded pixels / RGBA bytes:
Threads / preload policy / timed work:
All raw rounds and median:
Pinned libwebp same-input result and gap:
Encode or append time:
Phase breakdown:
Peak RSS and modeled working peak:
Project exact / pinned libwebp exact / mutation and limit tests:
Result: promote / benchmark-only / reject and roll back
Top-table action: add / replace / none
```
