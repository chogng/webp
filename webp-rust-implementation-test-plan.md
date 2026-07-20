# 纯 Rust WebP 实现方案与测试工程规范

> 文档状态：实施草案 v1.0  
> 基线日期：2026-07-20  
> 目标读者：负责纯 Rust WebP codec、Skia 适配、测试基础设施和安全验收的工程师或 Codex 代理  
> 核心原则：**先建立可判定的测试闭环，再扩展功能；正确性、安全性、兼容性和性能分别验收。**

---

## 1. 结论与推荐路线

建议新建一个以安全 Rust 为核心、测试环境可选链接 `libwebp` 的 workspace。运行时与发布产物不依赖 C；`libwebp` 只存在于私有的 oracle crate 和 CI 镜像中，用于生成基准答案、差分测试和交叉解码。

不要一开始追求完整的 `libwebp` C ABI，也不要同时推进 VP8、VP8L、动画、编码器、SIMD 和 Skia 接口。推荐顺序是：

1. 先完成测试基础设施、RIFF/VP8X 容器和内部数据模型。
2. 完成 VP8L 静态解码，并达到逐像素一致。
3. 完成 VP8 静态解码，并分别验证原生 YUV 与 RGBA 输出。
4. 加入 `ALPH`、metadata、动画和增量解码。
5. 完成 VP8L 编码器，再完成 VP8 编码器。
6. 最后加入 SIMD、并行编码、Skia adapter 和可选 C ABI。

这条路线的关键不是“先做简单功能”，而是保证每个阶段都能由自动化测试给出明确结论。每个实现任务必须同时提交对应的规范向量、属性测试、差分测试和至少一个 fuzz target 或现有 fuzz target 的覆盖证明。

### 1.1 推荐的首个生产目标

第一生产目标建议限定为：

- 静态与动画 WebP 解码；
- VP8、VP8L、`ALPH`、`VP8X`、`ANIM`、`ANMF`；
- ICC、EXIF、XMP 原样提取；
- 输出 canonical straight RGBA8，以及可选原生 YUV420；
- 明确的资源限制和增量输入；
- 可供 Skia adapter 调用；
- 默认核心无 `unsafe`。

编码器应作为独立里程碑。尤其是 VP8 有损编码器，规范正确只是最低要求，率失真质量、文件大小和速度需要单独的长期测试体系。

### 1.2 不建议的路线

不建议直接将 `libwebp` 逐文件翻译成 Rust，然后等全部翻译结束再测试。这样会同时继承原实现的隐式状态、宏、平台分支和数据布局，差分失败时很难定位到具体阶段。

也不建议只依赖“编码后能被自己解码”。编码器与解码器可能共享同一个错误并形成自洽闭环。所有编码输出至少要由一个独立实现解码；在本方案中，该独立实现是固定版本的 `libwebp`，VP8 裸 bitstream 还可选用 `libvpx` 作为第二 oracle。

---

## 2. 兼容目标必须先定义

WebP 的“正确”至少有四种含义。项目必须在 README 和测试代码中明确区分，不能用一个模糊的 `compatible` 概括。

### 2.1 规范正确性

输入符合 WebP container、VP8L bitstream 或 VP8 key frame 规范时，解码器应成功并产生规范定义的结果；规范明确要求拒绝的输入必须拒绝。这里的判定依据是 WebP container 规范、WebP lossless bitstream 规范和 RFC 6386。

### 2.2 libwebp 行为兼容性

Skia 替换场景通常还要求与既有 `libwebp` 行为接近，包括宽松解析、颜色转换舍入、动画合成、错误时机和增量 API 行为。建议提供两个 profile：

```rust
pub enum CompatibilityProfile {
    SpecStrict,
    LibwebpCompatible,
}
```

`SpecStrict` 用于格式验证和安全工具；`LibwebpCompatible` 用于 Skia/浏览器迁移。二者不能通过散落在代码中的条件分支实现，应集中到解析策略和输出策略中，并有独立测试矩阵。

### 2.3 API 兼容性

首版不要求复刻全部 `libwebp` C API。公共 Rust API应保持小而清晰。C ABI 或 Skia adapter 是外围 crate，其职责是参数转换、错误映射、像素布局转换和阻止 panic 穿越 FFI 边界。

### 2.4 性能兼容性

性能不参与“功能正确”的判定。一个输出完全正确但速度较慢的标量实现可以先合并。SIMD 和并行路径必须以标量路径作为可执行规范，并通过逐字节差分后才能启用。

---

## 3. 推荐 workspace 架构

```text
webp-rs/
├── Cargo.toml
├── crates/
│   ├── webp/                    # 公共 facade，稳定 API
│   ├── webp-container/          # RIFF、VP8X、metadata、mux/demux
│   ├── webp-vp8l/               # VP8L 解码与编码
│   ├── webp-vp8/                # VP8 key-frame 解码与编码
│   ├── webp-anim/               # 帧模型与画布合成
│   ├── webp-dsp/                # 标量 DSP；后续放 SIMD dispatch
│   ├── tests/                   # fixtures、corpus selection 与集成测试数据
│   ├── webp-oracle/             # 仅 dev/CI；固定版本 libwebp/libvpx FFI
│   ├── webp-skia/               # 可选 Skia/CXX adapter
│   └── webp-cli/                # 调试、语料处理、差分命令行
├── fuzz/
│   ├── Cargo.toml
│   ├── fuzz_targets/
│   ├── corpus/
│   └── artifacts/
├── tests/
│   ├── fixtures/smoke/          # 小型、可提交、每次 PR 运行
│   ├── fixtures/regressions/    # 所有历史 bug 的最小化输入
│   └── integration/
├── tools/
│   ├── faults/                  # codec 专用人工故障补丁
│   ├── oracle-build/
│   └── corpus-lock.toml
├── xtask/
└── docs/
```

如果维护成本需要更低，可以先将 `webp-container`、`webp-anim` 和 `webp-dsp` 做成 `webp` 内部模块。但 `webp-oracle` 和 fuzz crate 应始终与发布核心隔离。

### 3.1 安全边界

建议在核心 crate 使用：

```rust
#![forbid(unsafe_code)]
```

未来需要手写 SIMD 时，单独在 `webp-dsp` 中建立非常窄的 `unsafe` 模块。每个 SIMD 函数必须具备：

- 同签名的标量实现；
- 任意有效输入上 `simd(input) == scalar(input)` 的属性测试；
- Miri 可运行的调用前置条件测试；
- ASan/平台 sanitizer 测试；
- 强制指定 dispatch 路径的测试开关；
- 代码审计记录。

### 3.2 为测试设计内部接口

以下能力应从第一天加入，而不是出现 bug 后临时埋点：

```rust
pub struct DecodeLimits {
    pub max_input_bytes: usize,
    pub max_width: u32,
    pub max_height: u32,
    pub max_pixels: u64,
    pub max_frames: u32,
    pub max_total_frame_pixels: u64,
    pub max_metadata_bytes: usize,
    pub max_alloc_bytes: usize,
    pub max_work_units: u64,
}

pub(crate) struct WorkBudget { /* deterministic counters */ }

#[cfg(any(test, feature = "trace"))]
pub(crate) trait TraceSink {
    fn event(&mut self, event: DecodeEvent);
}
```

`max_work_units` 是防 CPU 拒绝服务的重要部分。墙钟超时容易受 CI 负载影响；确定性的 work counter 可以按“解析一个 chunk、读取一个 Huffman symbol、恢复一个像素、处理一个宏块或执行一次 LZ77 copy”扣费，使复杂度测试可重复。

所有尺寸计算必须经过集中函数，禁止在各模块散落 `width * height * 4`：

```rust
fn checked_image_bytes(width: u32, height: u32, channels: usize) -> Result<usize, Error>;
fn checked_rect_end(origin: u32, extent: u32, limit: u32) -> Result<u32, Error>;
fn checked_chunk_end(offset: usize, payload: u32, input_len: usize) -> Result<usize, Error>;
```

这些函数是 Kani、property testing 和 mutation testing 的重点目标。

---

## 4. 公共 API 草案

核心输出采用 straight/unpremultiplied RGBA8。Premultiplied alpha 是 Skia adapter 的职责，避免在 codec 内部混淆 WebP 原始像素、动画合成和显示格式。

```rust
pub fn decode(data: &[u8], options: &DecodeOptions) -> Result<Image, DecodeError>;

pub fn decode_animation(
    data: &[u8],
    options: &DecodeOptions,
) -> Result<Animation, DecodeError>;

pub fn read_info(data: &[u8], limits: &DecodeLimits) -> Result<ImageInfo, DecodeError>;

pub fn read_metadata(data: &[u8], limits: &DecodeLimits) -> Result<Metadata<'_>, DecodeError>;

pub struct IncrementalDecoder { /* state machine */ }

impl IncrementalDecoder {
    pub fn push(&mut self, bytes: &[u8]) -> Result<Progress, DecodeError>;
    pub fn finish(self) -> Result<Image, DecodeError>;
}
```

建议错误只承诺稳定的高层分类，不承诺每个内部 bit offset 永久稳定：

```rust
pub enum DecodeErrorKind {
    InvalidContainer,
    InvalidBitstream,
    UnsupportedFeature,
    UnexpectedEof,
    LimitExceeded,
    AllocationFailed,
    InvalidParameter,
}
```

错误对象可以携带内部 stage、offset 和上下文，便于 fuzz triage，但公共使用方只能依赖 `kind()`。

---

## 5. 测试总体模型

测试不应按“unit/integration/fuzz”简单分类，而应回答六个独立问题：

| 维度 | 要回答的问题 | 主方法 |
|---|---|---|
| 规范一致性 | 合法 bitstream 是否被正确解释 | 手工向量、生成式向量、规范断言 |
| 参考兼容性 | 与固定版本 libwebp 是否产生同一可观察行为 | 差分测试、golden hash |
| 健壮性 | 任意字节是否不会 panic、死循环或失控分配 | raw fuzz、截断/突变、资源预算 |
| 状态一致性 | one-shot、incremental、scalar、SIMD、线程数是否等价 | metamorphic testing |
| 编码质量 | 编码输出是否有效、可互操作、质量和体积是否回归 | 双向解码、RD 曲线、语料统计 |
| 平台一致性 | 字节序、字长、CPU feature 和 OS 是否改变结果 | 跨平台 CI、Miri、强制 dispatch |

