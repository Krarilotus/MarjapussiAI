# Rollout public activation facts

Base: 13d2015. Focus: preserve primitive public facts already owned by engine and
knowledge in the actor-response rollout. No strength claim or plan ownership change.

Known will carry possible uncalled pair suits separately from per-card may sets.
Initialize them from mind.public().pairs(seat).possible, never the deciding seat's
private deductions. Public card play and exclusions can only narrow this mask.

One observed engine-move path handles both direct and automatic answers and
announcements. NoPair excludes every still-uncalled whole pair for the respondent;
NoHalf excludes both halves. Announce/YesPair reveals both halves at that actor,
so remove them from other seats' possibilities. Do not infer a particular half
from YesHalf or inspect hidden allocations to learn public facts. Positive pair
ownership clears that owner's false blank reading. YesHalf clears the respondent's
reading; only a newly called pair proves the asker has a half and clears its
reading too. YourHalf is legal without an asker half, so a question alone never
clears an inferred blank reading.

Hypothetical exchanges invalidate the transferring seats' old pair exclusions:
widen their possible holdings to the team union and recompute only those seats'
pair masks. Loading a historical exchange into Known must NOT perform that reset.
Opponent constraints survive; post-exchange actor choices still use own cards and
public/legitimately exchanged information. Engine transitions and terminal scoring
remain authoritative. A public NoPair still allows later split-half questions.
After loading historical exchange readings, public Knowledge::count_bounds supplies
the cheap minimum suit count: a positive public minimum overrides the reading.
The observer's private certain cards or sampled allocations are not consulted.

Synthetic coverage: seeded public vs private pairs, direct/automatic NoPair and
NoHalf, legal repeated-Yours suppression, positive pair ownership, hidden swaps,
played-half exclusion, and hypothetical vs historical exchange invalidation.
Controls include a positive half answer without an asker half, a question-only
prefix, hidden half swaps, public lower bounds without certain card identity,
and private kept halves that must not clear a public reading. The existing
Start/Cards cache comparison checks the pair mask alongside every actor field;
the cache remains scoped to a fresh context with fixed public facts. A changed
public NoPair control warms the same query in the old context, confirms the fresh
context is empty, then compares fresh cold/warm results with the uncached oracle.

No new strategy weights/cases or actor-local response changes. This corrects
facts used by the honest response model; it is not a claimed remedy for the four
selected small-sample losses. Root reports a separate 2,000-game comparison of
13d2015 against af55: -0.30 with approximate interval [-2.48, +1.87], establishing
neither a drop nor an improvement. Old team-response semantics recovered those
four losses diagnostically but remain excluded from this patch.

No builds or tests by this author; root owns serialized validation and full
runtime qualification is pending.
