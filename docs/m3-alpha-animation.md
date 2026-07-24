# M3: alpha and animation functional exit

M3 completes the functional integration of WebP transparency and animation.
It does not claim the performance gate: performance work is intentionally
scheduled after the remaining functional milestones are complete.

## Delivered public behaviour

- Static VP8 frames combine their `ALPH` plane with decoded RGB into straight
  RGBA8. `ALPH` supports raw and headerless-VP8L compression, with none,
  horizontal, vertical, and gradient inverse filters.
- `decode_animation` validates an extended animation container and returns a
  full straight-RGBA8 canvas snapshot for every display frame. It applies
  blend, disposal-to-background, offsets, duration, loop count, and the ANIM
  BGRA background colour.
- `AnimationDecoder` is the bounded streaming alternative: it borrows the
  complete container, returns one composed canvas per pull, restarts with
  `reset`, exposes the validated demux view, supports RGBA/BGRA plus
  premultiplied output, and has an opt-in worker-thread path for independent
  color and `ALPH` payloads.
- Strict parsing checks ANMF geometry, nested chunk ordering and framing,
  resource bounds, and the VP8X alpha feature bit when an animation carries an
  `ALPH` subchunk. The compatible profile retains its documented recovery
  behaviour.

## Functional evidence

- Unit tests cover raw and lossless alpha filtering, canvas replacement/blend/
  disposal, malformed frame layouts, alpha-feature consistency, and resource
  limits.
- Pinned libwebp alpha vectors cover raw and lossless alpha filters. When the
  local oracle is available, alpha bytes are compared directly with `dwebp`;
  RGB is compared only for opaque pixels because `dwebp`'s PAM writer
  premultiplies translucent RGB while this API returns straight RGBA.
- Generated libwebp animation vectors assert pixel results for offset blend,
  disposal, background restoration, duration, and loop count.
- `animation_raw` fuzzes the public animation decode path under bounded input,
  dimension, frame, allocation, and work limits. Bootstrap always supplies a
  valid minimal ANIM/ANMF seed and uses the external animation corpus when it
  is available. The initial 10,000-run smoke pass completed without a crash.

## Exit status

The M3 functional scope is complete once the checks above remain green in CI.
The shared quality-gate performance and profiling requirement is explicitly
**performance pending** and must be handled only after functional scope is
complete, per the project sequencing policy.