### 5.1 “测试通过”的分层定义

一个模块只有同时满足以下条件才算完成：

1. 规范条目有对应的 feature matrix 行；
2. 正常路径有最小 golden vector；
3. 边界值有表驱动单元测试；
4. 关键代数关系有 property test；
5. 至少一个 public-path integration test 能进入该模块；
6. fuzz target 能从公开 API 到达该模块；
7. 关键比较、长度和索引错误能被 mutation test 杀死；
8. 任何已发现失败都转化为最小化回归 fixture。

代码覆盖率只作辅助指标。codec 中最重要的是“格式特征覆盖”和“故障模型覆盖”，不能用 90% line coverage 替代 Huffman 特殊树、动画 disposal 或 VP8 filter 边界的测试。

---

## 6. Oracle 体系

### 6.1 固定而不是追随 latest

`webp-oracle` 必须固定到明确的 `libwebp` commit，并将以下信息写入 `tools/corpus-lock.toml`：

```toml
[libwebp]
commit = "<40-char-sha>"
source_sha256 = "<archive-sha256>"
build_profile = "scalar-canonical-v1"
compiler = "clang-<pinned-major>"

[libvpx]
commit = "<optional-sha>"
source_sha256 = "<archive-sha256>"
```

更新 oracle 是一次显式迁移：先运行旧、新 oracle 的全语料差分，解释所有输出变化，再更新 golden。禁止 CI 每次拉取默认分支。

### 6.2 Canonical oracle 配置

建议至少准备两个 `libwebp` 构建：

- `scalar-canonical`：禁用或绕过 CPU SIMD dispatch，用于生成跨机器稳定 golden；
- `native-optimized`：使用当前平台优化路径，用于兼容性和性能比较。

构建方法应固定在 `tools/oracle-build/`，不要依赖开发机已安装的 `dwebp`。容器、编译器、编译 flags 和源码 SHA 都应被记录。

### 6.3 差分的比较层级

对于 VP8L：

- 宽、高和 alpha 信息必须完全相同；
- straight RGBA8 必须逐字节相同；
- fully transparent 像素的 RGB 也必须相同；VP8L 规范要求 bitstream 中这些颜色值可被精确恢复；
- metadata 字节必须原样相同。

对于 VP8：

- 原生 Y、U、V plane 应作为第一层精确比较对象；
- ALPH plane 精确比较；
- RGBA 对固定的 scalar oracle profile 做逐字节比较；
- 如果确认 oracle 不同平台颜色转换存在合法差异，必须将差异限制在明确命名的 `ColorConversionProfile`，不能在全局测试里直接允许每通道误差 `<= 1`；
- SIMD 输出必须与本项目标量输出逐字节相同。

对于动画：

- 比较原始 frame rect、duration、blend、dispose、frame codec 和 metadata；
- 再比较每个 composited canvas；
- duration `0` 或很小值只应被保留，不在 codec 层按浏览器策略改写；
- 如果提供线性光 alpha blend 和 libwebp-compatible blend，两种模式分别建立 golden。

### 6.4 malformed 输入不能简单要求“与 libwebp 同错误”

无效输入按 manifest 分类：

```rust
pub enum FixtureClass {
    MustAccept,
    MustReject,
    CompatAccept,
    ImplementationDefined,
}
```

- `MustAccept`：规范合法，当前实现必须接受。
- `MustReject`：违反明确的有效性条件或会突破安全边界，必须拒绝。
- `CompatAccept`：不够规范但被主流 libwebp/Skia 接受，兼容 profile 应接受。
- `ImplementationDefined`：规范允许宽松处理或错误时机无要求，只检查不 panic、不超资源和错误分类稳定。

对随机 malformed 输入，只要求安全性质；不要把 libwebp 的每个宽松行为自动变成永久兼容义务。

### 6.5 防止 oracle 与被测实现共享错误

测试中至少采用以下交叉关系：

```text
libwebp encode -> ours decode
ours encode    -> libwebp decode
ours encode    -> ours decode
libwebp encode -> libwebp decode   # 验证 fixture/oracle 自身
```

VP8 裸 key frame 可以额外执行：

```text
ours VP8 bitstream -> libvpx decode YUV
```

手工规范向量和慢速 reference model 仍然必要。只做“ours vs libwebp”会把 oracle 的偶然行为误认为规范，也无法在 oracle 自身崩溃时判断责任。

---

## 7. 语料库设计

### 7.1 四层语料

第一层是小型 smoke corpus。它应直接提交到 Git，每个文件尽量小于几十 KiB，覆盖所有主路径，保证普通 PR 在几分钟内完成。

第二层是官方/上游 conformance corpus。至少固定 `libwebp-test-data` 的 commit。该仓库包含 VP8 comprehensive vectors、VP8L transform combinations、alpha filter/compression 组合、极小尺寸、历史 endian 和 palette bug 等测试数据。

第三层是生成式 feature corpus。由 `xtask` 生成，目的不是模拟真实图片，而是精确控制 chunk order、Huffman tree、LZ77 距离、transform stack、动画矩形和错误位置。

第四层是 security/regression corpus。所有 fuzz crash、生产失败、CVE 类故障和跨平台不一致都必须最小化并永久保存。

### 7.2 每个 fixture 都必须有明确测试契约

示例：

```toml
id = "vp8l-huffman-unbalanced-valid-001"
file = "vp8l-huffman-unbalanced-valid-001.webp"
sha256 = "..."
class = "MustAccept"
source = "generated"
license = "CC0-1.0"
codec = "VP8L"
features = ["normal-huffman", "unbalanced-tree", "single-meta-group"]
expected_width = 3
expected_height = 2
expected_rgba_sha256 = "..."
max_work_units = 2000
max_alloc_bytes = 4096
notes = "guards table sizing assumptions; CVE-2023-4863 fault class"
```

测试契约不是文档装饰。直接消费 fixture 的测试应明确调用哪些 API、比较什么结果和施加哪些资源上限。

### 7.3 语料获取和可复现性

建议提交下载脚本和小型 smoke corpus。较大的上游和真实图像 corpus 通过：

```text
cargo xtask corpus fetch
cargo xtask corpus verify
cargo xtask corpus index
```

获取。`fetch` 只能下载 lock file 中固定的 commit/archive；`verify` 检查 SHA-256 和许可证信息；`index` 生成 feature coverage 报告。

不要依赖 git submodule 的浮动状态，也不要把来源不明的网络图片放进仓库。真实图片必须有可分发许可证，或只在私有 CI 中使用并与公开 corpus 分开。

### 7.4 回归 fixture 的生命周期

任何失败按以下流程处理：

1. 保存原始输入、运行命令、git SHA、平台、toolchain 和 stack trace；
2. 使用 fuzzer minimizer 和格式感知 reducer 最小化；
3. 判断 root cause，不按 stack trace 数量机械去重；
4. 为最小输入编写直接调用公开 API 的测试；
5. 在修复前确认新测试确实失败；
6. 修复后将 fixture 放入 `tests/fixtures/regressions/`；
7. 将 root cause 映射到一个 generic property 或 mutation fault，避免只记住单个字节串。

“删除已经修好的 crash 文件以缩小仓库”是不允许的。可按压缩包或 Git LFS 管理，但不能失去回归能力。

---

## 8. 基础模块测试明细

### 8.1 LSB bit reader / writer

VP8L 按 least-significant-bit first 读取。至少覆盖：

- 从 bit offset `0..7` 开始读取 `0..32` 位；
- 一次读取与逐位读取结果一致；
- 跨 1、2、4、8 字节边界；
- 全 `0x00`、全 `0xff`、交替 bit pattern；
- 输入为空、恰好结束、差 1 bit、差 1 byte；
- 失败后 cursor 是否前移，语义必须固定并测试；
- `read_bits(0)`；
- 禁止移位宽度等于整数位数；
- `usize` 为 32 位时仍无截断；
- writer -> reader 的属性往返；
- reader 的快速路径与逐位慢速 reference model 差分。

建议写一个仅测试使用的慢速 bit reader，每次只读取一个 bit。任何优化版 reader 都与它做 property differential。

### 8.2 checked arithmetic 与资源预算

对所有尺寸函数做穷举边界测试：

- `0`、`1`、最大合法值、最大值前后；
- `u32::MAX`、`usize::MAX` 附近；
- 32 位与 64 位 target；
- canvas product 恰好等于和超过限制；
- row stride 加 padding；
- frame rect `origin + extent`；
- RIFF `offset + 8 + payload + pad`；
- animation 总像素累计；
- metadata 累计；
- work counter 恰好耗尽与超出 1 unit。

这些函数必须没有 release/debug 行为差异。所有算术使用 `checked_*` 或经过证明的更宽整数，不依赖 release mode wrapping。

### 8.3 分配失败注入

仅有 `max_alloc_bytes` 不足以覆盖真实分配失败。建立专用测试 binary，安装 counting/failing global allocator，对一次正常解码的第 `1..N` 次分配依次失败。每个 failure point 应满足：

- 返回 `AllocationFailed` 或 `LimitExceeded`；
- 不 panic；
- 不泄漏已分配对象；
- decoder 状态可被安全 drop；
- FFI adapter 不传播 unwind；
- 不产生部分初始化的公开对象。

核心实现优先使用 `try_reserve` 和显式容量检查。不能假设 Rust 的内存安全自动解决 OOM 行为。

---

## 9. RIFF、VP8X、metadata 与 mux/demux 测试

### 9.1 RIFF 基础

表驱动覆盖：

- `RIFF`/`WEBP` magic 的每字节错误；
- file size 恰好、偏小、偏大、奇数、溢出；
- 文件末尾 trailing bytes：strict 与 compatible profile；
- chunk size 不含 header/padding；
- 奇数 payload 后的零 padding；
- 非零 padding：writer 永远不生成，reader 的 strict/compatible 行为明确；
- 0 长 chunk；
- 未完整的 FourCC、size 和 payload；
- 最大 32 位 chunk size 但实际输入极短；
- 多个未知 chunk；
- FourCC 大小写敏感；
- chunk end 计算发生整数溢出的输入。

### 9.2 simple 与 extended layout

必须覆盖：

