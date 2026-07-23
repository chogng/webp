# 标准 VP8L 架构设计与研究处置

状态：**目标架构；以标准 VP8L 为唯一产品格式**

本文把 VP8L 性能研究收敛为下一阶段可执行的架构契约。完整原始指标、实验分支、
提交与报告位置仍由 [`performance/vp8l.md`](performance/vp8l.md) 维护；本文不复制
每次实验的全过程，只回答四个问题：

1. 一个优秀的 Rust VP8L 实现应当拥有怎样的数据流与所有权；
2. 已验证的哪些机制已经进入 `main`，哪些值得在 latest `main` 上重建；
3. 哪些实验路径应停止或视为 deprecated research；
4. 如何在不牺牲格式、压缩率、内存和泛化性的前提下与 pinned libwebp 持平或领先。

## 1. 最终产品边界

产品目标不是私有快速表示，也不是用缓存后的预解码数据替代 VP8L。最终边界固定为：

- 输入和输出均为标准 WebP/VP8L；pinned libwebp 必须能够解码项目生成的流。
- `Default` 的公开行为、容错、metadata/animation 路径和 wire semantics 默认不变。
- `FastDecodeCompact` 与 `FastDecodeLowLatency` 是标准 VP8L 的显式 Pareto profiles，
  不是新的文件格式。
- 编码性能只能在相同或更小输出体积下与基线比较；解码性能必须使用同一批标准流。
- 单线程单图 latency 是算法主指标；批量并行和单图 producer/consumer 流水线单独记账。
- FDEC、FDC2、FDCS 和 prepared sidecar 只保留为研究证据，不进入标准 VP8L 产品路线。

性能目标不再设为任意的“必须提升 50%”。与成熟 libwebp 的合理目标是：

1. 先在跨域 corpus 的每个类别上持平；
2. 再争取 10%–20% 的稳定单线程优势；
3. 同时改善或至少不恶化内存与压缩率；
4. 只有真正删除完整 pass、重复 materialization 或大块中间表示时，才期待 30%–50%。

## 2. 当前事实基线

当前已封口的可比事实如下。百分比只在各自明确的比较口径内成立。

| 维度 | 当前结果 | 解释 |
| --- | ---: | --- |
| 标准 Rust decoder，m0+m3+m6 | 14.009 s | 306 个固定标准流 |
| pinned libwebp decoder | 14.363 s | Rust 在该 CLIC corpus 快 2.5% |
| 最初 Rust decoder | 20.863 s | 当前累计改善约 32.9% |
| Compact 相对 public Default | 体积 -6.609%，Rust decode -19.351% | 标准 VP8L Pareto |
| LowLatency 相对 public Default | 体积 -5.497%，Rust decode -19.845% | 标准 VP8L Pareto |
| Compact/LowLatency 相对 pinned m6 | 体积 +133.174%/+135.952% | 不能冒充 m6 压缩率替代品 |
| exact-cost single-write | encode -28.389%/-28.966% | 已进入 `main` |
| packed token writer | encode -27.005%/-26.561% | 已进入 `main` |
| E33 初版到 E37 最终阶段记录 | 约 -46% | 跨阶段趋势，不冒充单次 A/B |

这说明编码器已经证明“改变工作所有权”可以获得大收益；标准 decoder 的后续局部实验
则大多只有 0%–4%，需要新的 buffer/table/kernel 架构，而不是继续叠加旧循环补丁。

## 3. 架构原则

### 3.1 计划与执行分离

编码候选先产生不可变 plan，不产生完整 `Vec<u8>`：

```text
pixels
  │
  ├─ one-pass analysis ── token/statistics IR
  │                             │
  │                     bounded candidate plans
  │                             │
  └──────────────────── exact rate + decode-work selection
                                │
                         one winning plan
                                │
                       one materialization
                                │
                       packed bitstream sink
```

控制面拥有候选、成本、回退和预算；数据面只执行已选 plan。writer 不负责生成两个完整
结果供上层比较，hot loop 不携带 profile 决策。

### 3.2 验证与热路径分离

decoder 在边界上完成 dimensions、transform chain、Huffman tables、group map、
backward-reference limits 和 allocation limits 的验证，再生成不可变执行计划：

```text
VP8L bytes
   │
validated header / transform / entropy ownership
   │
DecodePlan
   │
symbol and LZ77 kernels
   │
transform pipeline
   │
single owned pixel backing
   │
RGBA
```

