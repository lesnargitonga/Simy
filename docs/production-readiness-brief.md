# Production Readiness Brief

Use this brief to present current readiness posture with evidence.

## Current Position

- The platform is being hardened as an end-to-end encrypted communications system.
- The relay and cryptographic core are mature enough for controlled pilot operations when automated checks pass.
- Full general-availability production claims remain blocked pending completion of security and release controls listed in docs/production-readiness.md.

## Generate Evidence Snapshot

Run from workspace root:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/production_readiness_snapshot.ps1 -RunFullValidation
```

This writes a presentation-ready snapshot to:

- artifacts/readiness/production-readiness-snapshot.md
- artifacts/readiness/production-readiness-snapshot.json

## Suggested Stakeholder Statement

"The platform currently meets operational readiness for controlled production pilot usage when the readiness snapshot shows all checks passing. Full GA release remains gated by independent crypto review, release-signing maturity, complete device trust lifecycle controls, and incident-response hardening."