- simple `VP8 `；
- simple `VP8L`；
- `VP8X + VP8`；
- `VP8X + ALPH + VP8`；
- `VP8X + VP8L`；
- ICCP 在 image data 前；
- EXIF/XMP 在 image data 后或规范允许的位置；
- reconstruction chunk 乱序；
- metadata/unknown chunk 的规范允许乱序；
- 重复 `VP8X`、`VP8`、`VP8L`、`ALPH`、`ANIM`；
- VP8X flags 与实际 chunk 不一致；
- reserved bits 非零。规范要求 writer 写零，同时要求 reader 忽略部分 reserved 字段，测试必须反映这一细节，不能统一“非零即拒绝”；
- VP8X canvas 与内层 frame/image 尺寸不一致；
- canvas width/height 的 24-bit 边界以及 product `2^32 - 1` 边界。

### 9.3 尺寸边界不要合并成一个常量

应分别测试并记录：

- VP8 key frame 头部的 14-bit 尺寸语义；
- VP8L 规范中 `ReadBits(14) + 1` 的尺寸语义；
- VP8X 24-bit `minus one` canvas；
- libwebp API 自己施加的 product/maximum dimension 限制；
- 项目 `DecodeLimits` 的产品级限制。

不要用一个 `MAX_WEBP_DIMENSION` 同时代表语法上限、libwebp 兼容上限和产品安全上限。边界 fixture 应覆盖每个限制的前一值、边界值和后一值。

### 9.4 metadata

ICC、EXIF、XMP 测试包括：

- 0 字节、1 字节、奇数长度、包含 NUL、任意非 UTF-8 EXIF；
- XMP FourCC 尾部空格；
- byte-for-byte 提取和重写；
- metadata 超预算；
- mismatched VP8X flag；
- 重复 metadata chunk 的策略；
- 只读 metadata 不触发像素分配；
- metadata rewrite 不修改 image bitstream；
- unknown chunk 保持原始顺序和内容；
- mux -> demux -> mux 的结构不变量。


---

## 10. VP8L 解码测试明细

VP8L 是第一个应达到生产门禁的 codec。不要只以“官方样例能显示”为完成标准。VP8L 的高风险区域是 Huffman table、嵌套子图像、LZ77 overlap、color cache、transform 顺序和尺寸派生。

### 10.1 VP8L header

覆盖：

- signature `0x2f` 正确与每个 bit 的错误；
- width/height 最小值和 14-bit field 全范围边界；
- `alpha_is_used` 只作为 hint，不能影响像素恢复；
- version `0` 接受，非零拒绝；
- header 每一个 truncation point；
- standalone VP8L bitstream 和 RIFF `VP8L` chunk 的长度一致性；
- RIFF canvas 与 VP8L header 尺寸一致性；
- 超产品 `max_pixels` 时在像素分配之前拒绝。

### 10.2 transform 列表与顺序

四种 transform 每种最多出现一次；inverse 按读取顺序的逆序执行。至少构造：

- 无 transform；
- 每种单独出现；
- 所有两两组合；
- 所有规范允许的三重和四重组合；
- 重复同类 transform；
- transform descriptor 截断；
- transform 子图像尺寸恰好整除与需要 round-up；
- 宽/高为 1 时的 transform；
- transform 后宽度改变对后续 transform 的影响；
- inverse 顺序故意颠倒的 mutation 能被测试杀死。

官方 `libwebp-test-data` 已包含多种 transform 组合，但仍需自行生成更小的 1×N、N×1、边界 block size 向量，便于定位。

### 10.3 predictor transform

WebP lossless 有 14 种 predictor mode。每种 mode 都要测试：

- 内部像素，四个邻居均存在；
- `(0,0)`；
- 第一行非首列；
- 第一列非首行；
- 最后一列对 top-right 的边界处理；
- block 边界前后；
- 1×1、1×2、2×1、2×2；
- 通道值 `0`、`1`、`127`、`128`、`254`、`255`；
- 加法和减法 modulo 256；
- averaging、clamped add/subtract 的所有分支；
- scalar fast path 与逐像素慢速模型完全一致。

建议在测试模块中写不优化的 predictor reference model，以直观公式实现，每种 mode 单独函数。生产实现无论采用 packed `u32` 还是 SIMD，都与该模型差分。

### 10.4 color transform

覆盖颜色乘数的符号扩展和整数舍入：

- multiplier 字节的所有关键值：`0x00`、`0x01`、`0x7f`、`0x80`、`0xff`；
- R/G/B/A 通道极值；
- 负 multiplier；
- 中间值超 8 位后的 wrap；
- transform block size 边界；
- forward + inverse 的属性往返（编码器完成后）；
- 故意将 signed 解释为 unsigned 的 fault patch 必须失败；
- 故意改变 shift/rounding 常量的 fault patch 必须失败。

### 10.5 subtract-green transform

该变换简单但适合发现通道顺序错误：

- `R += G`、`B += G` 的 modulo 256；
- alpha 不变；
- RGBA、BGRA、内部 ARGB packed 表示之间的映射；
- green 为 0、255；
- fully transparent 像素的 RGB 保留；
- forward/inverse property。

### 10.6 color indexing / palette transform

必须覆盖 palette size：

```text
1, 2, 3, 4, 5, 15, 16, 17, 255, 256
```

并覆盖：

- 1/2 色时 8 pixels per packed pixel；
- 3/4 色时 4 pixels per packed pixel；
- 5..16 色时 2 pixels per packed pixel；
- 17..256 色时不 bundling；
- 宽度不能整除 bundle size；
- 每行末尾未使用 bit；
- palette delta decoding 的各通道 wrap；
- palette index 等于 `size-1`；
- index 等于或超过 size 时输出 transparent black；
- palette 自身使用 entropy coding 的边界；
- palette image 不允许普通 transform 前缀；
- 1×N 和 N×1 palette；
- 故意将 index 从 red/blue channel 读取的 mutation 必须失败。

### 10.7 color cache

覆盖 cache bits：

- 合法 `1..11`；
- 非法 `0` 和 `12..15`；
- cache 初始全零；
- hash multiplier 和高位提取；
- collision 后覆盖；
- literal、LZ77 copy 和 cache hit 产生的每个像素都按规范更新 cache；
- LZ77 overlap 中逐像素更新顺序；
- fully transparent color 仍按完整 32-bit 颜色 hash；
- cache size 为 2 与 2048 的边界；
- 故意在一次 LZ77 copy 后批量更新 cache 的 fault 必须失败。

可对小 cache bits 和小像素序列做穷举，与 `HashMap`/直接数组慢速模型比较。

### 10.8 length/distance prefix 与 LZ77

对 24 个 length prefix 和 40 个 distance prefix 的所有边界值建立表驱动测试。至少验证：

- prefix `0..3` 无 extra bits；
- 每个区间的最小值与最大值；
- 最大 length 4096；
- distance code `1..120` 的二维邻域映射；
- distance code `>120` 的线性映射；
- 当前行宽为 1、2、7、8、15、16 时的 mapping；
- 映射落在当前位置之前；
- distance 为 0；
- distance 大于已产生像素数；
- copy length 超过剩余输出；
- overlap：`D=1, L>1`，以及 `D<L` 的多种情况；
- copy 跨行；
- copy 恰好到输出末尾；
- 输出像素计数成为隐式 EOI，不读取多余 symbol；
- 超长 symbol 序列受 work budget 限制。

LZ77 copy 应有一个逐元素慢速实现用于 property differential。优化的 slice doubling 或 `copy_within` 路径不能作为唯一实现和唯一 oracle。

### 10.9 Huffman / canonical prefix code

这是安全优先级最高的模块。测试必须比普通单元测试更强。

#### simple code

覆盖：

- 单 symbol，1-bit symbol id；
- 单 symbol，8-bit symbol id；
- 双 symbol；
- 两个 symbol 相同。规范允许但低效，行为必须与 oracle 对齐；
- symbol 0、1、255；
- bitstream 在每个字段截断；
- alphabet 上限检查。

#### normal code-length code

覆盖：

- `num_code_lengths` 的全部合法值；
- `kCodeLengthCodeOrder` 每个位置；
- code 0..15；
- repeat code 16 在已有非零值后；
- repeat code 16 出现在任何非零值之前，默认重复 8；
- code 17 的 3 和 10；
- code 18 的 11 和 138；
- repeat 恰好填满、超出 1、超出很多；
- `max_symbol` 使用完整 alphabet 和显式缩短两种路径；
- `max_symbol` 超 alphabet；
- single-leaf tree；
- complete balanced tree；
- 极端不平衡但规范合法的 complete tree；
- oversubscribed tree；
- incomplete tree；
- 空 distance tree 在无 backward reference 时；
- root table 需要 secondary table；
- table size 不能根据“通常平衡”做固定假设。

#### 安全故障类

为 Huffman builder 建立专门的 fault catalog：

- 少分配一个 table entry；
- 将 `<=` 改为 `<`；
- 忽略 secondary table size；
- code length 累加用窄整数；
- repeat count 加一/减一；
- 将 bit reverse 去掉；
- single-symbol 消耗 1 bit 而不是 0 bit；
- incomplete tree 被当作合法；
- symbol count 与 alphabet count 混用。

这些 fault 必须全部被 unit/property/conformance 中至少一种测试杀死。CVE-2023-4863 对应的 OOB write 正发生在 lossless Huffman table 构建一类逻辑中，因此这一模块不能只靠 safe Rust 的越界 panic；在生产库里，恶意输入触发 panic 仍然是安全/可用性缺陷。

### 10.10 meta prefix / entropy image

覆盖：

- 不使用 entropy image；
- `prefix_bits` 全部值；
- entropy image 宽高 round-up；
- 单 group；
- 多 group；
- group id 稀疏、最大值较大；
- group count 导致分配超预算；
- pixel `(x,y)` 到 group 的 block index；
- 最后一行/列；
- entropy image 本身的嵌套解码不允许 transform；
- group table 数量乘以 5 的溢出；
- 解析完表但输出像素为 0 的异常状态。

### 10.11 VP8L 端到端门禁

阶段门禁至少为：

- 官方 lossless vectors 100% 接受；
- lossless RGBA 与 pinned scalar libwebp 逐字节一致；
- 所有 transform 组合有 feature coverage；
- 所有小尺寸 fixture 在每个 byte truncation point 不 panic；
- random small RGBA 经 pinned libwebp encode 后可由 ours/libwebp 精确恢复；ours encode 产生的流由两边精确恢复属于 M4 encoder 门禁；
- raw VP8L fuzz、Huffman structured fuzz 和 transform structured fuzz 均无已知 crash；
- critical mutation score 不低于 95%，且无未解释 surviving mutant。

