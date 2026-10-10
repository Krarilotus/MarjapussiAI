# Checkpoint: preserve a partner entry before asking for trump

- Date: 2026-10-10
- Status: verified for the bounded entry condition; playing-strength effect unmeasured
- Tag: `checkpoint/2026-10-10-partner-entry`
- Branch: `fix/live-planning-review`
- Base: `215f563a7dd80a42af5d66f0b691a0a3d31d8f1c`
- Paired testbed: `efa249122b28d112b15a977af25d832298b4b661`

## Result

Asking for partner's pair could create trump before a known partner Ace gained
the lead. Announcement timing checked only the leader's own retained entry.
Knowledge now certifies a specific partner winner against every possible legal
opponent reply. The heuristic can lead to that card before asking when partner
retains an own-announcement right, both pair halves and another turn to lead.
The existing own-entry condition and protected own pair suits remain in use.

The owner requested this dependency after a saved live game. Public tests use
synthetic cards and all four seat rotations; private game records remain local.
The certificate proves partner can take the lead, not that their policy must
choose it or that delaying trump always maximizes the final score.

## Evidence

All commands ran with one build job, two logical CPUs, below-normal priority,
4 GiB process-tree limit and one test thread, using the shared target cache.

| Check | Result |
|---|---|
| `cargo test -p marjapussi-heuristic --test play -- --test-threads=1` | 36 passed, including entry rotation, spent right, ruff and consumed-winner cases. |
| `cargo test -p marjapussi-knowledge --test soundness --test privacy_swap -- --test-threads=1 --nocapture` | Both passed. Existing 3,000-game harness checked 11,673 certified entries against all legal current-trick continuations. Hidden-card swaps remain invariant. |
| `cargo clippy -p marjapussi-knowledge -p marjapussi-heuristic --all-targets -- -D warnings` | Passed. |
| `cargo run --release -p marjapussi-tools --bin perf -- --games 16 --repeat 1` | 810 decisions, action hash `0d8b47366ca28314`. Trump-or-lead maximum 31.21 ms; ordinary lead maximum 16.24 ms. Pass-back maximum 944 ms remains the separate rollout workload. |
| Independent Sol source review | No blocking findings; clarified that partner has an available choice, not an enforced policy. |

The small timing run used a shared machine and has no paired baseline; it is a
runtime check, not a speedup or strength claim. Validation logs remain in the
shared `tmp/2026-10-09-v1` evidence directory, names `partner-entry-*-v1`.

## Remaining work and cleanup

Broader conditional entry sequences and uncertain partner holdings remain
separate planning work. Safe pre-play raising is the next owned iteration.
All validation jobs exited. Shared build caches and review evidence are retained;
this iteration created no disposable server or container.
