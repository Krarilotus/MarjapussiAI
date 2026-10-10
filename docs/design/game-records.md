# Game records (`marjapussi.game-record/1`)

Coder (heuristic player), 2026-10-06, C14. Owner of the format: `crates/tools/src/record.rs`. Readers: the dev UI (playback), analyses.

One game per line (NDJSON, UTF-8). Agent games and human games use the same schema.

## Producing records
- Agent games: `arena --games N [--seed S] [--params FILE] --records FILE [--records-beliefs]` (4×HP1 self-play).
- Human games: `human_eval --games PATH --records FILE [--records-beliefs]`. Each human decision also carries what HP1 would have done from the same seat view.
- Size: about 5 KB per game without belief snapshots, about 30–36 KB with them. Keep record files out of the repository (they derive from the human dataset).

## Privacy
Human records never contain names. Players become stable pseudonyms `P1`, `P2`, … in order of first appearance in the dataset file, so one person keeps one pseudonym across games of one export. The mapping is not written anywhere.

## Top level

| Field | Type | Meaning |
|---|---|---|
| `schema` | string | always `"marjapussi.game-record/1"` |
| `source` | `"hp1"` \| `"human"` | who played |
| `id` | string | `hp1-s<seed>-g<n>` or `human-<line index>` |
| `seed` | integer \| null | deal seed (agent games) |
| `params_hash` | string \| null | FNV-1a hash of HP1's parameters (agent decisions; for human records: of the HP1 that made the suggestions) |
| `date` | string | `YYYY-MM-DD` (agent games: day of the run), `YYYY-MM` (human games: creation month) |
| `players` | [string; 4] | seat 0…3: `"HP1"` or a pseudonym |
| `deal` | [[card; 9]; 4] | start hands by seat |
| `actions` | [action] | every action after the four "start" actions, in order |
| `result` | object | outcome, below |

Seats are 0–3 clockwise; seats 0 and 2 form team 0, seats 1 and 3 team 1. Seat 0 opens the auction.

**Cards** are `"<suit>-<value>"`: suits `g` Green, `e` Acorns (Eichel), `s` Bells (Schell), `r` Red; values `6 7 8 9 U O K Z A` (Z = ten).

## Actions
Every action has `seat` and `type`, plus fields by type:

| `type` | fields | meaning |
|---|---|---|
| `bid` | `value` | a bid in the auction |
| `pass_bid` | – | leaves the auction |
| `pass_cards` | `cards` (4), `dir` `"forth"` \| `"back"` | MH passes to VH (forth), VH passes back |
| `raise` | `value` | VH raises the game value after the exchange |
| `card` | `card` | a card played |
| `announce` | `suit` | announces own pair as trump |
| `ask_pair` | – | asks the partner "Yours?" |
| `ask_half` | `suit` | asks the partner for a half of `suit` |
| `answer_pair` | `suit` | answers with a pair (it becomes trump) |
| `answer_no_pair` | – | no pair |
| `answer_half` | `suit` | holds a half of `suit` |
| `answer_no_half` | `suit` | holds no half of `suit` |

Optional fields on any action:
- `claimed_by` (seat 0–3): automatic card/announcement completing that seat's remaining-trick claim; retained in playback but excluded from human-choice assessment. Legacy archives may instead have top-level `claim: {"by": <seat>, "from": <zero-based raw actions index>}`: every raw action from `from` through the end is automatic. The boundary precedes a card or claimant announcement, never a grouped pass; replay validates legal actions, an empty-trick claimant turn at the boundary, and claimant wins for every remaining trick. Conversion keeps one note per applied engine action, so grouped passes and skipped legacy markers do not shift provenance or annotations.
- `trump` (suit): this action made `suit` trump (announcement, pair answer, or a half answer that completed a pair).
- `only_half` (suit): a half answer that did not complete a pair.
- `rule`, `why` (agent games): the HP1 rule ID that produced the action (`crates/heuristic/src/rules.rs`) and its facts, e.g. `{"egp": 131.2, "step": 5}`.
- `hp1` (human games, decisions with more than one legal action): `{ "action": <action>, "rule": "HB-4", "why": {…} }`, what HP1 would have done from the same seat view.
- `beliefs` (with `--records-beliefs`): the deciding seat's beliefs before acting:

```json
{ "seat": 2,
  "about": { "1": { "ace": 0.71, "pair": [g, e, s, r], "half": [g, e, s, r], "void": [g, e, s, r],
                    "cards": { "r-A": 0.39, "s-K": 0.12 } },
             "3": { … }, "0": { … } } }
```
`about.<seat>` is what the deciding seat believes about that seat's current hand: P(at least one ace), P(pair), P(at least one half) and P(void) per suit in the order Green, Acorns, Bells, Red, and P(holds) for the uncertain aces, tens, kings and obers (cards that are certain or impossible are left out). Values are rounded to 2 decimals.

## Result

| Field | Type | Meaning |
|---|---|---|
| `declarer` | seat \| null | the seat that won the auction (null: all passed) |
| `declarers` | 0 \| 1 \| null | its team |
| `value` | integer | final game value (115 if all passed) |
| `made` | bool \| null | the declarers reached the value |
| `schwarz` | bool | one team took no trick |
| `team_points` | [int; 2] | card points plus melds, by team |
| `scores` | [int; 2] | game score by the rules page (`GameFinishedInfo::scores`) |
| `tricks` | [{`cards`: [card; 4], `winner`: seat, `points`: int}] | in play order; the first card of each trick is the lead |

## Versioning
A change that removes or renames a field bumps the version (`/2`). New optional fields keep `/1`.
