# Phase R summarizer final-materialization schema invalidation

The only valid P23 recovery measurement completed on the unchanged locked
binary before its precommitted summarizer ran. The first summarizer invocation
then stopped on a census-only assertion that required `candidates ==
metric_exact` for every stage.

That assertion is correct for E, B, R, and Growth candidate rows, but not for
`FinalMaterialization`: the locked audit schema intentionally records zero
analytic candidates, one exact final rebuild metric, and one final
materialization. All 204 final rows therefore had the valid tuple
`candidates=0, metric_exact=1, final_materializations=1`.

No benchmark, warmup, retained sample, identity encode, or audit was rerun.
The summarizer was narrowed to apply candidate equality only to candidate
stages and to require zero candidates for `FinalMaterialization`, then was run
again over the same external files under
`/private/tmp/vp8l-analytic-exact-cost-p23-phase-r-65d56bb8`.
