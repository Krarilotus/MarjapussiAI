# Checkpoint: shared seat-view table

- Date: 2026-10-09
- Status: verified refactor; coordinated planning is still in progress.
- Tag: `checkpoint/2026-10-09-simulation-table`
- Branch: `integration/reviewed-ai`
- Base: `06a68fe75bc9cc02f571b6fb9ac224c8104b30ed`
- Testbed: unchanged, `353c786aad5ee64d683fe489ff6bdcc10d1fcbd2`

Move the existing table's seat filtering/tracking to `agent::Table`, re-exported
unchanged by `eval`. This makes the same privacy boundary usable for production
rollouts without depending on evaluation or copying the runner. No game rule,
agent choice or table algorithm changed. See [continuation design](../design/return-rollouts.md).

Release checks: frozen exchanges, 500 hidden-card swaps and deterministic
50-game self-play all pass. The 50-game before/after trace contains the same
2,642 actions and rule labels (action hash `f86598bd7118d640`). One explanation
differs only in the worlds completed before the existing 45 ms deadline (62 vs
61); the action/value is unchanged. Mean non-forced latency 794 vs 813 us on
the shared PC; this is no speedup claim. Source logic moved unchanged.

Evidence: outer workspace `tmp/2026-10-09-v1/table-{before,after}.{txt,log}`.
No private records added. T3 reviewer unavailable: agents window failed to start
on DevTools port 9335; no independent approval claimed. No merge or push.
Build caches retained for the ongoing rollout implementation; 0 bytes reclaimed.