---

## 11. VP8 有损解码测试明细

VP8 解码难点不是容器，而是 boolean entropy decoder、partition、预测、逆变换、滤波和颜色转换的整数语义。建议先提供原生 YUV420 输出，再增加 RGBA 转换。这样可以把“VP8 bitstream 错误”和“YUV→RGB 舍入差异”分离。

### 11.1 frame tag 与 uncompressed header

覆盖：

- frame tag 的 key/inter bit；WebP still image 必须使用 key frame；
- version、show frame、first partition length；
- key frame start code `0x9d 0x01 0x2a`；
- width/height 14-bit 字段；
- horizontal/vertical scale bits；
- color space/clamping bits；
- 每个 truncation point；
- first partition length 超输入、加法溢出、与 token partition 重叠；
- width/height 为 0 的处理；
- 产品尺寸限制在 macroblock scratch 分配之前执行。

### 11.2 boolean entropy decoder

建立最慢、最直观的 reference boolean decoder，并对优化版做差分。测试：

- probability `0, 1, 2, 127, 128, 254, 255`；
- range normalization触发 0..多次 shift；
- value 位于 split 前后；
- 输入恰好结束与补零规则；
- 连续数千 bit；
- 逐 bit 与批量读取路径；
- 随机概率序列和 bitstream；
- encoder 完成后 boolean writer -> reader property；
- 故意将 `< split` 改成 `<= split` 的 fault；
- normalization shift off-by-one fault。

### 11.3 partition

覆盖 token partition 数量 `1,2,4,8`，以及：

- 每个 partition 为空、极短、恰好结束；
- size table 截断；
- size sum 溢出；
- 最后 partition 隐式占剩余数据；
- macroblock rows 在 partition 间的映射；
- odd image dimensions；
- partition EOF 只影响其负责的 row；
- incremental input 跨 size table 和 partition 边界。

### 11.4 segmentation、quantization 和 probability updates

覆盖：

- segmentation disabled/enabled；
- map update 和 data update 的所有组合；
- absolute/delta segment feature mode；
- segment id tree 每个叶子；
- quantizer base index 全范围和各 delta 极值；
- clamp 到合法 dequant table index；
- loop filter delta enabled/update；
- coefficient probability 每个 update 分支；
- skipped macroblock 对 context 的影响；
- 故意漏掉 segment delta 或符号位的 fault。

### 11.5 intra prediction

对宏块 luma `DC/V/H/TM`、chroma mode，以及 4×4 `B_PRED` 的全部模式建立 microvector。每种至少覆盖：

- 左、上、左上邻居均有；
- 第一行；
- 第一列；
- 图像最右和最下不完整 macroblock；
- 相邻 macroblock 的边界像素；
- top-right extension；
- predictor 加 residue 后饱和到 `0..255`；
- 无系数时纯 predictor 输出；
- 固定小 block 的完整预期数组，而不是只检查 hash。

### 11.6 coefficient token decode

覆盖：

- EOB；
- ZERO；
- ONE；
- TWO/THREE/FOUR；
- CAT1..CAT6 的最小与最大 magnitude；
- positive/negative sign；
- zig-zag order；
- Y2/WHT 路径与无 Y2 路径；
- `mb_skip_coeff`；
- 上/左 non-zero context；
- 每个 plane/block type/band/context；
- coefficient count 恰好 16 和超出；
- partition EOF 在 token 中间；
- 概率表更新后使用新值。

### 11.7 inverse transform 与 dequant

对 inverse DCT、inverse WHT 分别使用：

- 全零；
- 仅 DC；
- 单 AC coefficient；
- 每个位置单独非零；
- 正负最大合法值；
- 可能触发中间值边界的组合；
- RFC/reference code 生成的 golden block；
- scalar 与 SIMD bit-exact；
- rounding constant +/-1 fault；
- shift count fault；
- saturation before/after add 顺序 fault。

建议用独立脚本从固定 libwebp/reference decoder dump 每个中间 block，生成小型 JSON/Rust arrays。不要只比较最终图像，否则 DCT 与 prediction 的两个错误可能互相抵消。

### 11.8 loop filter

覆盖 simple 和 normal filter：

- filter level `0,1,2,63`；
- sharpness `0,1,7`；
- HEV threshold 各分支；
- macroblock edge 与 subblock edge；
- segment/filter delta 后的 level；
- key frame interior/exterior edge；
- 边界不越图像；
- tiny width/height；
- 无 filter 与 filter disabled；
- 每种 pattern 的完整像素 vector；
- 标量/SIMD 一致。

### 11.9 YUV420 与 RGBA

原生 YUV 比较必须记录 plane dimensions 和 stride：

- odd width/height；
- Y、U、V stride 大于有效宽度；
- crop 到偶数/奇数边界；
- full decode 的 crop slice 与直接 crop API；
- YUV plane 与 libwebp/libvpx 逐字节一致；
- RGBA、BGRA 等 channel order 分开测试；
- scalar color converter 与固定 libwebp profile；
- 0、16、128、235、240、255 等边界样本；
- premultiply 在 adapter 中单独测试，不能污染 canonical straight RGBA；
- alpha merge 后 RGB 不被无意 premultiply；
- color conversion profile 不同造成的允许差异必须逐 case 记录。

### 11.10 VP8 端到端门禁

- 官方 VP8 comprehensive vectors 全部进入预期路径；
- 原生 YUV 与 canonical oracle 逐字节一致；
- canonical RGBA profile 逐字节一致；
- token partition `1/2/4/8` 全覆盖；
- 所有 prediction/filter/coeff category 有 microvector；
- 每个小 fixture 全 truncation 不 panic；
- raw VP8、boolean decoder、coefficients、loop filter fuzz 均无已知 crash；
- scalar/native dispatch 输出一致。

---

## 12. `ALPH` 测试明细

覆盖 `ALPH` header 的所有字段：

- compression method 0：raw alpha；
- compression method 1：headerless VP8L，最终取 green channel；
- filter `0/1/2/3`：none/horizontal/vertical/gradient；
- preprocessing `0/1`；
- preprocessing reserved 值；
- reserved bits；
- payload 长度恰好、短 1、长 1；
- alpha plane 尺寸与 VP8 frame 不一致；
- `ALPH` 乱序、重复、出现在 VP8L 前；
- alpha 全 0、全 255、棋盘、单行、单列、随机；
- filter 的 `(0,0)`、第一行、第一列、内部点；
- gradient predictor clamp；
- filter forward -> inverse property；
- method 1 内部 VP8L 的 color cache/transform 特殊路径；
- ALPH 在 `ANMF` 内的 nested padding。

官方 test-data 中 alpha compression/filter 组合应全部纳入 smoke/full matrix。另写手工 1×1、1×2、2×1、2×2 向量，防止大图掩盖 border bug。

---

## 13. 动画与画布合成测试

动画必须有一个独立、极慢、逐像素的 reference compositor。生产 compositor 无论如何优化，都与该模型做 property differential。

### 13.1 frame header

覆盖：

- `ANIM` 必需/缺失/重复；
- loop count `0,1,2,65535`；
- background BGRA byte order；
- frame X/Y 的乘 2 语义；
- frame width/height `minus one`；
- frame duration 24-bit 边界；
- blend `0/1`；
- dispose `0/1`；
- reserved bits；
- frame rect 恰好贴边和超出 canvas；
- 0 frame、1 frame、多 frame；
- `ANMF` nested chunk padding；
- frame 中 VP8L、VP8、VP8+ALPH；
- frame payload 含 unknown chunk。

### 13.2 合成顺序

至少覆盖以下序列：

1. 全画布 replace；
2. 小矩形 replace；
3. 小矩形 alpha blend；
4. 上一帧 dispose none；
5. 上一帧 dispose background；
6. dispose 后当前帧 blend；
7. 透明 frame 覆盖不透明背景；
8. background alpha 非 255；
9. frame rect 重叠与不重叠；
10. 多次 loop 前 canvas 重置；
11. 第一帧不是全画布；
12. 只解第 N 帧时的依赖回放。

特别测试“先应用前一帧 disposal，再绘制当前帧”。将顺序反转的 fault 必须失败。

### 13.3 alpha blend 算法

container 规范建议在线性颜色空间合成，但实际生态可能存在非线性/整数兼容行为。建议显式提供：

```rust
pub enum AnimationBlendProfile {
    LibwebpCompatible,
    LinearSrgb,
}
```

首个 Skia replacement 以 `LibwebpCompatible` 为默认，并建立逐像素 oracle。`LinearSrgb` 若实现，应有独立数学 property 和高精度浮点/定点慢速模型。不能混用两套 golden。

测试输入应枚举 alpha：`0,1,127,128,254,255`，源/目标 RGB 边界，以及最终 alpha 为 0 的 RGB 归零规则。

### 13.4 duration 和 loop 的职责

codec 只保存 bitstream duration。浏览器将 0ms 或很小 duration clamp 到某个显示值属于播放器策略，不应写入 codec。测试必须保证 decode/encode 保留原始 24-bit duration，Skia/UI adapter 的 clamp 另设测试。

### 13.5 动画门禁

- 每个 raw frame metadata 与 oracle 一致；
- 每个 composited frame 与 selected blend profile 一致；
- one-shot 与逐帧 iterator 输出一致；
- random access frame 与从 0 顺序回放结果一致；
- animation roundtrip 保留 loop/duration/blend/dispose；
- frame optimization/subrect 编码不得改变最终 canvas；
- frame/pixel/total-work limits 在分配前生效。

---

## 14. 增量解码与状态机测试

增量 API 是最容易出现“普通文件能解，但流式输入失败”的部分。其测试不能只随机分块。

### 14.1 exhaustive split

对所有不超过约 64 KiB 的 smoke/regression fixture：

- 在每个 byte offset 做单次切分；
- 对极小 fixture 做所有二次切分组合；
- 每次只 push 1 byte；
- 在 chunk header、size、padding、VP8 partition、VP8L Huffman code、ANMF header 内切分；
- 插入任意数量空 `push(&[])`；
- 最后一次输入后调用 `finish`；
- 提前 `finish` 返回 `UnexpectedEof`；
- 未 finish 时短输入返回 `NeedMoreData` 而不是永久错误。

最终输出或最终错误分类必须与 one-shot decode 一致。