安全检查不能删除，只能从 per-symbol/per-pixel 循环提升到一次性边界。任何 narrow
`unsafe` 或 SIMD 都必须位于已验证 plan 之后，具有 scalar fallback 和差分测试。

### 3.3 一次扫描、一次所有权

- 像素分析、tokenization、block statistics 和 entropy frequencies 共享一次输入扫描。
- token stream 只有一个 owner；候选只持有索引、统计和 plan，不复制 token。
- 最终输出 allocation 只有一个 owner；中间 layout 变化不应重新分配另一张完整图。
- scratch 的 owner、上界和生命周期必须由模块不变量表达，不能只靠 benchmark 注释。
- 并行分析只在单线程架构稳定后加入，且确定性合并不能改变 bytes。

## 4. 编码器目标架构

### 4.1 共享分析 IR

当前 `webp-rs/encode/src/vp8l/mod.rs` 同时拥有 transform 选择、tokenization、
frequency、table 和最终写出。下一次拆分应按真实所有权进行：

```text
vp8l/
├── mod.rs                       public orchestration and explicit re-exports
├── token_stream.rs              canonical tokens and source geometry
├── token_stream_tests.rs
├── transform_portfolio.rs       bounded transform candidates and invariants
├── transform_portfolio_tests.rs
├── entropy_plan.rs              exact Huffman plans and bit costs
├── entropy_plan_tests.rs
├── encode_portfolio.rs          budget, winner, strict fallback
├── encode_portfolio_tests.rs
├── packet_writer.rs             all-profile token packets and bit sink
├── packet_writer_tests.rs
├── spatial_plan.rs              spatial grouping only
└── spatial_plan_tests.rs
```

这不是要求立即按文件名机械搬运。只有当 state、算法、caller 与 invariant 能形成单向
依赖时才拆分；`lib.rs`/`mod.rs` 继续只保留文档、私有模块声明和明确 re-export。

共享 IR 至少拥有：

- canonical `EntropyToken` 序列；
- literal/copy/cache 与 distance census；
- block/transform sufficient statistics；
- channel frequencies 与 group frequencies；
- geometry、alpha、palette 和 resource limits；
- 可复算的 input identity。

候选不得重新扫描 RGBA 或重新生成 token。

首个 Phase B 生产切片已经把这条边界落到当前编码链路：

- 私有 `token_stream.rs` 是 canonical `EntropyToken`、source geometry、color-cache
  contract、literal/cache/copy/distance census 和全局 channel frequencies 的唯一 owner；
- `TokenStream` 构造完成时校验 token 数、覆盖像素数、copy/distance 一一对应以及五组
  frequency totals，内部事实不一致时 fail closed；
- Default/palette writer、fast-profile `Prepared`、`SinglePlan` 与 `SpatialPlan` 都借用
  同一个 stream/statistics object；plan 不再分别接收可失配的 token `Vec`、geometry
  与裸 frequency；
- token span 和 token-to-frequency 规则各只保留一份，spatial cluster/group
  frequency 从 canonical stream 派生，不重新扫描 RGBA 或重新 tokenization。

Phase B 的第二个生产切片在 `main` 上补齐了这条所有权边界：

- `source_analysis.rs` 一次 source-domain 扫描拥有 geometry、alpha/transparent census、
  palette cardinality/index resource、固定 transform sufficient statistics，以及由
  RGBA byte length 与 FNV-1a 组成的可复算 input identity；
- `TokenStream` 直接流式计算 residual 与 distance-one run，不再 materialize 一份
  `pixels × 4` residual image；四个 bounded color-cache 候选在同一 transform-domain
  扫描内计数，不再各自扫描整图；
- spatial block histogram 在 canonical token 构造时收集，`SpatialPlan` 只接受匹配
  block size 的共享 statistics，不再重新遍历 token 构建第二份 census；
- palette/indexing 与非 palette transform 决策都从同一 `SourceAnalysis` 取得。

为了保持已实测更快的访问局部性，获选的 color transform 仍 materialize 一份
transform-domain RGBA backing；predictor/cache/token 三个算法阶段借用该 backing。
这不是候选重复分析，且不应误写成“整个编码器只读输入一次”。Phase C 的通用 exact
plan 已删除最后一项失败 payload materialization。起点 archive 与当前树在 16 张
CLIC m6 source 的 Default/Single/Compact/
LowLatency 共 64 条 stream 上逐字节相同；41-file MustAccept VP8L、3 轮端到端编码从
743.435 ms 降至 731.267 ms（-1.636%），输出 bytes 与 checksum 不变。

