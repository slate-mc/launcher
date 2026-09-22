# Operations

## Modpack API observability

The API always emits structured JSON logs to standard output. Remote OpenTelemetry export is off
by default and must be enabled explicitly in the server environment. The desktop launcher never
receives the collector endpoint or its credentials.

When `SLATE_DEPLOYMENT_ENVIRONMENT=production`, startup now fails unless Sentry, PostHog,
OpenTelemetry export, and private support-report storage are all configured. Run the safe preflight
before deployment:

```powershell
cargo run -p slate-modpack-api -- check-config
```

The command reports only configuration booleans. It never prints DSNs, tokens, collector headers,
or storage credentials. `ops/modpack-api.production.env.example` is the canonical variable
inventory. The manually dispatched `production-readiness.yml` workflow additionally checks the
deployed health/readiness routes, sends an anonymous PostHog validation event through the API, and
can upload a synthetic private support report. Operators can run the same drill with
`tools/validate-production-services.ps1`.

Required production variables:

```text
SLATE_OTEL_ENABLED=true
SLATE_DEPLOYMENT_ENVIRONMENT=production
SLATE_OTEL_SAMPLE_RATIO=0.1
OTEL_EXPORTER_OTLP_ENDPOINT=https://your-otlp-gateway.example/otlp
OTEL_EXPORTER_OTLP_HEADERS=Authorization=Basic%20REDACTED
```

`OTEL_EXPORTER_OTLP_HEADERS` is consumed by the exporter and must come from the deployment secret
store. Do not commit it, print it, or pass it to a desktop build. HTTPS is required for remote
collectors; plain HTTP is accepted only for a collector on `localhost`, `127.0.0.1`, or `::1`.

The exporter sends OTLP/HTTP protobuf traces, metrics, and logs. It attaches:

- `service.name=slate-modpack-api`
- `service.version` from the Rust package version
- `deployment.environment.name` from `SLATE_DEPLOYMENT_ENVIRONMENT`

Current application metrics:

- `http_requests_total`
- `http_request_duration` in milliseconds
- `provider_requests_total`
- `provider_request_duration` in milliseconds
- `provider_errors_total`
- `cache_hits_total`
- `cache_misses_total`
- `install_plans_total`

HTTP metrics use normalized Axum route templates so project and version IDs do not create
unbounded metric labels. Provider metrics identify only the upstream service, not user or account
data. Local JSON logging stays active when remote export is enabled.

If the collector is unavailable, the API continues serving requests. The SDK bounds its export
queues and reports exporter failures through local logs. On an orderly shutdown, slate flushes the
metric, log, and trace providers.

## Recommended production alerts

- Elevated 5xx rate by normalized route
- Provider error rate or p95 provider latency above the upstream timeout budget
- API p95 latency by route
- Sustained cache miss spikes
- Install-plan failures or a sharp drop from the normal request baseline
- Missing telemetry from the production service

Sampling affects traces only. Metrics remain complete. Start with a 10% trace sample, retain errors
longer than successful requests, and review labels before adding any new high-cardinality field.

## Error reporting

The API enables Sentry only when `SLATE_SENTRY_DSN` is present. Desktop releases receive the public
ingest DSN at build time through `SLATE_DESKTOP_SENTRY_DSN` for native Rust and `VITE_SENTRY_DSN`
for the renderer. The release workflow expects:

```text
SLATE_DESKTOP_SENTRY_DSN     repository variable
SENTRY_ORG                   repository variable
SENTRY_DESKTOP_PROJECT       repository variable
SENTRY_AUTH_TOKEN            repository secret
```

The auth token is used only by the release build to upload browser source maps and native debug
information. It is never compiled into or shipped with slate. Source maps are deleted from the web
bundle after upload. Release binaries retain line-level native debug information for symbolication.

The API reports server failures automatically. Desktop reporting remains inactive until the user
turns on Share crash reports in Privacy settings. The native and renderer filters remove request
data, account identity, absolute paths, breadcrumb fields, local variables, instance metadata, and
secret-shaped values. A random installation ID is attached only after consent so Sentry can count
affected installations without receiving a Minecraft identity.

## Private support report storage

The API accepts sanitized ZIP reports at `POST /v1/support/reports`. Uploads are limited to 20 MiB
and 10 requests per minute per IP. The desktop sends a user-generated report ID with the archive;
the API stores it under a date-partitioned, collision-safe key and returns the same ID for support.

Set the bucket name in the server environment:

```text
SLATE_SUPPORT_REPORTS_BUCKET=slate-private-support
SLATE_SUPPORT_REPORTS_PREFIX=support-reports
AWS_DEFAULT_REGION=us-east-1
AWS_ACCESS_KEY_ID=from-the-deployment-secret-store
AWS_SECRET_ACCESS_KEY=from-the-deployment-secret-store
```

For an S3-compatible provider, also set `AWS_ENDPOINT_URL_S3` to its HTTPS endpoint. Storage
credentials belong only in the API deployment secret store and must never be passed to desktop
builds. Keep public access blocked, limit the API service identity to creating objects under the
configured prefix, give support operators a separate read role, enable encryption at rest, and
configure a bucket lifecycle rule that deletes reports after the support retention period.
Production readiness requires an upload, retrieval, expiry, and access-control drill against the
actual bucket.

Local S3 emulators may use a loopback HTTP endpoint with `AWS_ALLOW_HTTP=true`. The API rejects
non-loopback HTTP storage endpoints, and production deployments must not enable that override.

## Launcher update rollout

Stable and Beta are separate signed update channels. Stable uses
`SLATE_LAUNCHER_RELEASE_MANIFEST_URL`; Beta remains unavailable unless
`SLATE_LAUNCHER_BETA_RELEASE_MANIFEST_URL` is configured. Both URLs must use HTTPS. Control staged
availability with integer percentages from 0 through 100:

```text
SLATE_LAUNCHER_STABLE_ROLLOUT_PERCENT=10
SLATE_LAUNCHER_BETA_ROLLOUT_PERCENT=100
```

Each desktop installation generates a random update cohort ID. The API hashes the channel, target
version, and cohort so assignment is stable for a release without identifying a Minecraft account.
Clients outside the percentage receive `204 No Content`. Increase Stable gradually after reviewing
crash rate, launch success, and support volume. Set it to `0` to stop offering a release immediately.

The desktop updater always verifies the Tauri signature and refuses downgrades. Stable rejects
prerelease versions even if its configured manifest points to one. The launcher records the five
most recent update attempts locally; after restart it marks an attempt installed or interrupted so
the UI can explain that the previous version was preserved. A production rollout drill must cover
10%, 50%, 100%, emergency stop, invalid signature, interrupted installation, and Beta isolation.
