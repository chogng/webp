# Rust 最终架构与迁移契约

状态：**最终目标，供后续迁移任务直接执行**
适用范围：`webp-rs` 下的生产 Rust 代码、单元测试、集成测试、示例、fuzz target 与 Bazel/Cargo 构建入口。
不在本次迁移范围：codec 行为、压缩策略、现有公共 API 语义、性能算法、语料内容
以及新产品能力的主动修改。迁移只搬运现有 container 读取/写入行为并确立所有权，
不在同一批变更中补齐 demux、mux、decoder-only 或 SharpYUV。四项能力在结构迁移
全部验收后分别开发和验证。

## 1. 最终决策

最终保留两个面向用户的 production library crate：`webp-container` 和 `webp`。
前者是面向公开 demux/mux 的独立容器产品边界；后者提供像素编解码和高层 API，
并单向依赖 `webp-container`。其余格式域成为 `webp` 的私有模块。迁移先建立边界并
保留现有能力，完整公开 demux/mux 和后续 editor 在迁移完成后继续开发。

```text
用户操作层：decode / encode / inspect
                         ↓
产品编排层：static_image / animated_image / incremental / inspection
               ↓                         ↓
公共容器产品：webp-container       私有 codec 所有者：vp8 / vp8l / alpha / animation
        ↓                              ↓
container 私有实现             codec 私有机制：Huffman / LZ77 / transforms / kernels
        ↓                              ↓
自有 error/options/checked math  webp 内部 bit I/O / errors / limits / checked math
```

必须遵守以下结论：

1. `webp-container` 和 `webp` 是仅有的 published library crates。
2. `webp-container` 是 demux/mux 的最终公开所有者；`webp` 依赖它，反向依赖禁止。
   本次迁移不要求补齐尚不存在的完整 mux/editor 产品能力。
3. `xtask` 保留为 workspace 工具 crate；`fuzz` 继续作为 workspace 外的
   `cargo-fuzz` package。
4. `vp8`、`vp8l`、`alpha`、`animation` 是 `webp` 的私有格式/状态所有者，
   不是独立发布单元。
5. 不建立顶层 `webp-decoder`、`webp-encoder`、`webp-dsp`、`webp-color` 或
   `webp-bitstream` crate。
6. 编码和解码是公共操作及产品编排方向，不是横切所有格式的独立所有者。
7. VP8L Huffman、LZ77、color cache、color transform、indexing 和 predictor
   全部归 `vp8l` 私有模块所有。
8. ALPH 的无头 VP8L 表示通过 `crate::vp8l` 的窄 `pub(crate)` 接口复用 wire
   grammar；不得复制另一套 Huffman/prefix 实现。
9. `webp-container` 同时拥有 RIFF 读取、写入和无损编辑，但把 VP8/VP8L/ALPH
   payload 当作 opaque bytes，不解析 codec 内部语义。
10. VP8 的 WebP YUV420 布局与 RGB/YUV 转换归 `vp8`；VP8L color transform
   归 `vp8l`；animation alpha blending 归 `animation`。三者不得塞入泛化的
   `color` 或 `dsp` 杂物模块。
11. `webp-container` 的公开 API 只承诺容器语义；`webp` 内部默认使用私有模块，
    跨模块只开放完成调用所需的最窄 `pub(crate)` 表面。

## 2. 完整最终目录

下面的树是迁移完成后的目标。它覆盖当前所有生产源码、单元测试、集成测试、
example、fuzz target 和 Rust/Bazel manifest。`fuzz/corpus/**` 与
`fuzz/dictionaries/webp.dict` 原路径保留，因数量较大不逐个展开语料文件。