### 4.2 通用 exact plan

当前 `single_plan.rs` 已证明精确成本可以替代必输码流的完整写出。应把机制提升为
所有候选共享的 `EntropyPlan`：

- 保存 canonical Huffman lengths/tables；
- 精确计算 table、main symbols、extra bits、padding 和完整 RIFF bytes；
- 能在候选获胜时完全不物化其他 payload；
- 在回退或 byte tie 时写出与当前 `Default` 完全一致的 bytes；
- cost overflow、allocation failure 和 plan mismatch 必须 fail closed。

`single_plan` 可作为迁移 oracle，不能通过复制一套 spatial/transform plan 实现泛化。

### 4.3 通用 packet sink

当前 `spatial_packet_writer.rs` 把一个 literal/copy/cache 的完整 LSB-first wire bits
装入 `TokenPacket`，使 244,018,874 个 token 不再产生 732,056,622 次独立
`write_bits` 调用。该机制应从 spatial candidate 下沉为所有 profile 的唯一 token
sink：

- literal、copy、cache 都由同一个 packet contract 表达；
- prefix/header/table 仍由各自 owner 写入，token sink 不拥有格式决策；
- accumulator、flush、capacity 和 error semantics 只有一份实现；
- `Default` 迁移必须先证明 byte identity；
- copy-heavy、cache-heavy、palette、alpha 和 tiny corpus 必须补齐，不能继续以
  literal-heavy CLIC 数据外推。

旧的 per-field token write loop 只在 generalized packet sink 完成 Default byte
identity 后删除；在此之前保留为同 binary control，不建立第二套长期产品 writer。

### 4.4 Rate 与 decode work 联合选择

固定 128/64 和 256/16 是当前稳定 profiles，不应成为下一代算法内核。新的 bounded
portfolio 可以使用：

```text
score =
    exact_stream_bits
  + latency_weight * calibrated_decode_work
  + memory_weight * scratch_or_table_bytes
```

`decode_work` 只能由编码时可得的结构特征构成：

- entropy group 数与真实 group-run 次数；
- root/secondary Huffman lookup 模型；
- literal/copy/cache 分布；
- predictor/color/indexing/subtract-green 覆盖；
- transform pass 和输出扩张；
- table/map bytes 与 working-set proxy。

模型必须同时由本项目 decoder 和 pinned libwebp 校准；不能通过实际解码每个候选来
“预测”自己，也不能在 formal corpus 上继续调参数。

## 5. 解码器目标架构

### 5.1 当前最明确的所有权缺口

当前 `image_stream/pixel_buffer.rs` 的 `PixelBuffer` 在 `Argb(Vec<u32>)` 与
`Rgba(Vec<u8>)` 间转换时会重新分配并复制整图；若 transform 顺序需要反向转换，
还会再次重新分配。`pixel_sink.rs` 同时拥有另一份 entropy/LZ77 `Vec<u32>`。

下一架构的首要任务不是宣称“row streaming”，而是先建立一个单一 backing owner：

- entropy/LZ77、inverse transforms 与最终输出共享同一 pixel-store contract；
- layout state 由类型表达，但 layout 变化优先原位或接管 allocation；
- palette expansion 或无法原位处理的步骤拥有显式、有界 scratch；
- 公共 `Vec<u8>` 输出的接管方式必须通过 safe ownership 或极小、审计后的
  representation conversion 实现；
- allocation count、完整图 copy bytes 和峰值 live bytes 成为正式 gate。

VP8L backward reference 可以引用较早 transform-domain pixels。没有距离与最后使用
证明前，不允许把完整 history 武断替换成一行或固定 ring buffer。E19 的 -0.54% 已
证明仅把 transform 改成表面 row loop 不是架构收益。

### 5.2 DecodePlan

在 `image_stream` 内建立不可变计划，拥有：

- dimensions、output length 与 allocation limits；
- transform descriptors 和执行顺序；
- color cache contract；
- entropy group map 与 table views；
- image/group geometry；
- WorkBudget 初始上限；
- 选定的 scalar/SIMD kernel family。

模块依赖固定为：

```text
header/transform parse
        ↓
validated decode plan
        ↓
huffman symbol kernel
        ↓
pixel store / LZ77
        ↓
inverse transforms
        ↓
public RGBA
```

Huffman、pixel store 和 transform 模块不得互相读取实现细节；需要共享的 geometry
或 table view 归 plan 所有。

