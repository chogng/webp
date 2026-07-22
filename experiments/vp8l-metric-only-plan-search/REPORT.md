# P22 metric-only spatial-plan search recovery: rejected

## Identity and decision

- Task: `P22 independent VP8L metric-only search / final-plan materialization
  recovery experiment`
- Root task: `019f8321-035e-7211-8f53-987e18891c8c`
- Branch: `codex/vp8l-metric-only-plan-search`
- Base: `4280a59a1a7a22d1e312b9de131b46873688c008`
- Worktree: `/Users/lance/.codex/worktrees/c5fc/webp`
- Design: `479a5149`
- Authorized P20 production transplant: `5d44c41d` from source
  `67bd04274a60fe81fcd13cc2702d75e3fd0553a4`
- A-baseline evidence: `37f1f563`
- Metric-only mechanism: `60719703`
- Locked runner and measurement HEAD: `688452ec`
- Locked Phase R binary SHA-256:
  `c271208f7a296588a4c7a892c22877e44a4c5c057a5f990857ebb73bb631b318`
- Rebuilt P18 binary SHA-256:
  `2e5f7b11b959de3cb25a251649ee7ffa87528b0346d92fdfbb03547da5f5e570`
- Corpus manifest SHA-256:
  `9feb09f469753c43864011aa6f00cfc5ee1bd48da5aac8f9b16d105890e14f86`
- Screen manifest SHA-256:
  `474587feabe3178268b2eab6f7a166501d8ecc3d637a76bd412f4233dfa7b913`

Decision: **reject metric-only search/final materialization.** Ownership and
exactness changed exactly as designed, but the single valid recovery screen
failed both LowLatency timing gates. B was 0.080641% slower than A and had
31/41 per-image median regressions; the requirements were at least 3.0%
improvement and 0/41 regressions. The recovery stop prohibited B release
routing, product Phase A, product screen, formal, and final product gates.

## Pre-B retained-full-plan control

Before B existed, the selectively transplanted P20 A control ran over all 102
images. Compact totaled 599,398,064 bytes and LowLatency 601,400,998 bytes.
Size plus stream hash matched the rebuilt P18 oracle for 204/204 profile
streams and stderr was empty. The A-baseline binary SHA-256 was
`6847299b3c09c3709ff8ec46680a7eccb3fbbcfbbe2994455c31b9746d6c20df`.
The first complete encode was invalidated only by a summarizer column error;
its output is preserved and the full baseline was restarted at `2a48807b`.

## Mechanism and exactness

B built every exact `SpatialCostPlan`, copied its four-field metric, moved out
`PlanParts`, and immediately dropped prefix/table storage. The consuming
conversion cloned no `PlanParts`. After the identical spatial/single choice, B
built one final spatial plan from the selected parts. A retained P20 full-plan
ownership and remained the release route. The A/B switch, census, and lifetime
tracker were all `cfg(test)` and disabled during timing.

Unit/property coverage passed for consumed metric payload bits/bytes/RIFF/group
count, deterministic final rebuild bytes, allocation/overflow error category
passthrough, spatial-kind and tie order, strict single-tie behavior, forced
single-plan fallback, varied inputs, both profiles, and the lifetime high-water
change. The locked Phase R run then proved:

- A/B, public/A, and P18 identities: 204/204 each;
- E/B selector, final selector, and strict fallback: 204/204 each;
- Compact spatial planner/writer: 306/306; single: 102/102; growth 0/0;
- LowLatency spatial planner/writer: 408/408; single: 102/102; growth 336/336;
- exact aggregates 599,398,064 and 601,400,998 bytes;
- all Phase R, census, P18, and recovery stderr: zero bytes.

## Census and lifetime

Each tuple below is `full plans / final materializations / PlanParts clones /
prefix bytes / table entries / estimated heap bytes`. Heap estimates include
logical prefix, tables, assignment capacity, and frequency capacity, but not
Vec headers or allocator overhead.

| profile/variant | E | B | R | Growth | Final materialization |
| --- | --- | --- | --- | --- | --- |
| Compact A | 102/0/0/1,422,551/2,322/11,989,822 | 102/0/0/1,309,077/2,128/10,994,812 | 102/0/0/1,390,751/2,267/11,708,102 | 0/0/0/0/0/0 | 0/0/0/0/0/0 |
| Compact B | same | same | same | 0/0/0/0/0/0 | 102/102/0/1,390,751/2,267/11,708,102 |
| LowLatency A | 102/0/0/780,112/1,231/6,377,992 | 102/0/0/754,413/1,187/6,152,357 | 102/0/0/798,249/1,262/6,536,993 | 336/0/0/2,805,461/4,459/23,080,289 | 0/0/0/0/0/0 |
| LowLatency B | same | same | same | same | 102/102/0/994,486/1,598/8,260,014 |