```text
webp-rs/
├── BUILD.bazel
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── rustfmt.toml
│
├── container/
│   ├── BUILD.bazel
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs
│   │   ├── error.rs
│   │   ├── fourcc.rs
│   │   ├── chunk.rs
│   │   ├── layout.rs
│   │   ├── layout_tests.rs
│   │   ├── metadata.rs
│   │   ├── animation.rs
│   │   ├── animation_tests.rs
│   │   ├── demux.rs
│   │   ├── demux_tests.rs
│   │   ├── mux.rs
│   │   └── mux_tests.rs
│   └── tests/
│       ├── public_api.rs
│       └── round_trip.rs
│
├── webp/
│   ├── BUILD.bazel
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs
│   │   ├── error.rs
│   │   ├── bit_io.rs
│   │   ├── limits.rs
│   │   │
│   │   ├── api/
│   │   │   ├── mod.rs
│   │   │   ├── image.rs
│   │   │   ├── animation.rs
│   │   │   ├── metadata.rs
│   │   │   └── options.rs
│   │   │
│   │   ├── vp8/
│   │   │   ├── mod.rs
│   │   │   ├── bool_coder.rs
│   │   │   ├── bool_coder_tests.rs
│   │   │   ├── partitions.rs
│   │   │   ├── partitions_tests.rs
│   │   │   ├── intra_prediction.rs
│   │   │   ├── intra_prediction_tests.rs
│   │   │   ├── quantization.rs
│   │   │   ├── quantization_tests.rs
│   │   │   ├── transforms.rs
│   │   │   ├── transforms_tests.rs
│   │   │   ├── reconstruction.rs
│   │   │   ├── reconstruction_tests.rs
│   │   │   ├── loop_filter.rs
│   │   │   ├── loop_filter_tests.rs
│   │   │   ├── yuv_image.rs
│   │   │   ├── yuv_image_tests.rs
│   │   │   ├── frame_reader.rs
│   │   │   ├── frame_reader_tests.rs
│   │   │   ├── test_support.rs
│   │   │   ├── coefficients/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── probabilities.rs
│   │   │   │   ├── token_stream.rs
│   │   │   │   ├── residuals.rs
│   │   │   │   ├── token_stream_tests.rs
│   │   │   │   └── residuals_tests.rs
│   │   │   └── frame_writer/
│   │   │       ├── mod.rs
│   │   │       ├── source_image.rs
│   │   │       ├── macroblock_plan.rs
│   │   │       ├── partition_writer.rs
│   │   │       ├── coefficient_writer.rs
│   │   │       ├── source_image_tests.rs
│   │   │       ├── macroblock_plan_tests.rs
│   │   │       └── frame_writer_tests.rs
│   │   │
│   │   ├── vp8l/
│   │   │   ├── mod.rs
│   │   │   ├── header.rs
│   │   │   ├── header_tests.rs
│   │   │   ├── pixel.rs
│   │   │   ├── allocation.rs
│   │   │   ├── color_cache.rs
│   │   │   ├── color_cache_tests.rs
│   │   │   ├── image_reader.rs
│   │   │   ├── image_reader_tests.rs
│   │   │   ├── predictor_benchmark_tests.rs
│   │   │   ├── huffman/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── code_lengths.rs
│   │   │   │   ├── decode_table.rs
│   │   │   │   ├── symbol_reader.rs
│   │   │   │   ├── symbol_writer.rs
│   │   │   │   ├── code_lengths_tests.rs
│   │   │   │   ├── decode_table_tests.rs
│   │   │   │   ├── symbol_reader_tests.rs
│   │   │   │   └── symbol_writer_tests.rs
│   │   │   ├── backward_references/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── distance.rs
│   │   │   │   ├── copy.rs
│   │   │   │   ├── match_finder.rs
│   │   │   │   ├── tokens.rs
│   │   │   │   ├── distance_tests.rs
│   │   │   │   ├── copy_tests.rs
│   │   │   │   └── match_finder_tests.rs
│   │   │   ├── transforms/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── color.rs
│   │   │   │   ├── indexing.rs
│   │   │   │   ├── predictor.rs
│   │   │   │   ├── subtract_green.rs
│   │   │   │   ├── color_tests.rs
│   │   │   │   ├── indexing_tests.rs
│   │   │   │   ├── predictor_tests.rs
│   │   │   │   └── subtract_green_tests.rs
│   │   │   ├── image_stream/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── decode_profile.rs
│   │   │   │   ├── huffman_groups.rs
│   │   │   │   ├── pixel_buffer.rs
│   │   │   │   ├── pixel_sink.rs
│   │   │   │   ├── symbol_stream.rs
│   │   │   │   ├── huffman_groups_tests.rs
│   │   │   │   ├── pixel_sink_tests.rs
│   │   │   │   └── symbol_stream_tests.rs
│   │   │   └── image_writer/
│   │   │       ├── mod.rs
│   │   │       ├── palette_plan.rs
│   │   │       ├── transform_plan.rs
│   │   │       ├── cache_plan.rs
│   │   │       ├── tokenization.rs
│   │   │       ├── palette_plan_tests.rs
│   │   │       ├── transform_plan_tests.rs
│   │   │       ├── cache_plan_tests.rs
│   │   │       ├── tokenization_tests.rs
│   │   │       └── image_writer_tests.rs
│   │   │
│   │   ├── alpha/
│   │   │   ├── mod.rs
│   │   │   ├── header.rs
│   │   │   ├── header_tests.rs
│   │   │   ├── filters.rs
│   │   │   ├── filters_tests.rs
│   │   │   ├── level_reduction.rs
│   │   │   ├── level_reduction_tests.rs
│   │   │   ├── backward_references.rs
│   │   │   ├── backward_references_tests.rs
│   │   │   ├── palette_plan.rs
│   │   │   ├── palette_plan_tests.rs
│   │   │   ├── symbol_plan.rs
│   │   │   ├── symbol_plan_tests.rs
│   │   │   ├── plane_reader.rs
│   │   │   ├── plane_reader_tests.rs
│   │   │   ├── plane_writer.rs
│   │   │   └── plane_writer_tests.rs
│   │   │
│   │   ├── animation/
│   │   │   ├── mod.rs
│   │   │   ├── canvas.rs
│   │   │   └── canvas_tests.rs
│   │   │
│   │   ├── static_image.rs
│   │   ├── static_image_tests.rs
│   │   ├── animated_image.rs
│   │   ├── animated_image_tests.rs
│   │   ├── incremental.rs
│   │   ├── incremental_tests.rs
│   │   ├── inspection.rs
│   │   ├── inspection_tests.rs
│   │   │
│   │   └── fuzzing/
│   │       ├── mod.rs
│   │       ├── vp8.rs
│   │       └── vp8l.rs
│   │
│   ├── examples/
│   │   ├── alpha_encode_bench.rs
│   │   ├── animation_encode_bench.rs
│   │   ├── decode_bench.rs
│   │   ├── encode_bench.rs
│   │   ├── vp8_encode_bench.rs
│   │   └── vp8l_color_transform_reproducer.rs
│   │
│   └── tests/
│       ├── public_api.rs
│       ├── metadata_encode.rs
│       ├── animation_encode.rs
│       ├── alpha_encoder_oracle.rs
│       ├── animation_encode_oracle.rs
│       ├── vp8_encoder_oracle.rs
│       ├── vp8_libwebp_oracle.rs
│       ├── vp8l_encoder_oracle.rs
│       ├── external_animation_corpus.rs
│       └── external_upstream_corpus.rs
│
├── fuzz/
│   ├── Cargo.toml
│   ├── Cargo.lock
│   ├── README.md
│   ├── dictionaries/
│   │   └── webp.dict
│   ├── corpus/
│   │   ├── animation_raw/**
│   │   ├── container_raw/**
│   │   ├── incremental_raw/**
│   │   ├── vp8_bool_raw/**
│   │   ├── vp8_coefficients/**
│   │   ├── vp8_partition_raw/**
│   │   ├── vp8_residuals/**
│   │   ├── vp8_transforms/**
│   │   ├── vp8l_header_raw/**
│   │   ├── vp8l_huffman/**
│   │   ├── vp8l_raw/**
│   │   └── vp8l_transforms/**
│   └── fuzz_targets/
│       ├── animation_raw.rs
│       ├── container_raw.rs
│       ├── incremental_raw.rs
│       ├── vp8_bool_raw.rs
│       ├── vp8_coefficients.rs
│       ├── vp8_partition_raw.rs
│       ├── vp8_residuals.rs
│       ├── vp8_transforms.rs
│       ├── vp8l_header_raw.rs
│       ├── vp8l_huffman.rs
│       ├── vp8l_raw.rs
│       └── vp8l_transforms.rs
│
└── xtask/
    ├── BUILD.bazel
    ├── Cargo.toml
    └── src/
        └── main.rs

docs/
└── performance/
    ├── alpha.md
    └── vp8l.md
```

