# Pre-screen control/candidate fairness review

The same-binary screen layouts share the same `prepare` validation and
tokenization path, exact `SinglePlan`, strict complete-RIFF fallback rule,
`SpatialCostPlan`, nested-map/table writer, and packed final token writer.
They differ only in spatial planning:

- `compact-control` / `low-latency-control` build the latest-main E37 ordered
  Boyer–Moore `SpatialPlan` and exact-cost it before writing only the selected
  stream;
- `compact` / `low-latency` collect exact block frequencies, build/cost E and
  B prefixes, perform the fixed single reassignment/rebuild, and exact-select
  single/E/B/refined before writing only the selected stream.

The legacy test helper that physically writes both single and candidate remains
only for strict fallback and byte-identity checks; benchmark layouts do not use
it. A unit test proves the fair ordered control and legacy control emit the same
complete file. No screen sample was taken before this review.