### 14.2 chunk plan property

生成：

```rust
struct ChunkPlan(Vec<usize>);
```

将同一输入按随机 plan 分块。属性：

```text
decode_one_shot(bytes) == decode_incremental(bytes, any_chunk_plan)
```

对于 malformed input，比对高层错误分类，不强求内部 offset 完全相同，除非 API 明确承诺。

### 14.3 复杂度

跟踪：

- 已消费输入字节；
- 内部复制字节；
- parser step；
- symbol decode count；
- 临时 buffer 峰值。

1-byte push 不应导致 O(n²) 重复移动或重解析。为每个 fixture 设置上界，例如内部复制不超过输入的固定倍数，parser step 与输入+输出规模线性相关。墙钟 benchmark只作辅助。

### 14.4 状态错误

覆盖：

- 完成后继续 push；
- error 后继续 push；
- 多次 finish；
- callback 用户中止；
- output buffer 太小；
- decoder drop 在每个 state；
- allocation failure 在每个 state；
- 多 decoder 并行运行；
- 共享只读 table 初始化竞争。

---

## 15. 编码器测试体系

### 15.1 通用原则

编码器输出不要求与 libwebp 字节相同，但必须满足：

1. ours 能解码；
2. pinned libwebp 能解码；
3. lossless 模式精确恢复；
4. lossy 模式的解码重建与编码器内部 reconstruction 一致；
5. 输出确定性策略明确；
6. 文件尺寸、质量和速度回归单独监控；
7. 每个编码选项都经过生成式组合测试。

### 15.2 VP8L 编码器

#### 正确性

对随机小图、generated patterns 和真实 corpus：

```text
source RGBA
  -> ours VP8L encode
  -> ours decode == source
  -> libwebp decode == source
```

必须保留 fully transparent 像素的 RGB，除非 API 显式提供非 exact 优化，并且默认/选项语义有测试。

#### 选项与路径

按 effort 层级覆盖：

- literal-only；
- subtract-green；
- predictor；
- color transform；
- palette；
- color cache；
- LZ77；
- meta prefix groups；
- transform stacking；
- metadata wrapping；
- animation frame payload。

不能只检查 encoder 自己选择了某路径。应提供测试-only strategy override，强制每个合法路径，以便低频分支可测。

#### 确定性

同输入、同配置：

- 重复 100 次 bytes 相同；
- thread count `1/N` 相同，或文档明确说明不承诺 bitstream determinism；
- x86/aarch64/wasm 相同；
- scalar/SIMD 相同；
- HashMap iteration 不影响输出；
- 随机搜索必须显式 seed 并记录。

建议将 deterministic output 作为默认契约，因为它极大简化 golden、缓存和回归分析。

#### 压缩率

压缩率不是单图硬门禁。按 corpus category 统计：

- output bytes；
- 相对 pinned libwebp 同档配置；
- 相对上一发布版本；
- p50/p90/worst-case；
- 编码时间和峰值内存。

首个版本可以较大，但任何提交不得在未解释的情况下让 corpus 总体体积或 worst-case 大幅回退。

### 15.3 VP8 有损编码器

#### bitstream 合法性

- ours/libwebp/libvpx 均可解码；
- 只产生 WebP 支持的 key frame；
- partition、probability update、segment、filter 均合法；
- alpha plane 独立验证；
- internal reconstruction 与独立 decoder 的 YUV 完全一致；
- target size/quality 参数的边界不会溢出或死循环。

#### 质量测试

不能用“quality 值越大，每张图文件一定越大”作为硬属性，因为模式决策可能导致局部非单调。使用 corpus 级 rate-distortion：

- 固定质量点，例如 `0, 10, 25, 50, 75, 90, 100`；
- 固定 target-size 点；
- 记录 Y-PSNR、RGB-PSNR、SSIM，必要时增加 MS-SSIM/感知指标；
- 对照片、UI/文本、线稿、噪声、渐变、透明图分别统计；
- 与上一发布版本和 pinned libwebp 绘制 RD curve；
- 用 aggregate threshold 判定，而不是单张图的偶然波动；
- 保存最差退化样本的 artifact，人工查看后再批准。

质量 gate 示例：在固定 corpus 和相近总字节数下，Y-PSNR/SSIM aggregate 不得超过预设回退；或使用 BD-rate/BD-quality 衡量整条曲线。阈值应在积累首个稳定 baseline 后锁定，不要在实现尚未稳定时拍脑袋写一个永久数字。

#### 透明度

- lossless alpha 与 lossy RGB 的组合；
- alpha quality 全范围；
- RGB fully transparent 区域的 exact/non-exact 选项；
- premultiply 不进入 encoder core；
- edge color bleed 测试。

### 15.4 动画编码器

- 每帧使用 VP8/VP8L/auto；
- loop、duration、blend、dispose 原样；
- 子矩形优化前后最终 canvas 一致；
- 连续相同帧合并不能改变时间线；
- disposal 优化的 reference compositor 验证；
- ours/libwebp 解码每帧一致；
- metadata/unknown chunk 策略明确；
- 大量 frame 受 total pixel/work budget 限制。

---

## 16. Property-based testing 方案

推荐 `proptest`。生成器不能只有“任意 bytes”，应同时存在 value-domain generator 和 syntax-aware generator。

### 16.1 基础 property

```text
BitWriter(values) -> BitReader == values
RiffWriter(ast) -> RiffParser == normalized(ast)
VP8L forward_transform -> inverse_transform == original
Alpha forward_filter -> inverse_filter == original
Lossless encode -> decode == original RGBA
Incremental(any partition) == one-shot
SIMD(valid input) == scalar(valid input)
Mux(demux(file)) preserves image and selected chunks
Animation optimized compositor == slow compositor
```

### 16.2 小域穷举优先

随机 property 不容易撞到单一极值。例如 `u32::MAX`、Huffman repeat 恰好越界、alpha 255 或 width 1 应通过显式 `prop_oneof!`/weighted strategies 高概率生成，并保留表驱动测试。

对以下小域可直接穷举：

- predictor 邻居每通道选 `{0,1,127,128,254,255}` 的缩小组合；
- alpha filter 的 1×1、1×2、2×1；
- color cache bits 1..4 的短序列；
- palette size 1..17 的 packing；
- animation 2×2 canvas、1..3 frame、alpha 小集合；
- checked arithmetic 的边界集合；
- Huffman alphabet 缩小版的全部 complete trees。

### 16.3 shrinker

默认 byte shrink 会把合法 WebP 很快缩成“magic 错误”，失去深层路径。应为以下结构写自定义 shrink：

- `RiffAst`：删除非关键 chunk、缩短 payload、减小尺寸但保持 layout 合法；
- `Vp8lCase`：减少像素、transform、prefix group、symbol 序列；
- `AnimationCase`：减少 frame、缩小 rect、简化 alpha；
- `DecodeCase`：缩小 options、chunk plan、limits；
- lossy source：缩小宽高、颜色集合、quality 配置。

失败最小化的目标不是文件字节最短，而是仍能进入 root-cause stage 的最小语义输入。

### 16.4 regression seed 固化

`proptest-regressions` 必须提交。重要失败还要转成命名 fixture，因为 seed 文件不够直观，也可能随 generator 重构失效。

---

## 17. Metamorphic testing 方案

没有唯一 oracle 时，metamorphic relation 很有价值。

### 17.1 容器关系

在规范允许的位置插入 unknown chunk，不改变像素；metadata chunk 改变不应改变 image bitstream；只读 metadata 不应解码像素；合法重新排序 metadata/unknown chunks 不改变像素；mux 后再次 demux 应保持受支持 chunk。

### 17.2 解码关系

- one-shot 与任意 incremental split 一致；
- scalar、各 SIMD dispatch、thread count 一致；
- full-image crop 等于不 crop；
- 子区域直接 decode 的输出等于 full decode 后切片，但颜色上采样/crop 规则如果不同，必须限定到保证成立的 API profile；
- 解码到 RGBA 后转换 BGRA 等于直接 BGRA API；
- read_info 的尺寸与完整 decode 一致；
- read_metadata 不影响后续 decode；
- 同一 malformed 输入重复运行得到同一错误分类和 bounded work。

### 17.3 编码关系

- lossless encode 的任何合法 effort 都精确往返；
- 加/去 metadata 不改变解码像素；
- animation 子矩形优化不改变 composited frames；
- scalar/SIMD encoder 在 deterministic mode 产生同一 bytes；
- target-size 搜索的输出必须可解且不超过明确的算法容差；
- lossy 内部 reconstruction 与解码输出一致。

---

## 18. Fuzzing 总体架构

`cargo-fuzz`/libFuzzer 是首选本地工具。项目成熟后申请 OSS-Fuzz；在此之前，CI 中运行短时 fuzz，专用机器运行长时 fuzz。

不要把全部逻辑塞进一个 `decode_any` target。浅层 parser 错误会吞噬 coverage，复杂模块需要独立 target。

### 18.1 推荐 fuzz targets

| Target | 输入 | 主要不变量 |
|---|---|---|
| `container_raw` | 任意 bytes | 无 panic、无超限、解析终止 |
| `container_ast` | 结构化 RIFF AST | serialize/parse、严格/兼容策略 |
| `decode_any` | 任意 bytes + options | public API 安全性质 |
| `decode_incremental` | bytes + chunk plan | 与 one-shot 一致 |
| `read_info_metadata` | 任意 bytes | 不分配像素、无深层 decode |
| `vp8l_raw` | VP8L bytes | 无 panic、bounded work |
| `vp8l_huffman` | code-length AST | table/read 与慢速模型一致 |
| `vp8l_lz77` | symbol sequence | copy/cache 与慢速模型一致 |
| `vp8l_transforms` | pixels + transform config | inverse/roundtrip |
| `vp8_raw` | VP8 key frame bytes | 无 panic、bounded work |
| `vp8_bool` | bytes + probability sequence | 与慢速 decoder 一致 |
| `vp8_coefficients` | token stream/context | block bounds、oracle vector |
| `vp8_loop_filter` | pixel neighborhoods + params | scalar/reference 一致 |
| `alpha_raw` | ALPH payload + dimensions | filter/method 安全与差分 |
| `animation_compose` | structured frames | optimized == slow model |
| `mux_demux` | structured chunks | preserve/canonical properties |
| `lossless_roundtrip` | small RGBA + options | 双 decoder 精确恢复 |
| `lossy_encode_decode` | small image + options | bitstream valid、recon 一致 |
| `animation_roundtrip` | small frame sequence | composited frames 一致 |
| `simd_vs_scalar` | DSP input structs | bit-exact |
| `limits_allocator` | bytes + limit profile + fail point | 正确错误、无 panic/leak |