两个 `lib.rs` 都只允许包含 crate 文档、私有 `mod` 声明和显式公共 re-export。
不得把实现重新堆回任一 `lib.rs`。

## 3. 模块所有权

### 3.1 `api`

只拥有稳定公共数据模型，不执行 codec 工作：

- `image.rs`：`Image`、`ImageInfo`、`Progress`。
- `animation.rs`：`Animation`、`AnimationFrame`、`AnimationEncodeFrame`、
  `AnimationEncodeOptions`。
- `metadata.rs`：公共 `Metadata`。
- `options.rs`：`DecodeOptions`、`LossyEncodeOptions` 和其他稳定选项。
- `error.rs` 位于 crate 根，统一拥有 `DecodeError`、`DecodeErrorKind`、
  `EncodeError` 及其转换。

### 3.2 产品编排

- `static_image.rs`：静态 WebP 的 container/codec/alpha 组合，包含读和写两条方向。
- `animated_image.rs`：动画帧 payload、container animation chunk 与 canvas 的组合。
- `incremental.rs`：增量输入状态机；未真正支持的能力不得伪装成完整 decoder。
- `inspection.rs`：只读尺寸与 metadata，不 materialize 像素。

这些模块可以调用格式所有者，但不能拥有 VP8/VP8L 算法。

### 3.3 `webp-container`

`webp-container` 是独立发布的公共容器产品边界。package 名为 `webp-container`，
Rust import 名为 `webp_container`。demux、mux 以及未来的无损 editor 共享同一套
RIFF/chunk/layout 所有权，不拆成多个 crate。

- `fourcc.rs`：公开 FourCC 类型和已知 chunk 常量。
- `chunk.rs`：公开借用/owned chunk、padding 与 checked size 模型。
- `layout.rs`：VP8X flags、chunk 顺序、互斥、重复与 strict/compatible 规则。
- `metadata.rs`：ICCP/EXIF/XMP 的借用和 owned 表示。
- `animation.rs`：ANIM/ANMF wire geometry、flags、frame rectangle 和 opaque payload。
- `demux.rs`：承接现有零拷贝 parse、frame/chunk iteration 与查询行为；完整公开
  `Demuxer` 产品 API 在迁移后补齐。
