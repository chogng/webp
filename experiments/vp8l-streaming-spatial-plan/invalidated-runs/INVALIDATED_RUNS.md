# Invalidated runs

Every failed, partial, or methodologically invalid run is retained under a
descriptively named `raw/invalidated-*` directory and explained here.

No benchmark run was invalidated.

- The `f5e5bee5` screen is a valid failed variant measurement. It exposed that
  the first S state machine evaluated most nonmatching residuals twice; it is
  retained as evidence and is not classified as invalid.
- The `815df546` screen is the corrected, valid four-way S/C/F screen.
- The `292c1d74` screen is the valid, predeclared materialized-residual C+F
  diagnostic.
- One diagnostic setup command was launched from `webp-rs/` and could not find
  the repository-relative manifest. It exited before acquiring the benchmark
  lock or creating a run directory, so there is no partial benchmark to move.
- Two helper task streams disconnected after their edits were already on disk.
  No benchmark was running; this is recorded in `provenance.txt` as a task
  transport interruption, not as benchmark invalidation.

The formal 102-image, five-round benchmark was deliberately not started after
every valid 41-image screen failed the predeclared 10% and zero-regression gate.