首版至少完成 `container_raw`、`decode_any`、`vp8l_huffman`、`vp8l_raw`、`decode_incremental`；VP8 模块合并前补齐相应 targets。

### 18.2 raw 与 structured fuzz 必须并存

Raw bytes 擅长发现 parser、EOF、长度和状态错误，但很难穿过合法 Huffman/VP8 header。Structure-aware fuzz 通过 AST 或自定义 mutator 保持大部分结构合法，集中攻击深层逻辑。

建议实现两类 mutator：

1. parse-mutate-serialize：尽量解析为 AST，改变一个字段、chunk、tree 或 frame 后重写；
2. decode-mutate-reencode：对可解码输入先改像素/选项，再由一个受控 encoder 生成合法流。

第二类不能替代 bitstream-level generator，因为 encoder 通常不会生成极端不平衡 Huffman tree、重复 simple symbol 或罕见合法布局。

### 18.3 fuzz dictionary

至少包含：

```text
"RIFF", "WEBP", "VP8 ", "VP8L", "VP8X", "ALPH",
"ANIM", "ANMF", "ICCP", "EXIF", "XMP ",
0x2f, 0x9d012a,
常见 24/32-bit size 字节模式，
ALPH header 的 filter/compression 组合，
VP8L code-length repeat symbol 16/17/18 的编码片段。
```

可从 libwebp 的 fuzz dictionary 借鉴 token，但应将来源和许可证记录在工具目录。

### 18.4 seed corpus

每个 target 的 seed 只放能增加深层 coverage 的小输入。启动 corpus 来自：

- smoke fixtures；
- upstream conformance vectors 的最小子集；
- 每种格式特征一个 handcrafted seed；
- 历史 crash；
- structure-aware generator 输出；
- fuzzer `cmin` 后的 coverage-minimized 集合。

不要把数千张相似真实图片直接复制给每个 target，否则启动、同步和 minimization 成本很高。

### 18.5 differential fuzz 的隔离

任意 malformed bytes 不建议在同一进程直接调用 C oracle。即使固定的是已修复 libwebp，oracle 仍可能崩溃，导致无法判断是 Rust 实现还是 oracle 的问题。

采用：

- raw malformed target：只 fuzz Rust 实现，检查安全性质；
- structured-valid target：可在同进程调用 patched libwebp 做高吞吐差分；
- uncertain/malformed differential：将 oracle 放在子进程，设置 timeout/RSS limit，分别保存双方结果；
- C oracle 构建额外启用 ASan/UBSan，用于发现 oracle 自身问题，但不将其 crash 自动记为 Rust bug。

### 18.6 target 资源控制

每个 target 应限制：

- 最大输入尺寸；
- 最大生成图像尺寸；
- 最大 frame 数；
- encoder effort；
- libFuzzer timeout；
- RSS；
- 每 case deterministic work budget。

libwebp 自己的 encode/decode fuzzer也会在大图时降低最慢的编码设置，说明 fuzz harness 必须防止少数高成本 case 消耗全部预算。限制应在 harness 中显式记录，不能静默跳过所有复杂路径。

### 18.7 fuzz crash 处理标准

任何 panic、abort、timeout、OOM、输出不一致或违反 property 都是 crash。修复完成的标准是：

- 最小化输入可稳定重现；
- root cause 已分类；
- 添加命名 regression fixture；
- 添加更一般的 property/fault test；
- 原 target 在修复 commit 上重跑；
- 相关 targets 长时运行无同类 crash；
- 如果 crash 来自资源预算不合理，修改预算模型并加入上界测试，而不是简单扩大 timeout。

---

## 19. 系统化 malformed 与 adversarial 测试

Fuzz 之外还要有确定性的 mutation suite。

### 19.1 全截断

对每个 smoke fixture：

```text
for n in 0..file.len() {
    decode(&file[..n])
}
```

对 bit-sensitive micro fixture 还要在最后 1 字节中逐 bit 清除/截断。所有结果必须是成功或受控错误，绝不能 panic、无限循环或超过 manifest 预算。

### 19.2 单 bit flip

对小 fixture 的每一 bit 做 flip，并记录：

- 成功且输出变化；
- 成功且输出不变；
- 受控拒绝；
- 资源拒绝。

不要求所有 bit flip 都拒绝，但任何 crash 或预算失控都是 bug。对成功且输出不变的情况检查是否仅修改 padding/reserved/unknown metadata，否则可能暴露未读取字段。

### 19.3 长度字段攻击

对每个 RIFF/chunk/partition/frame length 改成：

```text
0, 1, actual-1, actual, actual+1,
0x7fffffff, 0x80000000, 0xfffffffe, 0xffffffff
```

并测试多个长度字段相互矛盾、嵌套 ANMF size、odd padding、trailing data。

### 19.4 尺寸与乘法攻击

- width/height 各自小但 product 大；
- VP8X canvas 大、frame 小；
- frame rect origin+extent overflow；
- animation 多 frame 总像素超限；
- metadata 和 pixel allocation 组合超限；
- 32-bit target 上 `usize` 截断；
- stride/row bytes 溢出；
- dimensions 通过 header 但在 transform 后派生尺寸溢出。

### 19.5 算法复杂度攻击

构造：

- 大量极短 unknown chunks；
- 大量 frame；
- 最大数量 prefix groups；
- 极端 Huffman tree；
- 大量单像素 LZ77/literal；
- 1-byte incremental push；
- encoder 中导致候选搜索爆炸的图像；
- target-size 搜索无法收敛的参数。

用 deterministic work counter 断言复杂度上界。独立子进程 wall-clock timeout 只作为第二道防线。

### 19.6 小栈测试

在自定义小 stack thread 中运行深层/恶意 fixture，确认 parser 不依赖输入控制的递归。VP8L 子图像解析应使用显式状态或严格固定深度；动画 frame 数不能映射为递归调用深度。

---

## 20. Mutation testing 与 codec 专用 fault injection

推荐同时使用 `cargo-mutants` 和人工 fault patch。通用 mutation 工具适合比较符、常量、返回值和分支；codec 特有错误需要人工定义。

### 20.1 指标

- `bitreader`、checked arithmetic、Huffman、LZ77 bounds、animation rect/compositor：至少 95% 可杀死 mutants；
- container、VP8L transforms、ALPH：至少 90%；
- 整体核心：至少 85%；
- surviving mutant 必须逐个标注为 equivalent、unreachable、性能-only 或真实测试缺口；
- critical module 不允许存在未解释 surviving mutant。

### 20.2 人工 fault catalog

在 `tools/faults/` 维护 patch，例如：

```text
riff-padding-ignore.patch
chunk-size-header-included.patch
vp8l-bit-order-msb.patch
vp8l-repeat16-off-by-one.patch
vp8l-huffman-small-table.patch
vp8l-distance-map-sign.patch
vp8l-cache-update-batch.patch
predictor-top-right-border.patch
color-transform-unsigned-mult.patch
vp8-bool-split-le.patch
vp8-idct-rounding-minus-one.patch
vp8-loopfilter-edge-swap.patch
alpha-gradient-no-clamp.patch
anim-dispose-after-draw.patch
anim-frame-offset-no-times-two.patch
premultiply-rounding.patch
limit-width-times-height-wrap.patch
incremental-reparse-prefix.patch
```

CI 定期应用每个 patch，预期测试失败。如果 patch 应用后测试仍通过，说明测试体系无法检测这一类真实错误。

### 20.3 避免把 fault 分支放进生产 binary

最好以 patch 或 mutation tooling 实现，不在生产代码加入 `#[cfg(fault = ...)]` 的大量分支。确需 compile-time fault injection 时，保证发布 profile 无相关 feature，且 `cargo package` 检查不会包含测试故障路径。

---

## 21. Miri、sanitizer 与模型检查

### 21.1 Miri

每晚或每周运行关键小测试：

```text
cargo +nightly miri test -p webp-container
cargo +nightly miri test -p webp-vp8l huffman
cargo +nightly miri test -p webp-dsp scalar
```

即使核心无 unsafe，Miri 仍能帮助检查越界、无效 intrinsic 前置条件和未来 SIMD/FFI wrapper。利用 Miri 的 `s390x-unknown-linux-gnu` 支持运行 endian-sensitive 单元测试，确保没有依赖 host `u32` byte order。

### 21.2 sanitizer

对含 unsafe、SIMD、C oracle、Skia adapter 的构建运行：

- AddressSanitizer；
- LeakSanitizer；
- MemorySanitizer（能全量 instrument 时）；
- ThreadSanitizer（引入线程、共享 cache 或并行编码后）；
- C/C++ oracle 侧额外启用适用的 UBSan。

Sanitizer 不是 safe Rust 测试的替代，而是验证 FFI、intrinsic 和外部库边界。

### 21.3 Kani

优先给纯函数写 bounded proof harness：

- `checked_image_bytes` 不溢出且结果满足定义；
- chunk end/padding 计算；
- animation rect containment；
- small-domain Huffman table build 不越界；
- LZ77 `distance <= produced` 时每次读取都在已初始化区域；
- palette unpack 对任何合法 width/index 不越界；
- alpha filter 小尺寸边界；
- premultiply/unpremultiply 算术范围。

Kani 证明是关键原语的补强，不要求对完整 VP8 decoder 做全状态 model checking。

---

## 22. 跨平台与 SIMD 测试矩阵

### 22.1 平台

最低矩阵建议：

- Linux x86_64；
- Linux aarch64；
- macOS aarch64；
- Windows x86_64；
- i686 或其他 32-bit target；
- big-endian s390x（Miri 或 QEMU）；
- wasm32；
- Android aarch64（Skia 目标需要时）。

每个平台运行 smoke/golden；full corpus 可在 Linux x86_64 和 aarch64 重点运行，release 前再扩展全矩阵。

### 22.2 强制 dispatch

不要只依赖当前 CPU 自动选择。测试 API 提供：

```rust
pub enum CpuBackend {
    Scalar,
    Sse2,
    Ssse3,
    Avx2,
    Neon,
    WasmSimd128,
    Auto,
}
```