- `mux.rs`：迁入当前 encoder 已有的 RIFF/chunk/animation serialization；迁移期只
  提供 `webp` 完成现有编码所需的最小跨 crate 表面，完整公开 `Muxer` 在迁移后补齐。
- `error.rs`：独立 `ContainerError`、offset/context 和 container limits/options。

迁移后的独立产品阶段再形成以下目标 API；它们是后续方向，不是本次迁移的验收项：

```rust,ignore
let file = webp_container::Demuxer::parse(bytes, options)?;
let encoded = webp_container::Muxer::new()
    .canvas(width, height)
    .vp8l(payload)
    .metadata(metadata)
    .finish()?;
let edited = webp_container::Editor::parse(bytes, options)?
    .set_exif(exif)
    .remove_xmp()
    .finish()?;
```

`webp-container` 只能把 VP8/VP8L/ALPH payload 当作 opaque bytes。对于没有 VP8X
且需要从 codec header 推导尺寸的 simple file，codec-aware `webp` 负责提供尺寸或
执行额外检查；不得让 `webp-container` 反向依赖 `webp`。

### 3.4 `vp8`

`vp8` 拥有 VP8 key-frame wire format、bool coder、partition、概率、macroblock、
YUV420 storage、reconstruction、loop filter 和 frame writer。RGBA/YUV 转换依赖
WebP 的 VP8 色彩约定，因此保留在 `vp8::yuv_image`，不抽成顶层 `color`。

### 3.5 `vp8l`

`vp8l` 是完整 VP8L codec 所有者：header、嵌套 image stream、Huffman、
backward references、color cache、四种 transform、像素表示、完整 image reader
和 image writer。

`vp8l::pixel` 必须消除当前 `vp8l-transform` 与 `vp8l-color-transform` 各自定义
`Rgba`/`RgbaImage` 的重复。codec 内只保留一种明确的内部像素表示；公共输出仍为
straight RGBA8 bytes。

### 3.6 `alpha`

`alpha` 拥有 ALPH header、filter、preprocessing、平面编解码和选项。其 lossless
payload 是 headerless VP8L 子格式，因此通过窄内部接口使用 `vp8l` 的 prefix、
Huffman writer/reader 与边界规则。ALPH 专属 filter、level reduction、scalar-plane
match search 仍由 `alpha` 所有。

### 3.7 `animation`

只拥有 canvas allocation、blend、dispose-to-background 和 frame application
顺序。ANIM/ANMF wire parsing 属于 `webp-container`，codec payload 属于 VP8/VP8L，
最终编排属于 `animated_image`。

### 3.8 `fuzzing`

`webp` 增加非默认、非稳定的 `fuzzing` feature：

```toml
[features]
default = []
fuzzing = []
```

`src/fuzzing` 仅在 `cfg(feature = "fuzzing")` 下编译，并以
`#[doc(hidden)] pub` 暴露 codec 内部的最窄字节入口。container fuzz target 直接
使用 `webp-container` 的公共 API。`webp-fuzz` 最终只能依赖两个 production crates：

```toml
webp = { path = "../webp", features = ["fuzzing"] }
webp-container = { path = "../container" }
```

禁止为了 fuzz 继续发布算法 crate 或把内部结构加入正常公共 API。

## 4. 现有文件迁移映射

### 4.1 基础设施、容器与动画

| 现有位置 | 最终位置 |
| --- | --- |
| `core/src/bit_io.rs` | `webp/src/bit_io.rs` |
| `core/src/error.rs` | codec 错误进 `webp/src/error.rs`；container 错误语义进 `container/src/error.rs` |
| `core/src/limits.rs` | codec/work budget 进 `webp/src/limits.rs`；container size/layout 限制进 `container` options |
| `core/src/lib.rs` | 删除；exports 由 `webp/src/lib.rs` 明确给出 |
| `container/src/container.rs` | 按职责拆入 `container/src/{fourcc,chunk,layout,metadata,animation,demux}.rs` |
| `container/src/container_tests.rs` | `container/src/{layout,demux}_tests.rs` 和 public integration tests |
| `container/src/animation_tests.rs` | `container/src/animation_tests.rs` |
| `container/src/lib.rs` | 保持 crate root，只含文档、私有声明和公开 re-export |
| `animation/src/compositor.rs` | `animation/canvas.rs` |
| `animation/src/compositor_tests.rs` | `animation/canvas_tests.rs` |
| `animation/src/lib.rs` | `animation/mod.rs` |

当前 `webp/src/encoder.rs` 中的 `wrap_*`、`push_chunk`、`chunk_storage_len`、
metadata 和 ANIM/ANMF serialization 必须迁入 `webp-container` 的 `mux.rs`、
`chunk.rs` 或 `animation.rs`，不能留在 codec 产品门面。这里只搬迁已有逻辑，不补齐
通用 mux/editor，也不得改变现有 encoder 产生的标准字节流。

### 4.2 VP8

