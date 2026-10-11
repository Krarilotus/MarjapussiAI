# Completion dominance in an authored cashing branch

Base: `676dc5f`. Status: **incomplete**. The authentic descendant and 146 library
tests pass; both original whole-planning gates still defer at their existing
limits. Intended tag: `checkpoint/2026-10-11-authored-cash-dominance-integration`.

Integrated B's `3520655` and A's independently reviewed descendant fixture
(private patch SHA256
`58f3e4855c02b422092e2ec52e1ccfc5edb5fbe58001861598a2393dfff1d65f`).
E reviewed both source changes. Only `sequence/dominance.rs` changes production
behavior; the remaining additions provide structural and scenario tests.

The first alternative can now be literal Tail. Rival suffixes may contain a
validated DAG of card actions, choices and guards ending at Tail. Every choice
entry and every yes/no/unknown guard arm is checked. Actual guard truth, future
card bindings and domain ordering are preserved by the ordinary evaluator.
Unsupported gates remain ineligible. Empty bindings keep their existing
interruption-to-Tail behavior. The selected route still has to complete normally.

The authentic fixture follows the existing composed resource program through a
legal three-Ace prefix in the original two worlds, weighted 0.4/0.6. Its diagnostic
receipt preserves the full owning program. At the existing final cash choice,
the real finish evaluator completes Tail with engine score difference -420 and
proves the unexecuted rival cannot improve it. An ordinary owning-program Tail
reference agrees on response calls, transitions and terminal outcomes. The probe
does not construct enclosing Parent/Global authorization or a SelectedPlan.

## Runtime evidence

All checks used the existing serialized wrapper: two logical CPUs, below-normal
priority, 4 GiB process-tree commit, Cargo jobs one, test threads one, optimized
test profile with the profiling feature. Receipts remain in the private
`tmp/2026-10-09-v1` evidence directory.

| Receipt | Result | Log SHA256 |
|---|---|---|
| `completion-dominance-suffix-authored` | 146 library tests passed, including six structural tests and the authentic descendant; two expensive cases tested separately | `333cece732df9921966cb804bc239944cf9fe688a4f1db7b9d6902b992230dd4` |
| `completion-dominance-suffix-six-card` | Failed: whole decision exhausted 32,768 represented comparisons | `ca41d0f96f345ef81bd99fe3393fe7de6f21cdecbc5f14fe022f87026c3bb9eb` |
| `completion-dominance-suffix-depth-two` | Failed: auction/return decision exhausted 2,000,000 represented transitions | `f7872687c200e9659a1c0de7576957837a9d03e033f2824cfc9de881adbac234` |
| `completion-dominance-suffix-clippy` | Compiled with the existing eight library and three test warnings; no new warnings | `6f31c6216ee5828030585838d4fd79457c336fc491cffe4156804f37f213c200` |

The six-card case used 321,820 actual / 362,390 represented transitions and
32,551 actual / 32,768 represented comparisons. The depth-two case used
1,258,262 / 2,000,000 transitions and 99,467 / 99,467 comparisons. Both completed
zero outer candidates. Their domains, support, limits and assertions were not
weakened. Different stopping points preclude a causal speed comparison with
earlier runs.

## Next diagnosis

Failed proof attempts cost one comparison before ordinary full comparison.
Optional cache reuse still recharges represented work. Exact numerical reuse can
reduce physical computation, but saving represented units requires a reviewed
single dependency owner, not a cache rebate. Current counters do not attribute
attempts, successful proofs or completed NoSeparation results sufficiently to
choose that change. Measure those demands and exact repeated numerical inputs
before expanding authorization or changing ownership.

Whole-root completion, performance qualification, strategic dryruns, the paired
256-game campaign and the new review-page delivery remain pending. Production
deployment is unchanged.