### 5.3 Huffman 与 symbol kernel

E09/E25/E31 的失败说明，在旧 table 旁边增加 pair table、可变 root 或 transducer
会因 working set、初始化和分支成本变慢。下一版必须重做 ownership，而不是叠表：

- root/secondary entries 连续存储；
- entry layout 以 cache line 和 load 数为准；
- group switch 只替换紧凑 view；
- literal-heavy、copy-heavy、cache-heavy 使用少量静态 kernel family；
- 一次 bit snapshot 的多-symbol 解码只在没有附加大表时采用；
- malformed/truncated prefix 与 WorkBudget 必须保持逐 bit 差分等价。

E28 的双 literal 覆盖 86.862% entropy pixels 但端到端只快 3.576%，可保留为
kernel 机制证据，不能直接 cherry-pick 为下一架构。

### 5.4 Transform pipeline

inverse predictor、color transform、subtract-green、indexing 与 RGBA 写出应根据
真实依赖组合成少量静态 pipeline，而不是一个包含所有分支的大循环。目标是减少：

- 完整图 pass；
- ARGB/RGBA 来回复制；
- per-pixel mode/group 查找；
- transform 之间的临时 allocation。

融合前必须记录每个 transform 的输入/输出 layout、邻居依赖、是否可原位、是否会改变
宽度以及哪些 bytes 仍被后续 backward reference 需要。没有这些不变量时不做融合。

## 6. 实验处置

### 6.1 已进入 `main`，继续保留

| 实验 | `main` 代码 | 决定 |
| --- | --- | --- |
| E30 color-transform wire | `fb17a98` | 标准正确性修复，永久保留 |
| E33 coarse spatial profiles | `9776da40` | 保留公开 profiles 与标准 bitstream |
| E35 exact-cost single-write | `97d6f1f4` | 保留；作为通用 exact plan 的迁移 oracle |
| E37 packed token writer | `b3b96fdc` | 保留；下一步泛化到所有 profile |

这些代码不得通过重跑旧研究树重新迁移；后续直接以当前 `main` 为实现基线。

### 6.2 只迁移机制，禁止直接 cherry-pick

| 实验 | 可复用结论 | 处理 |
| --- | --- | --- |
| E38 streaming statistics | producer 应拥有 sufficient statistics | 在共享 IR 中重建；旧组合代码拒绝 |
| E43 profile hybrid | bounded portfolio 能同时改善 rate 与时间 | 重新设计跨域 policy；旧研究代码不迁移 |
| E49 rank-sum exact cost | 删除候选 table allocation 可行 | 合并进 `EntropyPlan`，不保留独立 recovery 路径 |
| E50 frequency sink | token producer 可直接拥有 group frequencies | 合并进共享 IR；旧实现 2.16% 不单独晋级 |
| E28 two-literal | 多-symbol snapshot 有高覆盖 | 只作为新 Huffman kernel 的机制证据 |

E43 的研究 formal 达到约 51%–52%，但 latest-main 产品树 LowLatency 只有 48.19%，
所以它证明方向而不是可直接进入 `main` 的提交。

### 6.3 保留报告，产品代码 deprecated/rejected

- E02 batch parallel 与 E04 two-stage pipeline 只保留为吞吐类别，不作为算法 headline。
- 账本中决定为“回滚”“拒绝”“benchmark-only”的 E03–E14、E18–E29、E31、
  E39–E42、E45–E50 均不再 cherry-pick 生产代码；其报告仍用于避免重复实验。
- E20 的错误基线、E44 的 base race、E55 的终态矛盾和 E57 的错误 work invariant
  永远不能作为性能证据。
- `codex/vp8l-lz77-aware-parse` 工作树仍有未提交内容，既不晋级也不清理，直到其
  owner 明确处置；本文不把未提交代码当作研究结论。

### 6.4 FDEC/sidecar 路线停止

E15–E17 与 E51–E59 的 FDEC/FDC2/FDCS/sidecar 代码和性能结论统一标记为：

> deprecated research; out of scope for the standard VP8L product

保留的知识只有 row-group ownership、bounded scratch、fused write、atomic evidence
和 strict fallback 的设计经验。以下内容不进入 `main`：

- 私有 FDEC chunk 或 FDCS sidecar；
- Zstd/LZ4 作为 VP8L 产品依赖；
- prepared decoder/cache API；
- canonical source + 多倍 private cache 的部署模型；
- 用 hot-hit latency 代表标准 VP8L 首次解码。