| 现有位置 | 最终位置 |
| --- | --- |
| `vp8/src/bitstream.rs` | `vp8/bool_coder.rs` |
| `vp8/src/bitstream_tests.rs` | `vp8/bool_coder_tests.rs` |
| `vp8/src/partition.rs` | `vp8/partitions.rs` |
| `vp8/src/partition_tests.rs` | `vp8/partitions_tests.rs` |
| `vp8/src/entropy.rs` | `vp8/coefficients/{probabilities,token_stream}.rs` |
| `vp8/src/entropy_tests.rs` | `vp8/coefficients/token_stream_tests.rs` |
| `vp8/src/coefficients.rs` | `vp8/coefficients/{mod,residuals}.rs` |
| `vp8/src/intra.rs` | `vp8/intra_prediction.rs` |
| `vp8/src/intra_tests.rs` | `vp8/intra_prediction_tests.rs` |
| `vp8/src/quantization.rs` | `vp8/quantization.rs` |
| `vp8/src/quantization_tests.rs` | `vp8/quantization_tests.rs` |
| `vp8/src/transform.rs` | `vp8/transforms.rs` |
| `vp8/src/transform_tests.rs` | `vp8/transforms_tests.rs` |
| `vp8/src/reconstruction.rs` | `vp8/reconstruction.rs` 和 `vp8/coefficients/residuals.rs` |
| `vp8/src/reconstruction_tests.rs` | 对应 sibling test files |
| `vp8/src/loop_filter.rs` | `vp8/loop_filter.rs` |
| `vp8/src/loop_filter_tests.rs` | `vp8/loop_filter_tests.rs` |
| `vp8/src/frame.rs` | `vp8/{yuv_image,frame_reader}.rs` |
| `vp8/src/frame_tests.rs` | `vp8/{yuv_image_tests,frame_reader_tests}.rs` |
| `vp8/src/encoder.rs` | `vp8/frame_writer/**`；RGBA/YUV layout 移入 `vp8/yuv_image.rs` |
| `vp8/src/encoder_tests.rs` | `vp8/frame_writer/*_tests.rs` |
| `vp8/src/test_support.rs` | `vp8/test_support.rs`，保持 `cfg(test)` |
| `vp8/src/lib.rs` | `vp8/mod.rs`，只做私有声明和窄内部 re-export |

拆 `vp8/src/encoder.rs` 时按 source YUV ownership、macroblock decision、partition
serialization 和 coefficient serialization 拆分，不得只按行数机械切割。

### 4.3 VP8L

| 现有位置 | 最终位置 |
| --- | --- |
| `vp8l/src/lib.rs` | `vp8l/header.rs` 与 `vp8l/transforms/mod.rs` |
| `vp8l-huffman/src/lib.rs` | `vp8l/huffman/**` |
| `vp8l-entropy/src/lib.rs` | `vp8l/backward_references/{distance,copy}.rs` 及 `image_stream/symbol_stream.rs` |
| `vp8l-color-cache/src/lib.rs` | `vp8l/color_cache.rs` |
| `vp8l-color-transform/src/lib.rs` | `vp8l/transforms/color.rs`；删除重复像素类型 |
| `vp8l-indexing/src/lib.rs` | `vp8l/transforms/indexing.rs` |
| `vp8l-transform/src/lib.rs` | `vp8l/transforms/{predictor,subtract_green}.rs`；删除重复像素类型 |
| `vp8l-literal/src/allocation.rs` | `vp8l/allocation.rs` |
| `vp8l-literal/src/decode_profile.rs` | `vp8l/image_stream/decode_profile.rs` |
| `vp8l-literal/src/huffman_group.rs` | `vp8l/image_stream/huffman_groups.rs` |
| `vp8l-literal/src/image.rs` | `vp8l/image_reader.rs` |
| `vp8l-literal/src/image_data.rs` | `vp8l/image_stream/{symbol_stream,pixel_sink}.rs` |
| `vp8l-literal/src/inverse_color.rs` | `vp8l/transforms/color.rs` 的 pipeline integration |
| `vp8l-literal/src/inverse_indexing.rs` | `vp8l/transforms/indexing.rs` 的 pipeline integration |
| `vp8l-literal/src/inverse_predictor.rs` | `vp8l/transforms/predictor.rs` |
| `vp8l-literal/src/pixel.rs` | `vp8l/pixel.rs` |
| `vp8l-literal/src/pixel_buffer.rs` | `vp8l/image_stream/pixel_buffer.rs` |
| `vp8l-literal/src/pixel_output.rs` | `vp8l/image_stream/pixel_sink.rs` |
| `vp8l-literal/src/transform_list.rs` | `vp8l/transforms/mod.rs` 与 `vp8l/image_reader.rs` |
| `vp8l-literal/src/*_tests.rs` | 对应目标模块的 sibling `*_tests.rs` |
| `vp8l-literal/src/predictor_benchmark_tests.rs` | `vp8l/predictor_benchmark_tests.rs` |
| `vp8l-literal/src/lib.rs` | 删除；完整入口由 `vp8l/mod.rs` 所有 |

