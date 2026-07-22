# Superseded Phase A: full-plan trace ownership

The first passing Phase A run used binary `034ae904…f01c` and is retained under
`raw/phase-a-102`. Its audit trace kept every historical `PlanParts`, even
though only scalar split metadata is needed. That ownership could unfairly
inflate candidate RSS and copying cost in a same-binary screen.

Commit `cea589b5` moved accepted plan ownership directly into the current plan
and retained only scalar historical metadata. Binary `fac7d627…bc7` reran
Phase A under `raw/phase-a-102-final` and exited zero. A second fairness review
then made P16 reuse the exact E37 preparation/tokenization/single owner in
`58327b09`; screen binary `1828e721…d84e` reran under
`raw/phase-a-102-screen-binary` and exited zero. All 4,518 keyed
image/profile/iteration rows are identical across all three runs after
excluding wall-time fields; rates, assignments, groups, seeds, penalties,
partitions, decisions, hashes, and exactness flags did not change. Only the
screen-binary run authorizes the screen.
