# Shared continuation reconstruction

Status: verified reconstruction/refactor; broader planning release remains incomplete.
Base: `6239b6e`. Tag: `checkpoint/2026-10-10-continuation-reconstruction`.

`agent::Table::from_view` reverses visible exchanges and played cards, then replays
the canonical seat history through engine legality. It requires the reconstructed
deciding view to match exactly. Hidden transfers require a history hypothesis and
are rejected; the helper does not invent them. Public seat kinds and series state
are retained. Return forecasts use this helper instead of their separate replay.

Validation: `continuation-reconstruction-checks` ran the agent and heuristic library
and integration suites: 95 tests passed, 22.97 seconds, 292.9 MiB peak committed
memory, one build job/two logical CPUs/4 GiB cap. New tests cover full visible
exchange/play reconstruction, announcement history, all observers' private views,
and rejection of hidden transfers, wrong own hands and duplicate/missing cards.
Formatting passed. Independent review could not start: the verified T3 agents
relay was unavailable; neither window was restarted.

This iteration does not change candidate selection or introduce the broader
contract/sequence planner. The preceding joint-belief checkpoint remains subject
to release qualification before promotion to the live branch.