仅 test/bench feature 暴露强制 backend。所有支持 backend 对同一 corpus 与 scalar bit-exact。不能在测试机不支持某 feature 时非法执行 intrinsic；可交叉编译并在对应 runner/模拟器运行。

### 22.3 字节序与像素表示

禁止通过 `transmute::<u32, [u8;4]>` 隐式定义公共像素顺序。内部 packed ARGB 可以使用，但读写通道应通过显式 shift/mask，并用 big-endian tests 验证。所有 golden 采用明确的 byte order 名称。

### 22.4 并发与重入

- 同一 input 在多个线程同时 decode；
- 不同 input 并行；
- shared static table/`OnceLock` 首次初始化竞争；
- encoder thread count 变化；
- callback 在其他线程触发取消；
- TSan 无 data race；
- 无全局可变“当前 CPU backend”影响其他测试，强制 backend 应为 decoder-local 或 test process-local。

---

## 23. 性能、内存与回归基准

### 23.1 性能测试与正确性测试分开

功能 CI 不因普通云 runner 的 5% 抖动失败。性能 gate 在固定 runner 上执行，记录硬件、OS、toolchain、CPU governor 和 oracle build。

### 23.2 corpus 分类

至少有：

- tiny：1×1、16×16、图标；
- medium：256/512；
- large：2K/4K；
- photo；
- UI/text；
- line art/palette；
- random/noise；
- alpha gradient；
- animated partial frames；
- malformed complexity cases。

### 23.3 指标

Decoder：

- MB/s、MPix/s；
- first-pixel/first-frame latency；
- one-shot 与 incremental；
- allocations count；
- peak allocated bytes/RSS；
- scalar vs native SIMD；
- output format conversion成本。

Encoder：

- MPix/s；
- output bytes；
- quality metrics；
- peak memory；
- thread scaling；
- deterministic mode overhead；
- target-size search iterations。

### 23.4 回归门禁建议

初始阶段先记录不阻塞。稳定后设置：

- 单项 median 退化超过 10% 自动失败；
- 5%..10% 需要人工批准；
- corpus aggregate 和 worst-case 同时看；
- 新增更正确的检查若导致性能下降，可以批准，但必须记录；
- SIMD 上线前必须证明输出一致，再谈速度；
- 内存峰值回归比速度更严格，任何超预算直接失败。

目标可分阶段：标量正确版允许慢于 libwebp；Skia production gate 再要求关键 corpus 接近既有后端。不要为了提前达到 benchmark 而合并未经证明的 unsafe/SIMD。

---

## 24. Skia adapter 专项测试

Skia integration 不应直接污染 codec core。`webp-skia` 负责 CXX/FFI、SkImageInfo、rowBytes、alpha type 和 frame API 映射。

### 24.1 FFI 安全

- 所有 Rust entry point 使用 `catch_unwind` 或保证编译配置/边界使 panic 不穿越 FFI；
- null pointer、length mismatch、unaligned output、过小 rowBytes；
- C++ callback 抛异常/取消的策略；
- Rust error 到 Skia result code 映射；
- drop/ownership；
- ASan/TSan；
- 同一 codec object 的允许/禁止并发使用。

### 24.2 像素接口

对每种支持的 SkColorType/SkAlphaType：

- RGBA/BGRA channel order；
- opaque/unpremul/premul；
- rowBytes 大于最小值；
- output buffer 尾部 guard bytes 不被写；
- subset/crop；
- odd dimensions；
- color profile 的传递与应用边界；
- fully transparent RGB 的处理；
- scalar codec output 到 Skia output 的独立 conversion golden。

### 24.3 A/B 验证

在同一 Skia revision 构建两个后端：

```text
Skia + libwebp
Skia + webp-rs
```

对 corpus 比较：

- `getInfo`；
- frame count、duration、required frame、loop count；
- full decode；
- incremental decode；
- subset/scale（如果 adapter 支持）；
- incomplete input；
- alpha type；
- ICC behavior；
- error category。

Pixel comparison 应明确：VP8L/alpha/animation compat profile 逐字节；VP8 RGBA 先以 pinned scalar libwebp profile 为 canonical。任何容差都必须按颜色转换路径局部定义。

### 24.4 切换门禁

- 公开/内部 corpus 100% 无 crash；
- must-accept 100%；
- pixel mismatch 全部归因并有 manifest；
- Android/Linux/macOS 主平台性能和内存达标；
- 连续 fuzz 周期无高严重度问题；
- FFI sanitizer clean；
- 可通过 feature flag 快速回退到 libwebp，至少保留一个发布周期。

---

## 25. CI 分层

### 25.1 每个 PR

目标是约 10–20 分钟内给出强反馈：

```text
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo test --workspace --no-default-features
cargo nextest run <smoke + regression + small differential>
cargo xtask corpus verify --smoke
cargo xtask feature-matrix check
git diff origin/main...HEAD > git.diff
cargo mutants --in-diff git.diff
short fuzz smoke for affected targets
cross-check 32-bit compilation
```

对关键 primitive 的 Miri test 可放 PR；较慢的全 Miri 放 nightly。

### 25.2 Nightly

- full upstream/generated corpus；
- full libwebp differential；
- proptest 高 case 数、固定 seed + 随机 seed；
- 所有 fuzz target 分片运行；
- sanitizer jobs；
- Miri critical suite；
- Kani selected harness；
- aarch64 runner；
- benchmark recording；
- allocation failure sweep；
- all-split incremental suite；
- corpus feature coverage 报告。

### 25.3 Weekly

- long fuzz campaign；
- full cargo-mutants；
- codec fault patch catalog；
- big-endian/32-bit/wasm；
- Windows/macOS full smoke；
- performance/RD full corpus；
- CVE/regression audit；
- fuzz corpus minimize/merge；
- dependency/license/audit。

### 25.4 Release candidate

发布候选应冻结：源码 SHA、Rust toolchain、oracle SHA、corpus lock、benchmark baseline。所有输出 artifact 带版本信息。RC 期间原则上只接受带回归 fixture 的修复。

---

## 26. 量化验收门禁

下表是推荐起点，可按资源调整，但不能取消类别。

| 类别 | VP8L milestone | Full decoder milestone | Skia production milestone |
|---|---:|---:|---:|
| MustAccept fixtures | 100% | 100% | 100% |
| MustReject fixtures | 100% | 100% | 100% |
| Panic/abort on arbitrary input | 0 | 0 | 0 |
| VP8L RGBA differential | 100% exact | 100% exact | 100% exact |
| VP8 native YUV differential | N/A | 100% exact | 100% exact |
| VP8 canonical RGBA | N/A | 100% exact或逐例豁免 | 100% exact或批准清单 |
| Animation composited frames | N/A | 100% selected profile | 100% compat profile |
| Incremental vs one-shot | VP8L smoke全 split | 全 smoke/regression全 split | 全迁移 corpus |
| Critical mutation score | ≥95% | ≥95% | ≥95%，无未解释 mutant |
| Overall core mutation score | ≥85% | ≥85% | ≥90%目标 |
| Resource budget regressions | 0 | 0 | 0 |
| Scalar/SIMD mismatch | N/A/0 | 0 | 0 |
| Known regression fixtures | 100% | 100% | 100% |
| FFI sanitizer failures | N/A | N/A | 0 |

Fuzz 时间建议分级：

- PR：受影响 target 1–5 分钟 smoke；
- nightly：每个 target 15–60 分钟，按 shard 并行；
- RC：累计至少数百到上千 CPU-hours，并观察 coverage 是否仍增长；
- production：接入 OSS-Fuzz 或等价持续系统，至少经历一个稳定观察周期后再默认替换。

“运行了 24 小时”不是充分证明。必须同时查看 unique paths、feature matrix、coverage plateau、exec/s、timeout/RSS 和 crash triage 质量。

---

## 27. 实施里程碑与每阶段退出条件

### M0：测试地基与容器

交付：workspace、oracle build、corpus lock、fixture manifest runner、bit reader、checked arithmetic、RIFF/VP8X parser、初始 fuzz/CI。

退出条件：

- official smoke corpus 可自动下载/验证；
- `container_raw` 连续 fuzz 无 crash；
- 所有 RIFF truncation 测试通过；
- oracle 输出可复现；
- allocation/work budget 框架已进入公共 API；
- fault catalog 至少覆盖 bit order、size overflow、padding。

### M1：VP8L production decoder

交付：完整 VP8L、metadata、静态文件。

退出条件：第 10.11 节门禁全部满足；不得以“后续再补 fuzz/Huffman tests”退出。

### M2：VP8 production decoder

交付：VP8 key frame、原生 YUV、canonical RGBA。

退出条件：第 11.10 节门禁全部满足，且 VP8 与颜色转换错误能被分层定位。

### M3：ALPH、动画、增量

退出条件：所有 frame composited golden、全 split incremental、frame limits 和 random access 测试通过。

### M4：VP8L 编码器

退出条件：双 decoder lossless roundtrip、所有 strategy path 可强制测试、确定性和压缩率 baseline 建立。

### M5：VP8 编码器

退出条件：独立 decoder 接受、internal reconstruction一致、RD baseline 和 regression gate 建立。

### M6：SIMD、Skia 与 hardening

退出条件：scalar/SIMD bit-exact、跨平台、FFI sanitizer、Skia A/B corpus、长时 fuzz 和性能/内存门禁通过。

---

## 28. Codex/多代理任务拆分建议

并行代理只能处理接口稳定的模块。主代理/技术负责人先冻结以下契约：pixel representation、error kind、limits、bit reader 语义、fixture manifest 和 oracle result schema。

可并行 workstream：

- Agent A：container、metadata、mux/demux；
- Agent B：VP8L Huffman/entropy；
- Agent C：VP8L transforms/LZ77/cache；
- Agent D：VP8 boolean/headers/partitions；
- Agent E：VP8 prediction/transforms/filter；
- Agent F：ALPH/animation compositor；
- Agent G：oracle/corpus/differential runner；
- Agent H：fuzz/property/mutation infrastructure；
- Agent I：Skia adapter；
- Agent J：bench/SIMD，必须在 scalar 合格后开始。

每个任务 prompt 必须包含：

```text
实现范围
不可修改的接口
规范章节
必须新增的 fixture
必须新增的 unit/property/fuzz tests
oracle 比较层级
资源预算
退出命令
禁止降低或删除的现有测试
```