| profile/variant | max live plans | max live tables | max live prefix | max live estimated heap |
| --- | ---: | ---: | ---: | ---: |
| Compact A | 3 | 141 | 84,372 B | 725,604 B |
| Compact B | 1 | 49 | 29,292 B | 252,124 B |
| LowLatency A | 3 | 48 | 29,844 B | 248,100 B |
| LowLatency B | 1 | 16 | 9,948 B | 82,700 B |

The mechanism therefore achieved the intended ownership reduction: B never
had more than one complete plan live, versus A's peak of three, and it had zero
`PlanParts` clones. B paid one final materialization on every image/profile
because spatial won all 204 final selections.

## Single valid recovery screen

The fixed binary preloaded the first 41 images, ran one unscored warmup round,
then retained all three F/R/F rounds. It was not rerun. All 82 A/B output pairs
were byte-identical and stderr was empty.

| gate | Compact | LowLatency |
| --- | ---: | ---: |
| A samples (s) | 2.493447 / 2.493782 / 2.479023 | 2.654492 / 2.561734 / 2.576800 |
| B samples (s) | 2.518911 / 2.516327 / 2.511623 | 2.611756 / 2.578878 / 2.574910 |
| independent B vs A | **+0.917607% slower** | **+0.080641% slower** |
| required | no worse than +1% | at least 3% faster |
| per-image median regressions | 34/41 (reported) | **31/41; required 0/41** |
| gate | pass | **fail** |

Compact regressions were: `000 +1.211896%`, `001 +1.393773%`,
`002 +0.707834%`, `003 +2.098734%`, `004 +2.026427%`, `005 +0.594461%`,
`006 +1.192191%`, `007 +2.224817%`, `008 +1.830643%`, `009 +0.014570%`,
`010 +1.401247%`, `011 +1.602482%`, `012 +0.826638%`, `013 +1.176859%`,
`014 +2.477456%`, `015 +1.189981%`, `016 +1.810246%`, `017 +0.645715%`,
`018 +3.069214%`, `019 +2.534067%`, `020 +2.073609%`, `022 +0.376022%`,
`023 +1.740028%`, `024 +1.861933%`, `025 +1.548811%`, `026 +0.796783%`,
`027 +0.916969%`, `031 +1.008988%`, `033 +1.471867%`, `034 +0.064719%`,
`035 +1.281681%`, `037 +0.534397%`, `038 +2.159819%`, and
`039 +0.358056%`.

LowLatency's 31 regression IDs are retained in `recovery-summary.json`. The
result rejects the causal hypothesis: substantially shorter full-plan
lifetimes do not recover P20's missing LowLatency screen margin. The extra
selected-plan rebuild outweighed or neutralized the removed retention cost on
this end-to-end workload.

## Stop, quality, and external evidence

Release remains the exact A route. Static inspection confirms the variant
dispatch, metric-only orchestration, census, and lifetime tracking are
test-only; no dependency, feature, public API, unsafe block, thread, runtime
classifier, `Default`, metadata, animation, or limit change was introduced.
The stopped branch passed `cargo test -p webp --lib` (299 passed, 4 ignored),
all-target Clippy with `-D warnings`, and fmt. These are branch-integrity
checks, not the prohibited final product quality gate.

There is no B-only product binary, product Phase A, latest-main/E37 product
screen, formal 102x5, all-layout final correctness, Default identity, complete
workspace/docs audit, binary/rlib product-size audit, or isolated product
replay. None is claimed as passing.

Raw TSV, stderr, build logs, targets, binaries, and the checksum manifest remain
external at
`/private/tmp/vp8l-metric-only-plan-search-p22-phase-r-688452ec`.
`SHA256SUMS` hashes to
`233a5c409080c83ff33fed74cd3cc0a1804eb0691579ad19e738624e99352843`.
Durable design, A-baseline summary, runners, summarizers, invalidation notes,
Phase R/recovery summaries, provenance, and this negative report remain on the
explicit P22 branch. Recommendation: reject and do not integrate either the
P20 A control or P22 B experiment into `main`.