当前 `webp/src/encoder.rs` 中以下内容必须迁入 `vp8l/image_writer/**`：

- `encode_vp8l_payload` 与 VP8L header 写入；
- palette plan 与 palette subimage；
- predictor/color-transform plan 与 transform subimage；
- color-cache selection；
- literal/cache/backward-reference tokenization；
- VP8L Huffman frequency、code-length 与 canonical symbol 写入；
- VP8L length/distance prefix 写入。

迁移后 `static_image.rs` 只能调用类似
`vp8l::encode_rgba_payload(...)` 的完整内部入口，不得了解 VP8L symbol alphabet。

### 4.4 ALPH

| 现有位置 | 最终位置 |
| --- | --- |
| `alpha/src/alpha.rs` | `alpha/{header,filters,plane_reader}.rs` |
| `alpha/src/alpha_tests.rs` | 对应 sibling tests |
| `alpha/src/encode.rs` | `alpha/plane_writer.rs` 与 `alpha/symbol_plan.rs` |
| `alpha/src/encode_tests.rs` | `alpha/plane_writer_tests.rs` |
| `alpha/src/encode_filter.rs` | `alpha/filters.rs` |
| `alpha/src/encode_filter_tests.rs` | `alpha/filters_tests.rs` |
| `alpha/src/encode_huffman.rs` | 通用 wire 逻辑合入 `vp8l/huffman/symbol_writer.rs`；ALPH frequency plan 留在 `alpha/symbol_plan.rs` |
| `alpha/src/encode_huffman_tests.rs` | 按被测所有者分别迁移 |
| `alpha/src/encode_lz77.rs` | `alpha/backward_references.rs`；通用 prefix 规则调用 `vp8l` |
| `alpha/src/encode_lz77_tests.rs` | `alpha/backward_references_tests.rs` |
| `alpha/src/encode_palette.rs` | `alpha/palette_plan.rs` |
| `alpha/src/encode_palette_tests.rs` | `alpha/palette_plan_tests.rs` |
| `alpha/src/level_reduction.rs` | `alpha/level_reduction.rs` |
| `alpha/src/level_reduction_tests.rs` | `alpha/level_reduction_tests.rs` |
| `alpha/src/lib.rs` | `alpha/mod.rs` |

### 4.5 公共门面与编排

| 现有位置 | 最终位置 |
| --- | --- |
| `webp/src/api.rs` | `api/**` 与 crate 根 `error.rs` |
| `webp/src/decoder.rs` | 静态路径进 `static_image.rs`；动画路径进 `animated_image.rs` |
| `webp/src/decoder_tests.rs` | `static_image_tests.rs` 与 `animated_image_tests.rs` |
| `webp/src/encoder.rs` | VP8L 部分进 `vp8l/image_writer/**`；RIFF 部分进 `webp-container`；产品组合进 `static_image.rs`/`animated_image.rs` |
| `webp/src/encoder_tests.rs` | codec 私有测试下沉；跨域 round trip 留在 orchestration sibling tests |
| `webp/src/incremental.rs` | `incremental.rs` |
| `webp/src/info.rs` | `inspection.rs` |
| `webp/src/lib.rs` | 保持路径，只收敛为文档、声明与公共 re-export |
| `webp/examples/**` | 原文件名和路径保留 |
| `webp/tests/**` | 原文件名和路径保留 |

### 4.6 README、Cargo 与 Bazel

| 现有位置 | 最终位置 |
| --- | --- |
| `alpha/README.md` | `docs/performance/alpha.md` |
| `vp8l/README.md` | `docs/performance/vp8l.md` |
| 除 `container`、`webp`、`xtask` 外各旧 crate `Cargo.toml` | 删除 |
| 除 `container`、`webp`、`xtask` 外各旧 crate `BUILD.bazel` | 删除 |
| `webp-rs/Cargo.toml` | members 最终为 `container`、`webp`、`xtask`，继续 exclude `fuzz` |
| `container/Cargo.toml` | 保留 package `webp-container`，不依赖 `webp` 或 codec crate |
| `webp/Cargo.toml` | 只保留本仓库 path dependency `webp-container`；添加非默认 `fuzzing` feature |
| `fuzz/Cargo.toml` | 只依赖 public `webp-container` 和 feature-enabled `webp` |
| `container/BUILD.bazel` | 独立 public production `rust_library` 和 container tests |
| `webp/BUILD.bazel` | production `rust_library` 依赖 `//webp-rs/container`，加现有 tests/examples targets |
| `webp-rs/BUILD.bazel` | 保留 workspace filegroup，移除旧 crate 假设 |

所有指向旧 README、crate label、Cargo package 和 Bazel target 的脚本与文档链接必须
同步更新。语料、性能原始数据和历史 commit/branch 记录不得重写。

## 5. 依赖与可见性规则

允许的方向：

