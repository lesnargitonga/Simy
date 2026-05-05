# Production Readiness Baseline

This document defines the minimum go or no-go controls before Simy can be treated as a production E2EE system.

## Scope

- The relay is not the trust anchor for plaintext.
- End-to-end security guarantees must hold at the client boundary.
- Passing relay tests alone is not enough for production release.

## Blocking Gates

All gates below are release blockers.

1. Independent cryptography and protocol review

- External review completed for X3DH bootstrap and Double Ratchet lifecycle handling.
- Findings triaged and resolved, with explicit residual-risk signoff.

2. Reproducible signed releases

- Release artifacts are reproducible from tagged source.
- Desktop release binaries are signed and signatures are verifiable.
- Update channel verifies signatures before install.

3. Device trust and revocation lifecycle

- Device enrollment and revocation flows are fully implemented.
- Trust-state transitions are visible to users and auditable.
- Key rotation and compromised-device recovery paths are tested.

4. End-to-end interoperability and recovery tests

- End-to-end encrypted send and receive works across restart, rekey, and out-of-order message scenarios.
- Session persistence and restore behavior is validated by automated tests.
- Failure injection covers network retries and relay unavailability.

5. Incident response and key compromise runbook

- Runbook includes key-compromise containment and user notification steps.
- On-call diagnostics and rollback instructions are documented.
- Security telemetry redaction policy is verified in CI artifacts.

## Current Status

- Current repository status is pre-production.
- Browser root UI remains a live testing and troubleshooting surface.
- Production candidate path is desktop client hardening first.