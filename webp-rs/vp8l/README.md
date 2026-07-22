# VP8L 性能总账

| 纪录类别 | 实现 / profile | 图 / 流 | 解码线程 | 输入或容器 bytes | 输出 RGBA bytes | 中位时间 | 输入 MB/s | RGBA MB/s | MP/s | 相对 pinned libwebp | 相对 m6 体积 | 正确性 | 可追溯位置 |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | --- |
| libwebp 基准 | pinned libwebp m0 | 102 / 102 | 1 | 290,266,556 | 1,007,432,548 | 4,776 ms | 60.8 | 210.9 | 52.7 | 基准 | +9.526% | 102/102 | `733c91e`；[quality gates](../../docs/quality-gates.md) |
| libwebp 基准 | pinned libwebp m3 | 102 / 102 | 1 | 267,917,268 | 1,007,432,548 | 4,881 ms | 54.9 | 206.4 | 51.6 | 基准 | +1.093% | 102/102 | `733c91e`；[quality gates](../../docs/quality-gates.md) |
| libwebp 基准 | pinned libwebp m6 | 102 / 102 | 1 | 265,020,980 | 1,007,432,548 | 4,777 ms | 55.5 | 210.9 | 52.7 | 基准 | 基准 | 102/102 | `733c91e`；[quality gates](../../docs/quality-gates.md) |
| libwebp 基准 | pinned libwebp m0+m3+m6 | 102 / 306 | 1 | 823,204,804 | 3,022,297,644 | 14,363 ms | 57.3 | 210.4 | 52.6 | 基准 | 三种标准流合计 | 306/306 | `733c91e`；[backend record](</Users/lance/.codex/worktrees/4c95/webp/tools/vp8l-backend-bakeoff/RESULTS.md>) |
| 标准 VP8L 纪录 | 当前 Rust，m0+m3+m6 | 102 / 306 | 1 | 823,204,804 | 3,022,297,644 | 14,009 ms | 58.8 | 215.7 | 53.9 | **快 2.5%**，同输入同轮次 | 相同 | 306/306 | main lineage；最初记录于 `eca32b4` |
| 标准 VP8L 自编码流纪录 | Rust `fast_no_cache` | 102 / 102 | 1 | 724,306,686 | 1,007,432,548 | 2,613 ms | 277.2 | 385.5 | 96.4 | 约快 45.3%†，相对 m6 C 基准 | +173.302% | 102/102，两套 decoder | `codex/vp8l-fast-decode-profile@232a32c`；[report](</Users/lance/.codex/worktrees/c68f/webp/docs/vp8l-fast-decode-research.md>) |
| 私有兼容表示实用档纪录 | FDEC Zstd-1 / RGB / Row-Sub，融合输出 | 102 / 102 | 1 | 663,622,132 | 1,007,432,548 | 923.689 ms | 718.4 | 1,090.7 | 272.7 | 约快 80.7%†；同轮 Rust m6 快 81.8% | +150.404% | 102/102；libwebp fallback 102/102 | `codex/fdec-hot-path-migration@ba4b530`；[report](</Users/lance/.codex/worktrees/a386/webp/docs/fdec-hot-path-migration.md>) |
| 私有兼容表示极速档纪录 | FDEC LZ4 / RGB / none，融合输出 | 102 / 102 | 1 | 935,997,910 | 1,007,432,548 | **416.581 ms** | 2,246.9 | 2,418.3 | 604.6 | 约快 91.3%†；同轮 Rust m6 快 91.8% | +253.179% | 102/102；libwebp fallback 102/102 | `codex/fdec-hot-path-migration@ba4b530`；[report](</Users/lance/.codex/worktrees/a386/webp/docs/fdec-hot-path-migration.md>) |
| 单图流水线纪录 | entropy producer + transform consumer | 102 / 306 | 2 | 823,204,804 | 3,022,297,644 | 9,375 ms | 87.8 | 322.4 | 80.6 | 快 34.7% | 相同 | 306/306 | `codex/vp8l-single-image-pipeline@66356c6` |
| 批量吞吐纪录 | 当前 Rust，jobs=12 | 102 / 306 | 12 | 823,204,804 | 3,022,297,644 | **2,842.808 ms** | 289.6 | 1,063.1 | 265.8 | 快 80.2%；但不是单图 latency | 相同 | 306/306 | `codex/vp8l-batch-parallel-ab@664d142`；[report](</Users/lance/.codex/worktrees/ffb9/webp/docs/vp8l-batch-parallel-benchmark.md>) |

