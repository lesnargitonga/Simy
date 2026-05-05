# Next Build Plan

## Objective

Move from feature-by-feature execution to a continuous delivery program that ships secure, testable increments every week across core, relay, and clients.

## Delivery Model

### Workstreams

Run these tracks in parallel instead of serial one-by-one tasks.

1. Protocol and core crypto
- ratchet persistence lifecycle hardening
- message replay policy and error taxonomy
- deterministic test vectors for client wrappers

2. Client storage and runtime
- Android first store integration with keystore-backed encryption
- iOS storage contract parity and keychain plan
- desktop storage contract parity and local hardening plan

3. Device trust and identity lifecycle
- signed device registration and display model
- key-change and device revocation UX contract
- trust transition state machine and audit events

4. Relay and operations hardening
- endpoint abuse control tuning
- structured observability and alert baselines
- deployment and rollback runbooks

5. QA and security validation
- integration tests for restart-safe sessions
- regression test matrix for relay plus client contracts
- security gate checks and release criteria

6. User-side product flow and end-to-end communication
- define complete user journeys from workspace creation through verified contact establishment and message exchange
- ensure identity, pairing, device trust, recovery, and media flows connect without manual dead ends or contradictory UX
- maintain a user-facing demo and test path proving two users can communicate end to end with the current implementation

## Cadence

### Weekly Sprint Rhythm

1. Monday planning
- lock scope for one-week sprint
- mark dependencies and blockers
- assign owner per workstream deliverable

2. Daily execution
- 15-minute async standup in shared channel
- blocker escalation within same day
- maintain one active branch per workstream

3. Wednesday checkpoint
- integration cut from all active streams
- run full test matrix on integration branch
- either continue or reduce scope to preserve ship date

4. Friday release candidate
- freeze new scope
- run release checklist and security gates
- publish sprint demo notes and next sprint delta

## Backlog Rules

### Definition of Ready

No work starts unless all are true:

- user outcome and risk are explicit
- affected end-to-end user journey is identified when the change touches client behavior
- test strategy is defined
- dependencies are identified
- rollback and recovery are understood

### Definition of Done

No work closes unless all are true:

- code merged with passing CI
- tests added or updated
- docs updated in affected areas
- operational impact recorded

## Quality Gates

Every release candidate must pass:

1. core tests
- cargo test -p comm-core
- cargo test -p comm-core --features ratchet-store-fs ratchet_store_fs

2. relay checks
- cargo check -p relay
- relay health and authenticated happy-path checks

3. security checks
- no plaintext persistence of session secrets
- no bypass of admin and managed-user boundaries
- no regression in replay protection behavior

4. documentation checks
- README consistency with active runtime and APIs
- docs and examples aligned to current implementation

5. user-flow checks
- workspace setup, pairing, contact sync, and direct-message exchange succeed across two user contexts
- recovery paths for broken local state, key changes, or missing local linkage are documented and testable

## 6-Week Execution Plan

### Weeks 1-2

Primary outcomes:

- complete Android keystore-backed ratchet session storage wiring
- add restart continuity integration tests around serialized ratchet session flow
- stabilize error handling and recovery semantics for failed session load

Exit criteria:

- Android storage adapter can save, load, and delete encrypted session state reliably
- restart continuity tests pass in CI for core plus adapter contracts

### Weeks 3-4

Primary outcomes:

- implement device trust and revocation contract across relay plus client boundary
- define signed device update and key-change handling UX contract
- add trust transition audit trail and regression tests
- document the user-side trust and pairing journey so the security model is visible in the product flow instead of only backend behavior

Exit criteria:

- trust state transitions are deterministic and tested
- revocation behavior is reflected in relay responses and client handling
- the user-facing flow explains who is trusted, what changed, and how two users restore end-to-end communication after a disruption

### Weeks 5-6

Primary outcomes:

- automate client media transfer orchestration around existing relay media APIs
- finalize release packaging checks and deployment guardrails
- publish v1 hardening report with residual risk list

Exit criteria:

- media upload and download path is automated in client flow
- release pipeline can produce repeatable signed artifacts
- unresolved risks are documented with owners and due dates

## Program Metrics

Track these weekly:

1. delivery
- planned vs completed sprint commitments
- cycle time from ticket start to merged

2. quality
- test pass rate on integration branch
- escaped defects per sprint

3. security
- open high-severity findings
- mean time to fix security regressions

4. operational readiness
- deployment success rate
- rollback readiness verification status

## Immediate Next Actions

1. create sprint board with lanes per workstream
2. convert this plan into sprint-1 tickets with owners and test tasks
3. lock sprint-1 scope to Android storage completion plus restart continuity tests
4. schedule mid-sprint integration checkpoint and Friday release gate review
5. add a standing user-journey review for setup, pairing, trust, recovery, and end-to-end communication before closing client-facing work