P34/E59 的 product screen 数字有效，但它测量预生成 sidecar 的复用热路径；约
1.79 GB sidecar/cache 对应 444.9 MB source，峰值 footprint 约 6.82 GB。因此它是
终止证据，不是标准算法 promotion。

## 7. 迁移顺序

每一步从当时 latest committed `main` 新建唯一分支；不得从历史实验 worktree 继续。

### Phase 0：验证资产契约

VP8L 架构实验开始前，fixture 与 benchmark corpus 必须拥有可复现的 artifact identity。
本地生成集合使用完整 manifest、逐文件 SHA-256、跨进程 lock、不可变 generation 和
原子 current marker；测试不得通过枚举目录静默决定覆盖率。fixture 的具体协议见
[`test-corpus.md`](test-corpus.md)。

CLIC 等大型 benchmark corpus 后续采用同一身份原则，但不与 fixture 修复一起扩成
全仓库缓存框架。每次 VP8L screen/formal 仍需登记 source、manifest、runner、branch
和 commit；缓存命中不能替代 evidence identity。

### Phase A：泛化基线

- 固定 pinned libwebp commit、工具链、命令和 source SHA。
- corpus 至少分为 photo、alpha、palette/cache、tiny/icon、copy-heavy、synthetic、
  very-large 和 animation/ALPH reuse。
- 同时记录 aggregate、per-image p50/p95/worst、输入 bytes、pixels、CPU、RSS、
  allocation/copy census 和 binary delta。
- ARM64 与 x86-64 分开封口；单线程先于 SIMD 和并行。

### Phase B：编码共享 IR

- **完成**：canonical token/geometry/census/global-frequency owner 由 Default、
  single-plan 与 spatial plan 共享；source owner 包含 transform、alpha、palette、
  resource facts 与可复算 identity；residual 流式生成，block statistics 与 token
  同步收集。
- **完成**：起点 archive 与当前树的 Default byte identity，并额外覆盖 Single/
  Compact/LowLatency。
- **完成**：candidate planning 只借用 source/token/statistics，不复制 token、不重新
  扫描 RGBA；必要的获选 transform backing 是唯一的 transform-domain image。
  Phase C 的 exact plan 在写出前比较完整 RIFF bytes，正常路径只 materialize 获胜
  payload。

### Phase C：通用 exact plan 与 packet sink

- **完成**：`EntropyPlan` 是 single、spatial groups 与 nested group-map 共用的
  canonical Huffman table/view 和 exact table/symbol/extra-bit cost owner；写完时再
  校验 planned/written bit count。
- **完成**：`packet_sink.rs` 是 Default、palette、Single、Compact、LowLatency 的
  唯一 main-token sink；nested group-map token 也通过同一 sink 后无复制地归还
  `BitWriter` 继续写 prefix。
- **完成**：copy/cache/palette/alpha/tiny 与 exact fallback 由单元、profile 和
  byte-identity gates 覆盖；正常路径不写失败 payload，plan failure 才走保持旧错误
  语义的双写 control。
- **完成**：64 条四档 stream 相对 Phase B 逐字节相同；41-file/5-round 同体积编码
  1,192.215 ms 降至 958.604 ms（-19.595%）。

### Phase D：decoder pixel-store

- **完成**：冻结旧路径 census。无 indexing 时主图为 entropy `Vec<u32>` 加最终
  `Vec<u8>` 两份 full-image allocation，并写出 `pixels × 4` layout conversion；
  indexing 还增加 expanded ARGB backing。predictor 的旧行融合虽减少独立 pass，
  仍同时持有两份整图 allocation。
- **完成**：`DecodePlan` 在主 entropy 前验证 coded/final geometry、transform
  逆序终点、output/allocation/work limits、retained transform bytes、kernel family
  与 storage census；主图从 entropy/LZ77 起只拥有一份预分配 RGBA backing。
- **完成**：predictor、color、subtract-green 原位处理 RGBA；palette 从后向前在同一
  backing 内扩张。主图 full-image allocation 从 2（palette 为 3）降为 1，
  layout-copy bytes 从 `pixels × 4`（palette 更多）降为 0；transform subimage 仍各自
  保留一份必要的 packed backing。
- **完成**：RGBA LZ77 copy 保留完整历史、先验证/扣减 WorkBudget 再修改，重叠 copy
  与 packed reference 差分通过；没有引入 row/ring streaming。limits 与既有保守错误
  边界保持不变。
