# Public v1 exploratory trial — 2026-10-14

Status: deployed and runtime smoke verified; playing-strength superiority remains
unproven. This record does not change the frozen policy.

- Public site: https://marja.unofficialcrusaderpatch.com
- Running AI: 13d2015192ef04a27b21920ae314bd0853d35278.
- Running testbed: a2113678a41c636ce6c472e55119f4158edead88.
- Image: marja:a2113678a41c-13d2015192ef.
- Previous image retained: marja:cd553cc0beb0-b3abdaf1f5e4, immutable ID
  sha256:08d236cd356a380dad0c626521e4faa019f0789e7c5ed7db98fd1650ba21ddd2.

Backend-only guarded deployment completed. Health is ok with zero active games
and series. Mongo stayed running; frontend-release.txt remains cd553cc/b3abdaf.
The native module reports the exact running source pair. Two isolated in-memory
default-policy games (seeds7 and42) finished in49 and47 actions respectively;
each tested player projection excluded other hands, seed and the delivery bundle.
Public landing page loads; no logged-in browser game was played by this smoke.

Rollback retains the old image, its verified 75,933,450-byte gzip archive and
private configuration snapshot at /srv/marja/backups/20261014-pre-v1-13d2015.
The deployment also created fresh database/worker backups. Removed75,114,201
bytes of temporary upload staging; the builder is stopped and its reusable cache
retained. Current and previous release images remain. One disposable Windows
failed-test folder was left after automatic approval review rejected deletion.

The published updater reads its exact AI pin from the fetched testbed commit.
AI source/report checkpoint is on origin/v1/main; testbed release is on
origin/feature/takeback-claim. The installed command bootstrap on Kraris-GPTler
uses the stable wt/play checkout pair, preserving its local branches. Git Bash:
source ~/.bashrc; then update marja backend/server/frontend or restart marja server.
No restart is needed to try the deployed candidate. Active games/series block it.

Evidence: tmp/2026-10-14-v1-completion/{deploy.log,native-live-smoke.log,
live-verification.log,updater-tests-final.log,player-views-tests-final.log}.
Qualification, intervals, known strategic failures and latency tails are recorded
in 2026-10-14-v1-test-release.md. The existing old-backend health/stop race remains;
no atomic admission-drain guarantee is claimed.
