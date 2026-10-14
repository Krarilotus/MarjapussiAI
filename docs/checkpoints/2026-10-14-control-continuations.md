# Checkpoint: control-preserving continuations

- Date: 2026-10-14
- Status: **incomplete; validation pending**
- Branch: `work/2026-10-14-v1-control`
- Base: `af55bed`
- Tag: `checkpoint/2026-10-14-control-continuations`
- No deployment or paired testbed change.

## Result

The [design note](../design/control-continuations.md) records the prerequisites,
counter-cases and information boundaries. This changes five heuristic source
files, with synthetic tests inside their existing modules.

Return comparison has an independent default-enabled `return_run_route` switch;
`pass_route` stays false. It compares the stored return with the best stored
candidate preserving each top-controlled suit, including pairless runs. The
same joint worlds, actor-local continuation plans and terminal engine scoring
evaluate both. Only a clearly better evaluated return replaces the baseline.
The cheap level-0 return preserves the existing paired-run prior, or the longest
top-controlled suit when no trump plan exists. A suit contains at most nine
cards, so preserving it in a thirteen-card hand leaves at least four return cards.

Question deferral is attached to a specific lead-retaining cash, not merely a
top-trump hand shape. At least three cards are needed to leave partner a spare
beyond its pair. No-trump deferral additionally requires a non-half cash and an
own half in its suit, excluding partner's full pair in that suit. Only Yours is
deferred; own announcements and half questions retain their existing priority.
HO2 still evaluates positive partner activation and cross-ruff alternatives.
The private continuation plan now belongs to its selecting actor, never both
teammates. The remaining-card union in `lead0` is public and stays unchanged.

HL4's fallback priority and partner-overtaking prerequisite are corrected, but
`lead_draw_trump` remains false. Default effects come from the continuation and
question priors, not a newly enabled unconditional draw-trump rule.

## Evidence

| Check | Result | Limitation |
|---|---|---|
| Workflow, source, development examples and available owner notes | Read | Examples motivate behavior; not a strength measurement |
| Root/reviewer source feedback | Incorporated | No independent final approval claimed |
| Rustfmt on owned files | Completed | Parsing/formatting only; no type check |
| Staged whitespace/scope review | Completed with CRLF-aware Git check | Existing per-file line endings retained |
| Synthetic tests | Authored, **not run** | Build/test execution reserved to root |
| Recorded fixture replay | Queries prepared, **not run** | No claim that recorded choices or results improved |
| Performance / strength / live acceptance | **Not run** | Root owns serialized qualification and backed-up live build |

Synthetic coverage includes actual default return evaluation from thirteen
visible cards, independent configuration, run versus length, legal return sizes,
hidden-card swaps, private plan ownership, own-pair announcement before a cash
consumes a half, safe non-half cash, half-question retention, and both harmful
and valuable partner activation at two cards scored through the engine.

Reused development source: `fresh2000-7309dd`. Selected recorded game indices
1153, 1729 and 1944 correspond to development deals 576, 864 and 972. Fifteen
queries cover returns, announcements, lead transfers and Yours decisions.
Query-file SHA256:
`9d43347ebd366c2a71287e9c247eb31fb3df60ec2231a4484b1afc1ed2bce37b`.
Source records and query/record fingerprints remain in the external fixture
manifest; no raw deals or participant identifiers are committed.

## Required root validation

One bounded job at a time, `CARGO_BUILD_JOBS=1`, two logical CPUs, below-normal
priority and 4 GiB committed memory; preserve the checkout's target cache:

```text
cargo test -p marjapussi-heuristic --lib -- --test-threads=1
cargo test -p marjapussi-heuristic --test play --test passing --test params -- --test-threads=1
cargo build -p marjapussi-tools --bin replaydiff
```

Feed the prepared `queries.ndjson` to `replaydiff --run <source-run>` using the
root-owned build, and compare against the frozen previous policy. Replay checks
individual decisions on recorded prefixes; full continuations and an untouched
paired comparison are still needed to assess outcomes and strength.

## Limits and remaining work

No Rust compile or runtime result is available for this commit. Any failing test
or changed action must be reported before promotion. The existing verified
return-planning/entry checkpoints and root's backed-up live build are not replaced
by this incomplete checkpoint; `af55bed` is the frozen implementation baseline,
not a newly verified release claim.

Return forecasts use the current contract and skip the later Raising decision.
The continuation policy and sampled comparisons remain estimates. The new return
path adds work only when it finds a distinct run-preserving candidate; its cost
and effects on existing HO1/HO2 forecasts remain unmeasured. Root must replay the
three reviewed examples and recheck positive activation/cross-ruff cases before
any strength or live-readiness claim.

## Cleanup

No build, test, benchmark, container, server or background process was started.
No disposable build output was produced; reclaimed space **0 bytes**. Shared
caches and source data are retained. The small prepared query/manifest files are
retained outside Git for root's planned validation.
