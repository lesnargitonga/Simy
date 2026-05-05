# Testing Incident Runbook

This runbook maps common test failures to immediate diagnosis commands and likely fixes.

## 1) Preflight fails

Symptoms:
- Missing command errors (for example `cargo` or `docker`)
- Missing required environment variable errors
- Admin token placeholder rejected

Commands:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/preflight.ps1 -EnvFile ./.env
powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/preflight.ps1 -EnvFile ./.env -EnforceNonDefaultAdminToken
```

Fixes:
- Install missing toolchain components (Rust or Docker).
- Populate missing values in `.env`.
- Replace default `RELAY_ADMIN_TOKEN` with a strong non-placeholder value.

## 2) Relay does not become healthy

Symptoms:
- Relay health wait timeout
- Smoke script cannot connect to `/healthz`

Commands:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/run_relay.ps1 -StartDependencies -WaitForHealth
Invoke-RestMethod -Uri http://127.0.0.1:8081/healthz
```

Fixes:
- Ensure PostgreSQL and Redis are up (`docker compose up -d`).
- Verify `POSTGRES_DSN` and `REDIS_URL` are correct in `.env`.
- Confirm relay bind address matches expected URL.

## 3) Smoke test cannot find posted message

Symptoms:
- `smoke test failed: posted message not found`

Commands:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/relay_smoke.ps1 -BaseUrl http://127.0.0.1:8081 -OutputPath ./artifacts/smoke.json
Get-Content ./artifacts/smoke.json
```

Fixes:
- Increase polling tolerance:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/relay_smoke.ps1 -BaseUrl http://127.0.0.1:8081 -MaxPollAttempts 10 -PollDelayMilliseconds 500
```

- Check relay logs for request failures or auth mismatches.

## 4) Latency SLO violations

Symptoms:
- Retrieve or total latency above configured SLO
- Failure when `-FailOnLatencySlo` is enabled

Commands:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/relay_smoke.ps1 -BaseUrl http://127.0.0.1:8081 -MaxRetrieveLatencyMs 3000 -MaxTotalLatencyMs 8000 -OutputPath ./artifacts/smoke.json
```

Fixes:
- Confirm host machine is not overloaded.
- Check Docker resource limits for database and cache containers.
- Increase thresholds temporarily only when justified by environment constraints.

## 5) CI core-relay workflow fails

Symptoms:
- GitHub Actions `core-relay-tests` job failure

Where to inspect:
- Job summary section in the workflow UI for smoke health and latency fields.
- Uploaded artifact bundle: `core-relay-failure-artifacts`.

Expected artifact contents:
- `artifacts/healthz.json`
- `artifacts/relay.log`
- `artifacts/smoke.log`
- `artifacts/smoke.json`

Fixes:
- Use the same command sequence locally to reproduce.
- Compare local smoke JSON event sequence with CI sequence.
- Apply config, dependency, or timeout adjustments based on the failing step.

## 6) Full local verification before push

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/test_all.ps1 -RunRelaySmoke -RelayHealthTimeoutSeconds 20
```

If this command is green and CI still fails, prioritize environment-specific differences (runner load, networking, container startup timing).
