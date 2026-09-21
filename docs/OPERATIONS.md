# Operations

## Modpack API observability

The API always emits structured JSON logs to standard output. Remote OpenTelemetry export is off
by default and must be enabled explicitly in the server environment. The desktop launcher never
receives the collector endpoint or its credentials.

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
