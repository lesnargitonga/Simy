# Deployment Baseline

## Minimum Components

- Relay service instances behind TLS termination
- PostgreSQL with encrypted storage and restricted network access
- Redis for replay tokens and throttle counters
- Secret management for runtime configuration
- Metrics and logs with aggressive redaction

## Security Controls

- TLS 1.3 only at the edge
- Minimal network exposure between relay, PostgreSQL, and Redis
- Separate credentials per environment
- Automated patching for base images and runtime dependencies
- Signed build artifacts and reproducible release process where possible

## Logging Policy

- Do not log plaintext content
- Do not log full mailbox tokens or bearer credentials
- Avoid durable IP retention unless there is a documented abuse-response requirement
- Prefer aggregated metrics to request-level logs

## Rollout Sequence

1. Stand up isolated staging infrastructure.
2. Run schema migrations and health checks.
3. Run integration tests against staging PostgreSQL and Redis.
4. Review log output for secret leakage.
5. Perform external security review before production traffic.
