# Invalidated Phase A: pre-attribution binary

Binary `2cb26c02000f9d483bb9eadb87ab6b00658b879b45016ddf0ccd07292595bf9d`
produced exact expected rates (Compact 599,398,064 B; LowLatency 601,400,998 B)
and passed all exactness/tail checks. It is nevertheless invalid as final P18
evidence because the Phase A schema omitted separate shared-prepare and
selected-writer timing fields required by the frozen attribution gate.

The planner and wire rules did not change. Instrumentation added only those two
timers, which changes the test binary; therefore the entire Phase A and every
later phase must use the replacement binary. The raw TSV, empty stderr, summary,
and tail evolution are retained here and are excluded from final gate counts.
