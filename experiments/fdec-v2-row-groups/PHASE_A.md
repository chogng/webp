# FDEC v2 Phase-A exactness audit — negative stop

## Identity

- Root task: `019f8321-035e-7211-8f53-987e18891c8c`
- Worktree: `/Users/lance/.codex/worktrees/f9db/webp`
- Branch: `codex/fdec-v2-row-groups`
- Creation base: `564adc27c6366f18fc51a8a475c28f473cfbfa1f`
- Candidate/auditor source: `3f9b6499035f5c9195b47b217b65bed39594518d`
- Release binary: `webp-rs/target/release/examples/fdec_v2_research`
- Binary SHA-256: `0a85f5535f40502067eb0b04270181b0f82eddeddd042a38eb6e86222295c654`

## Fixed corpus reconstruction

The retained temporary CSV was only a 165-row subset and was rejected before
output creation. The audit then used the read-only E17 committed ledger:

```text
/Users/lance/.codex/worktrees/cb21/webp/docs/fdec-generalization-samples.csv
SHA-256 3e9e0c0b6546826ba121f40d9cd98ae9f742bdf2c084504a2fa6b22aefe2f878
```

It has the required 229 distinct identifiers and 28 non-opaque inputs. Legacy
containers were read from
`/private/tmp/fdec-generalization-candidates/zstd-rgb-sub`; every candidate is
first reduced to its RIFF fallback and compared with the fixed fallback hash.
The auditor creates A (standard decode), the same-binary whole-image v1 O
controls, and B practical/fastest candidates. It also checks B private
selection, standard fallback equality, fallback-byte identity, no full-frame
residual allocation, one output-byte visit, and pinned `dwebp` PAM pixels.

## Run and invalidation

The only executable audit run was:

```text
webp-rs/target/release/examples/fdec_v2_research audit \
  /private/tmp/fdec-generalization-candidates/zstd-rgb-sub \
  /Users/lance/.codex/worktrees/cb21/webp/docs/fdec-generalization-samples.csv \
  /opt/homebrew/bin/dwebp \
  /private/tmp/p26-fdec-v2-phase-a-3f9b6499
```

It stopped before processing `clic-test-mobile-01` because O practical failed
private selection. The first five B practical and fastest candidates were
written, while O completed only one opaque candidate. The raw output is kept
at `/private/tmp/p26-fdec-v2-phase-a-3f9b6499` and must not be overwritten or
used as a timing input. Representative artifact hashes are:

```text
2715266faffc55e1fd790236b83ce69d799463edff2cc4e2f1d695cfe8014550  b-practical/alpha-binary.webp
952bc467ca63902d75e38cc52e52731ec34c2e6c7f0ef41ffc7b1c5a3a51ed8c  b-practical/clic-test-mobile-00.webp
```

The denominator was therefore not proven. No decode warmup, timing sample,
rate result, or product-viability claim was run. Release/default remain
standard A and product migration is prohibited. A fresh research task must
repair and independently review the v1-control failure, start from a new
latest-main base, and rerun the entire exactness gate with a new output path.