```text
webp public API
  └── static_image / animated_image / incremental / inspection
        ├──→ webp-container public API
        ├──→ vp8
        ├──→ vp8l
        ├──→ alpha ──→ vp8l 的窄 wire helper
        └──→ animation

vp8 ───────┐
vp8l ──────┼──→ webp crate 的 bit_io / limits / error
alpha ─────┤
animation ─┘

webp-container
  └──→ 自有 error / options / checked RIFF arithmetic

禁止：webp-container ──→ webp 或任意 codec module
```

禁止：

- `vp8` 与 `vp8l` 相互依赖；
- `webp-container` 依赖 `webp` 或 codec 内部类型；
- `webp` 绕过 `webp-container` 私自写 RIFF/chunk；
- `animation` 解析 RIFF 或解码 codec payload；
- codec 调用 `static_image`、`animated_image` 或公共 API；
- 为方便测试把内部结构改成正常 `pub`；
- 建立名为 `decoder.rs`、`encoder.rs`、`entropy.rs`、`utils.rs`、`dsp.rs` 的
  泛化顶层所有者；
- 同一 wire 常量或算法在 VP8L encoder、decoder 和 ALPH 中保留多份定义。

如果两个模块需要彼此实现细节，必须重新审查边界；不得用扩大可见性解决循环所有权。

## 6. 迁移阶段

迁移必须分阶段，每阶段保持可编译、测试通过并可独立审查。不要做一个同时移动全部
文件、改变算法和重写测试的巨型提交。

### 阶段 0：基线与保护

1. 从执行时最新 `main` 建立 `codex/final-rust-architecture` 工作树。
2. 记录 base SHA、workspace test、release test、Clippy、fmt、Bazel 和所有质量门禁。
3. 记录 VP8L/VP8/ALPH benchmark 基线；迁移提交不得顺手优化算法。
4. 确认公共 API 测试覆盖现有所有 re-export 和入口。

### 阶段 1：先在现有 workspace 内确立所有者

1. 将 VP8L encoder payload 逻辑从 `webp::encoder` 下沉到 VP8L 所有者。
2. 将 RIFF/metadata/animation chunk 写入下沉到 `webp-container` 所有者。
3. 为 `webp-container` 建立独立错误和 limits/options；保留现有 parse 行为，并只提供
   当前 `webp` encoder 所需的最小 serialization 表面。不要在本阶段设计完整
   demux/mux/editor 产品 API。
4. 保持其他现有 crate 暂时存在，以小提交消除层次反转。

### 阶段 2：合并 VP8L 微 crate

按依赖方向依次合并：

1. pixel 与 transforms；
2. Huffman；
3. backward references；
4. color cache；
5. image stream；
6. 完整 image reader/writer；
7. 删除 `vp8l-literal` 名称和旧微 crate。

每一步都运行 VP8L unit、fuzz smoke、oracle、CLIC decode 和 encode gate。

### 阶段 3：整理 VP8、ALPH、webp-container、animation 内部模块

按最终树拆分大文件并移动 sibling tests。只按状态和不变量拆分，不因文件超过某个
行数就机械切割。ALPH 在本阶段消除与 VP8L 重复的 Huffman/prefix wire 逻辑。
`webp-container` 必须在本阶段完成现有 parse 与 encoder serialization 的回归测试、
现有 WebP round trip 和 no-codec-dependency 测试；不新增 editor 或通用 chunk
mutation 行为。

### 阶段 4：将 codec owners 并入 `webp` crate

建议顺序：

1. `animation`；
2. `alpha`；
3. `vp8`；
4. 已收敛的 `vp8l`；
5. `core` 最后拆分：codec 部分并入 `webp`，container 部分并入
   `webp-container` 的 error/options/checked arithmetic。

`webp-container` 始终保留为独立 public crate，不参与本阶段合并。每并入一个其他
owner，就删除对应 path dependency 和 Bazel library target；不要长期保留两套 source
copy。

### 阶段 5：fuzz、构建和文档收口

1. 建立 feature-gated `fuzzing` wrapper。
2. 令 `webp-fuzz` 只依赖 `webp-container` 和 `webp` 两个 production crates。
3. 收敛 Bazel 为 container/webp 两个 public library targets，更新所有脚本 label。
4. 移动两个性能 README 并修复链接。
5. 删除旧 crate 目录和 manifest。
6. 检查仓库中不再出现旧 package 名与旧 Bazel label。

## 7. 每阶段验证

普通验证使用 workspace stable Rust：

```sh
cd webp-rs
cargo fmt --all -- --check
cargo test --workspace
cargo test --release --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
cd ..
bazel test --test_output=errors --test_tag_filters=-external-corpus //...
```

涉及对应 codec 的阶段还必须运行 `docs/quality-gates.md` 中当前有效的：

- VP8L conformance decode、CLIC decode 和 static encode；
- VP8 static encode；
- VP8/ALPH static encode；
- animation、metadata 与 pinned-libwebp oracle；
- fuzz target build 以及每个受影响 target 的短 smoke run。

