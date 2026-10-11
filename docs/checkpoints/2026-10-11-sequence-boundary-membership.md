# Boundary and nested prepared membership

Status: **incomplete; source-only handoff**. Follows owned `5750b041`.

Adds `prepared_lookup_with_boundary(receipt, view, legal, mind, context,
authored_membership, boundary_membership: Option<&Membership>, prepared)`.
Only the exact initial semantic suffix uses the supplied boundary comparison
domain. Subsequent stages use ordinary authored membership. Existing
`prepared_lookup` forwards None. Stored records, candidate ordering, posterior
scope, support and assessments are never rewritten to obtain a cache hit.

One canonical eligible-action iterator excludes UndoRequest only from
Membership.legal. Membership constructors, same-information membership cache
lookup, and the bound reconsideration record all use it. Actual engine legal
checks, offer binding, Table application and response identities remain intact.
Root constructs boundary membership through Membership::from_candidates, from
the current composed child stages including actual retained child, and owns its
global conditional-coverage certificate. This helper does not create that proof.

The source regression evaluates the complete bound authored-plus-extra-retained
domain on identical nondyadic support. Its explicit fixture order puts a legal
direct action first for stable ties; it constructs the parent certificate from
that actual outer ValueStore assessment and uses descendant records unchanged.
It verifies positive boundary-to-nested lookup, old-wrapper rejection, rejection
of a different retained child, and raw-vs-filtered UndoRequest membership parity.
No production ordering is changed. The existing near-end legal fixture is reused.

Root reported the preceding joint compile: 55 tests, 52 passed, all prior new
owned tests passed. The failed warm fixture exposed the two membership bugs
addressed here. These new tests remain **unexecuted**. Rustfmt parsing/formatting
and diff whitespace are the only local validation; no builds/tests were run.

Important remaining distinction: Tail still returns NeedsReview in lookup.
Selecting a legacy tail does not by itself provide a prepared executable Offer.
The upcoming explicit Response operator/receipt records are separate work; this
commit neither silently warms Tail nor claims all warm failures are resolved.
Root owns integration, coverage checks and validation. No builds or disposable
resources created; 0 bytes reclaimed.
