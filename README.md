# MarjapussiAI

A fair, fast AI for the card game Marjapussi. Rules: marjapussi.de/rules.

- `crates/engine`: the corrected Rust rules engine (provenance in `NOTICE`).
- `docs/ARCHITECTURE.md`: the layers and their invariants. The AI never sees hidden cards.

Build and test (on the shared PC):

```sh
CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=../tmp/<you>-target cargo test -j 2
```

Licence: GPL-3.0.