架构迁移不允许用性能退化换目录整洁。若出现显著回归，先定位 cross-module
inlining、数据布局或重复转换，再决定边界；不得无记录地放宽 gate。

## 8. 最终验收条件

只有同时满足以下条件，迁移才算完成：

1. `cargo metadata` 中 production library packages 只有 `webp-container` 和 `webp`，
   另有工具 `xtask`。
2. `webp-rs` 下保留 `container`、`webp`、`xtask`；不再存在 `core`、`animation`、
   `alpha`、`vp8`、`vp8l` 或任何 `vp8l-*` crate 目录。codec owners 位于
   `webp/src`，container 保持独立 crate。
3. `webp/Cargo.toml` 的本仓库 production path dependency 只有 `webp-container`；
   `webp-container/Cargo.toml` 不依赖 `webp` 或任何 codec crate。
4. `webp-container` 保留并测试迁移前已有的 container parse 行为，接管 encoder
   已有的 RIFF/chunk/animation serialization；本项不要求新增 editor 或完整 mux。
5. `fuzz/Cargo.toml` 除 `libfuzzer-sys` 外只依赖 public `webp-container` 和
   feature-enabled `webp`。
6. 正常文档不暴露 fuzz/internal module；现有 `webp` 与 `webp-container` 公共行为
   保持兼容，迁移所需的最小新增跨 crate 表面有清楚的 provisional 文档和错误语义。
7. 两个 crate 的 `lib.rs` 都只包含文档、module declarations、re-exports 和极薄
   入口委托。
8. 不存在泛化顶层 `decoder`、`encoder`、`entropy`、`utils`、`dsp` 或 `color`
   模块承担跨格式所有权。
9. VP8L/ALPH 不再复制 Huffman canonical code、length/distance prefix 或 color-cache
   wire 常量。
10. 所有新增 test module 使用 sibling `*_tests.rs` 和显式 `#[path = ...]`。
11. workspace debug/release tests、Clippy、fmt、docs、Bazel、oracle、fuzz smoke 和性能
    gate 全部通过。
12. 所有已删除 Cargo package 名、Bazel label 和 README 路径引用都已更新。
13. 迁移提交不包含无关算法改进、语料变化或用户未提交文件。

## 9. 迁移完成后的产品路线

以下四项明确排除在架构迁移之外。只有第 8 节全部通过并形成稳定基线后，才分别建立
独立任务、测试与性能记录；不得重新混入迁移分支。

1. **完整 demux API — 已完成**：`Demuxer` 提供稳定零拷贝 parse、chunk/frame
   iteration、随机查询与资源策略；兼容的 `Container`/`parse` 入口继续保留。
2. **完整 mux/editor API — 已完成**：`Muxer` 提供静态图像、动画帧和通用 owned chunk
   构造；`Editor` 提供 metadata/frame/chunk mutation、未知 chunk 保留以及无需像素
   重编码的 strict edit round trip。
3. **decoder-only 产品档 — 已完成**：`webp` 提供 additive
   `decode`/`encode`/`animation` Cargo features，默认仍是完整兼容产品档；decoder-only
   已验证不编译 encoder orchestration，并记录了 archive 大小、编译时间和依赖差异，详见
   [`decoder-only.md`](decoder-only.md)。
4. **SharpYUV 等价能力 — 已完成**：VP8 唯一生产转换路径使用私有 scalar SharpYUV，
   明确限定为 straight RGBA8、sRGB transfer、WebP limited-range matrix 和四次重建感知
   refinement。旧 2×2 box sampler 已删除，不作为兼容档、回退或隐藏配置保留；逐字节
   pinned libsharpyuv oracle、公开 VP8/dwebp oracle、客观质量记录和性能门禁均已建立，
   详见 [`sharpyuv.md`](sharpyuv.md)。

四项迁移后产品能力现已全部完成。demux/mux 继续共同位于 `webp-container`，
decoder-only 使用 `webp` features，SharpYUV 因只有 VP8 encoder 一个真实用户且没有
独立依赖、版本或发布周期，保留为 `webp::vp8` 私有模块，不建立空 crate。

## 10. 交给新对话的执行指令

新对话应从本文件开始，不重新发明目标目录。建议直接使用以下任务描述：

> 按 `docs/final-rust-architecture.md` 执行 Rust 最终架构迁移。先读取根
> `AGENTS.md`、本文档和 `docs/quality-gates.md`，检查工作树与最新 main，建立
> 独立 `codex/final-rust-architecture` 工作树并记录基线。严格按阶段提交，每阶段
> 保持编译、测试和相关性能门禁通过；不改变公共行为，不夹带算法优化，不覆盖用户
> 修改。持续执行直到本文档第 8 节全部验收条件满足。不要在迁移任务中实现第 9 节
> 的完整 demux、mux/editor、decoder-only features 或 SharpYUV；这些在迁移完成后
> 分别开独立任务。