顶部表只保留 pinned 基准，以及在自己的可比类别中刷新时间纪录或形成明确速度/体积 Pareto 的结果。被后续结果完全支配、仅改善内存但降低速度、未通过正确性、或只产生诊断信息的实验只进入下方实验账本。

`MB/s` 使用十进制 MB，所有主解码时间均排除文件读取和进程启动，输入先载入内存，输出 RGBA 完整分配、写出并参与校验。`MP/s` 按 RGBA 像素数计算。标有 `†` 的 FDEC/自编码流相对 libwebp 数字使用历史 pinned C 基准作为固定参考，不是同一最终二进制对相同候选容器的交错 A/B；下一次统一基准必须补齐这一列。FDEC 的 `306` 结果若出现，是同一 102 图 profile 重复三次的等价投影，不应写成 306 个不同码流。

### 纪录的资源与产品成本

| Profile | Encode / append | 标准 fallback | 私有 payload | 最大 decode working peak | 已观测进程峰值 / 增量 | 依赖或二进制成本 | 完整附加 I/O break-even | Alpha 加速覆盖 |
| --- | ---: | ---: | ---: | ---: | --- | --- | ---: | ---: |
| pinned libwebp m6 | 未测 | n/a | n/a | 未分离 | aggregate live-allocation 下界 835,656,644 B | pinned C static library | 基准 | 由标准 VP8L 覆盖 |
| Rust `fast_no_cache` | 未保留可比 encode 计时 | n/a | n/a | 未分离 | 799,277,056 B RSS | 默认 safe Rust workspace | 未实测 | 本轮 CLIC 为 opaque |
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

根任务：`019f8321-035e-7211-8f53-987e18891c8c`。下表覆盖该任务已经收口的 32 个 VP8L/FDEC 实验、验证与产品迁移任务；更早的 `vp8l-huffman-paper-feasibility` 属于另一根任务，未混入这份计数。一个假设若因系统中断另建 latest-main 产品迁移树，两棵树分别登记，避免把诊断提交误认成产品 HEAD。

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

### 进行中的 latest-main 产品迁移

E31/E32 均从各自创建时最新的本地 `main@11f6f669215479848628c1bdcd438c2a891e96fb` 建树；E32 通过后没有直接合入，而是按规则从届时最新 `main@52c6b8fc64cd86b4fccd0f30fb996d825a6dd2ec` 新建 P08。远端 `origin/main@5e54dd3` 仍是旧祖先，不得用于替换本地基线。

| 暂存 ID | 假设 | 分支 | 工作树 / task | 当前 gate |
| --- | --- | --- | --- | --- |
| P08 | coarse spatial stable profiles | `codex/vp8l-coarse-spatial-product` | [070b](</Users/lance/.codex/worktrees/070b/webp>)；task `019f87f5-d9a0-7281-a319-5d6e4a1fc510` | 从最新 main 迁移 128/64 compact 与 256/16 low-latency 档；保持默认 API/输出不变，独立复验中 |

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

## 下一阶段：优先寻找更强且更通用的新架构

标准 VP8L 的局部优化已经给出一致信号：Huffman、predictor、LZ77、PGO、单个 copy kernel 各自只有个位数收益或以明显 rate/latency 回退换取收益。后续优先级应从“继续打磨一个旧循环”转向能够同时改变表示、依赖图和输出流水线的架构方案。

### 第一优先：coarse spatial 产品化与单流写出

- P08 从创建时最新 `main@52c6b8fc` 迁移 E32，而不是复用旧 worktree：默认 `encode_lossless_rgba` 和 metadata/animation 输出保持逐字节不变，128/64 compact 与 256/16 low-latency 只能由显式稳定 options 选择。
- 产品树必须重新生成标准流并运行项目/pinned libwebp exact、同二进制正式 A/B、public default 与 m6 的绝对差距、跨目标构建和 API 文档审计；只有这些完成后才能进入顶部纪录表或 main。
- 产品迁移稳定后另开 latest-main 实验，把 Huffman 表和 token code length 汇总为精确 bit-cost plan，在不写 losing RIFF 的前提下保留逐图 size fallback；目标是维持 8%–9% 体积收益，并把约 2.3x 编码成本压回接近单流。

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