Codex 完成任务后，主代理不只看“测试绿色”，还检查：测试在修复前是否会失败、是否只测试实现自身、是否吞掉错误、是否放宽容差、是否扩大 timeout/limits 来掩盖问题。

---

## 29. 建议的测试代码骨架

### 29.1 统一可比较输出

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalDecode {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub yuv: Option<CanonicalYuv>,
    pub metadata: OwnedMetadata,
    pub frames: Vec<CanonicalFrame>,
}

pub trait DecodeOracle {
    fn name(&self) -> &'static str;
    fn decode(&self, bytes: &[u8], case: &OracleCase)
        -> Result<CanonicalDecode, OracleError>;
}
```

所有 decoder 先转换到 canonical result，再比较。不要直接比较不同 API 的内部 struct。

### 29.2 差分断言

```rust
fn assert_decode_matches_oracle(case: &Fixture, profile: CompareProfile) {
    let ours = decode_canonical(&case.bytes, &case.options);
    let reference = libwebp_oracle().decode(&case.bytes, &case.oracle_case);

    match case.class {
        FixtureClass::MustAccept => {
            let ours = ours.expect("ours must accept fixture");
            let reference = reference.expect("oracle must accept fixture");
            compare_canonical(&ours, &reference, profile)
                .unwrap_or_else(|diff| panic!("{}: {diff:#?}", case.id));
        }
        FixtureClass::MustReject => {
            assert!(ours.is_err(), "{} unexpectedly accepted", case.id);
        }
        FixtureClass::CompatAccept => {
            // 在 compatible profile 中是硬门禁；strict profile 可拒绝。
        }
        FixtureClass::ImplementationDefined => {
            assert_bounded_and_no_panic(case);
        }
    }
}
```

### 29.3 structured fuzz

```rust
#![no_main]
use libfuzzer_sys::fuzz_target;

#[derive(arbitrary::Arbitrary, Debug)]
struct DecodeCase<'a> {
    bytes: &'a [u8],
    chunks: Vec<u16>,
    limit_profile: u8,
}

fuzz_target!(|case: DecodeCase<'_>| {
    let limits = limits_from_profile(case.limit_profile);
    let one_shot = webp::decode(case.bytes, &limits);
    let streamed = decode_with_chunk_plan(case.bytes, &case.chunks, &limits);
    assert_same_public_result(one_shot, streamed);
});
```

实际 target 要限制 `chunks` 数量和总 work，避免生成器自身耗尽资源。

### 29.4 feature matrix

维护机器可读文件：

```yaml
vp8l:
  predictor:
    modes: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]
    tests:
      - unit::predictor_all_modes
      - corpus::lossless_vectors
      - fuzz::vp8l_transforms
  color_cache:
    valid_bits: [1,2,3,4,5,6,7,8,9,10,11]
    invalid_bits: [0,12,13,14,15]
    tests:
      - unit::color_cache_hash
      - property::cache_model
      - fuzz::vp8l_lz77
```

`cargo xtask feature-matrix check` 验证每个 required feature 至少关联 unit/conformance/fuzz 中的两类，避免文档与测试脱节。

---

## 30. 风险清单

### 30.1 只比较最终 RGBA

风险：内部 YUV、alpha、prediction 或 transform 错误可能互相抵消。对策：为关键阶段生成中间 microvector，并对 VP8 先比较 YUV。

### 30.2 只用 libwebp 当唯一真相

风险：复制 oracle 行为、继承宽松 bug、无法解释平台颜色差异。对策：规范分类、手工 vector、慢速模型、可选第二 oracle。

### 30.3 safe Rust 等于安全

风险：panic、OOM、CPU DoS、巨大 allocation、逻辑越界仍可用。对策：limits、work budget、fallible allocation、subprocess timeout、fuzz。

### 30.4 测试只覆盖 encoder 会生成的流

风险：自家 encoder 不会生成罕见合法 Huffman/transform layout。对策：bitstream builder、官方 vectors、structure-aware fuzz。

### 30.5 过早 SIMD

风险：调试维度翻倍，平台不一致掩盖算法错误。对策：标量先通过完整门禁，SIMD 永远与标量 bit-exact。

### 30.6 容差不断放宽

风险：为了让差分绿色，将所有 RGBA 比较改成每通道误差。对策：容差只能存在于命名 profile 和逐 case manifest，VP8L/alpha/YUV 不允许模糊容差。

### 30.7 fuzz 运行但进不去深层路径

风险：看似运行很久，coverage 停在 magic/header。对策：granular target、structure-aware mutator、seed cmin、feature coverage。

### 30.8 回归只保存 crash bytes

风险：稍微重构后输入不再到达同一 root cause。对策：同时添加 generic property 和 fault patch。

### 30.9 Oracle 漂移

风险：开发机 `dwebp` 版本不同，golden 自动变化。对策：源码 SHA、构建镜像和 output hash 全固定。

### 30.10 Skia 行为与 codec 语义混合

风险：premultiply、duration clamp、颜色管理侵入核心，使 standalone 行为难以定义。对策：adapter 分层和独立 A/B tests。

---

## 31. 最小可执行命令集合

建议最终提供：

```bash
# 基础检查
cargo xtask check

# 小型、无 C oracle
cargo nextest run --profile ci

# 固定 oracle 差分
cargo xtask oracle build
cargo xtask corpus fetch
cargo xtask differential --suite smoke
cargo xtask differential --suite full

# 全截断/bit mutation
cargo xtask adversarial truncation --suite smoke
cargo xtask adversarial bitflip --suite micro

# fuzz
cargo fuzz run decode_any
cargo fuzz run vp8l_huffman
cargo fuzz run decode_incremental

# 回归重放
cargo xtask regressions replay

# mutation/fault
cargo mutants -p webp-vp8l
cargo xtask faults verify

# Miri/Kani/sanitizer
cargo +nightly miri test -p webp-vp8l
cargo kani -p webp-container
cargo xtask sanitizer address

# benchmark/RD
cargo xtask bench decode
cargo xtask bench rd

# Skia A/B
cargo xtask skia compare --suite migration
```

所有 `xtask` 命令都应输出机器可读 JSON 和人类可读摘要，CI 保存原始 artifact。

---

## 32. 首批应立即编写的测试清单

在开始大规模实现前，先完成以下 25 项：

1. bit reader 逐 bit reference differential；
2. checked `width * height * channels`；
3. RIFF 每 byte truncation；
4. odd chunk padding；
5. chunk size overflow；
6. VP8X canvas product limit；
7. unknown chunk preserve；
8. fixture manifest loader；
9. pinned libwebp build；
10. canonical oracle result；
11. official test-data smoke subset；
12. raw `decode_any` fuzz；
13. `container_raw` fuzz；
14. allocation/work budget；
15. failing allocator test binary；
16. regression fixture schema；
17. feature matrix checker；
18. proptest regression persistence；
19. scalar DSP reference module；
20. incremental chunk-plan harness；
21. full truncation xtask；
22. bitflip xtask；
23. cargo-mutants config；
24. first 5 codec-specific fault patches；
25. CI artifact/report format。

完成这些后，Codex 实现 VP8L Huffman、transform 和 LZ77 才有可靠反馈闭环。

---

## 33. 资料与实现基线

以下资料是本方案的规范和测试基线；实施时应将实际使用的版本/commit 写入 lock file。

1. WebP Container Specification  
   https://developers.google.com/speed/webp/docs/riff_container

2. WebP Lossless Bitstream Specification  
   https://developers.google.com/speed/webp/docs/webp_lossless_bitstream_specification

3. RFC 6386 — VP8 Data Format and Decoding Guide  
   https://datatracker.ietf.org/doc/html/rfc6386

4. libwebp reference source  
   https://chromium.googlesource.com/webm/libwebp/

5. libwebp tests/fuzzer  
   https://chromium.googlesource.com/webm/libwebp/+/HEAD/tests/

6. libwebp official test data  
   https://chromium.googlesource.com/webm/libwebp-test-data/

7. CVE-2023-4863 对应的 libwebp Huffman table 修复 commit  
   https://chromium.googlesource.com/webm/libwebp/+/2af26267cdfcb63a88e5c74a85927a12d6ca1d76

8. Rust Fuzz Book / cargo-fuzz  
   https://rust-fuzz.github.io/book/cargo-fuzz.html

9. Structure-aware fuzzing  
   https://rust-fuzz.github.io/book/cargo-fuzz/structure-aware-fuzzing.html

10. OSS-Fuzz  
    https://google.github.io/oss-fuzz/

11. Miri  
    https://github.com/rust-lang/miri

12. Rust sanitizer documentation  
    https://doc.rust-lang.org/beta/unstable-book/compiler-flags/sanitizer.html

13. Kani Rust Verifier  
    https://model-checking.github.io/kani/

14. proptest  
    https://github.com/proptest-rs/proptest

15. cargo-mutants  
    https://github.com/sourcefrog/cargo-mutants

可参考但不应作为规范真相的纯 Rust 项目：

- image-rs/image-webp： https://github.com/image-rs/image-webp
- P4suta/webpkit： https://github.com/P4suta/webpkit
- OxideAV/oxideav-webp： https://github.com/OxideAV/oxideav-webp

研究或复用任何代码、测试向量和 fuzz dictionary 前，必须确认对应许可证并保留 attribution。发布 crate 不应意外包含 test-only `libwebp` 链接或不兼容许可证代码。

---

## 34. 最终执行原则

这个项目的主要风险不是 Codex 写不出函数，而是实现迅速增长后，团队无法判断某次“修复”是否真正正确。解决办法不是更多普通单元测试，而是一套分层、独立且可复现的验证系统：规范 microvector 告诉你算法细节是否正确，libwebp differential 告诉你生态行为是否兼容，property/metamorphic testing 告诉你不同执行方式是否一致，fuzz 和资源预算告诉你恶意输入是否安全，mutation/fault testing 告诉你这些测试是否真的能发现错误。

严格遵守以下顺序：

```text
定义可观察行为
→ 建立 oracle 和 fixture
→ 写会失败的测试
→ 实现最小正确路径
→ 差分定位
→ fuzz/突变补强
→ 性能优化
→ SIMD/FFI
→ 生产准入
```

任何跳过测试闭环、直接扩展功能或性能的提交，都会把成本转移到项目末期，并显著降低最终替换 libwebp 的可行性。