- **完成**：CLIC-102 的 306 个相同标准流相对 Phase C fresh archive 三轮交替中位
  15,163.347 ms 降至 14,866.429 ms（-1.958%）；同一进程口径峰值 RSS
  905,658,368 B 降至 869,482,496 B（-3.994%）。workspace、fixture、上游 corpus、
  animation/ALPH、truncation、malformed、limit 与 WorkBudget gates 全部通过。

### Phase E：Huffman/kernel families

- **完成**：pixel entropy 只长期保留 `FastHuffmanTable` 的 single、packed
  root/secondary 或 strict fallback 三种静态 scalar family；通用 strict table 只在
  header validation/build 期间存在，不保留长期双表。
- **完成**：packed root/secondary 改为 immutable boxed-slice view，64-bit table handle
  从 56 B 降至 40 B；每个五表 entropy group 的 handle working set 减少 80 B，entry
  layout、root/secondary lookup 与 group switch contract 不变。
- **完成**：literal-heavy 继续使用一次 bit snapshot 的四 channel scalar kernel；
  copy/cache 与 rare large-cache fallback 使用严格 single-symbol kernel。尝试再增加
  group-level hot-loop dispatch 没有独立 fresh evidence，未进入产品。
- **完成**：ARM64 SIMD screen 未发现适合在不增加附表/unsafe 双路径的 variable-length
  Huffman kernel；predictor 又有逐像素依赖，因此按“若确有收益”条件不添加显式 SIMD，
  保留 LLVM scalar/auto-vectorized fallback。x86-64 SIMD 仍是外部硬件验收项。
- **完成**：malformed、全字节 truncation、allocation limit、WorkBudget、上游 corpus、
  animation/ALPH、项目 exact 与 pinned-libwebp differential gates 全部通过。
- **完成**：fresh Phase D 对照的 CLIC-102/306-stream 三轮中位
  14,904.641 → 14,720.128 ms（-1.238%），checksum/bytes 不变；release binary
  628,800 → 628,736 B。

### Phase F：联合选择与并行

- bounded portfolio 只使用编码时特征和 exact cost。
- 在 disjoint corpus 决策，formal corpus 不调参。
- 并行只优化 throughput，必须保持 bytes deterministic。

## 8. 晋级与停止条件

一项新架构只有同时满足以下条件才能进入 `main`：

1. 标准 VP8L，双 decoder 完整像素一致；
2. Default byte identity，或公开说明并验证新的显式 profile；
3. 广泛 corpus 每个类别都有完整分母；
4. 单线程 aggregate 改善，p95/worst 没有未解释回退；
5. 编码比较保持相同或更小体积，解码比较使用相同输入 bytes；
6. RSS、allocation、copy、binary 和依赖成本完整；
7. latest-main 产品树独立于研究树；
8. 原始样本、runner、manifest、branch、commit 和决定进入账本。

下列情况在 screen 阶段立即停止并归档，不运行 formal：

- 收益来自私有表示、预计算 cache 或改变比较口径；
- 只在单一 corpus/profile 上成立；
- 需要明显增大标准流却未形成明确 Pareto；
- per-image regressions 超出预注册门槛；
- evidence identity、终态或 runner ownership 不唯一；
- 机制只减少模型计数，真实端到端时间没有转化。

## 9. Worktree 与历史证据

实验 worktree 不是永久档案。永久定位键是：

```text
branch + commit + report path + manifest/hash + ledger row
```

已封口、clean、提交完整且不再运行的 VP8L/FDEC worktree 应删除以回收磁盘；分支与
commit 保留。dirty、detached 且身份不明、仍被其他任务使用、或拥有唯一未提交证据的
worktree 禁止删除。删除工作树不能删除分支，也不能删除账本中引用的外部唯一证据，
除非证据已逐字复制并以 manifest 封存。

删除工作树后，账本里的历史绝对路径可能不再存在；应使用登记的分支和 branch-relative
report path 读取或恢复：

```bash
git show codex/example-branch:experiments/example/REPORT.md
git worktree add /absolute/recovery/path codex/example-branch
```

2026-07-23 的清理集合为 53 个已封口、clean 且有命名分支的 VP8L/FDEC worktree，
清理前占用约 20.97 GiB。以下内容明确排除在清理之外：

- 根 `main` 工作区及其用户改动；
- dirty 的 `codex/vp8l-lz77-aware-parse`；
- Alpha 与 final-rust 工作树；
- detached、身份或 owner 尚未确认的工作树。
