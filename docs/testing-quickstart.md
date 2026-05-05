# Testing Quickstart

This document gives a smooth, repeatable testing path for local development and CI.

## Local Fast Path (Windows PowerShell)

Run professional preflight checks (toolchain + env validation):

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/preflight.ps1 -EnvFile ./.env
```

Run strict secret linting (rejects placeholder admin token values):

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/preflight.ps1 -EnvFile ./.env -EnforceNonDefaultAdminToken
```

Start relay with strict env validation and optional dependency startup:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/run_relay.ps1 -StartDependencies -WaitForHealth
```

Run Rust tests and feature-gated ratchet tests:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/test_all.ps1
```

Run everything above plus relay smoke test (requires relay already running on 127.0.0.1:8081):

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/test_all.ps1 -RunRelaySmoke -RelayHealthTimeoutSeconds 20
```

Note: `test_all.ps1` now runs preflight automatically unless `-SkipPreflight` is provided.

Run only relay smoke test:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/relay_smoke.ps1 -BaseUrl http://127.0.0.1:8081
```

Run relay smoke and save structured JSON output for diagnostics:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/relay_smoke.ps1 -BaseUrl http://127.0.0.1:8081 -OutputPath ./artifacts/smoke.json
```

Run relay smoke with latency SLO enforcement:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/relay_smoke.ps1 -BaseUrl http://127.0.0.1:8081 -MaxRetrieveLatencyMs 3000 -MaxTotalLatencyMs 8000 -FailOnLatencySlo
```

## Mobile Platform Tests

Android instrumented ratchet store test:

```bash
gradle --no-daemon :simy-security:connectedDebugAndroidTest --stacktrace
```

iOS Swift package ratchet store tests:

```bash
swift test -v
```

## CI Workflows

- Core and relay: `.github/workflows/core-relay-tests.yml`
- Mobile ratchet: `.github/workflows/mobile-ratchet-tests.yml`

The core relay CI now runs the same preflight gate used locally.
On failure, CI uploads a bundled artifact containing relay logs, smoke logs, and a health snapshot.
CI now also publishes a job summary with smoke health and retrieval counts for quick triage.
CI artifact processing now redacts known token-like values before upload.

## Troubleshooting

- If relay smoke fails with connection errors, start relay and verify `GET /healthz` first.
- If relay startup fails, run `./scripts/run_relay.ps1 -StartDependencies -WaitForHealth` to get strict env and dependency diagnostics.
- If Android emulator jobs are slow, keep API level and profile stable to benefit from CI caching.
- If iOS tests fail on keychain behavior, ensure test cleanup removes per-test aliases.
