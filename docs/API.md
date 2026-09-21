# Public content API

The content service is the provider boundary for the launcher. Development uses
`http://127.0.0.1:8080`; production uses `https://api.slatelauncher.org`. The desktop never embeds
provider credentials or parses provider-native manifests.

## Envelope

Every JSON success response uses:

```json
{
  "success": true,
  "data": {},
  "error": null,
  "meta": {
    "requestId": "generated UUID",
    "traceId": "generated 32-character trace ID",
    "spanId": "generated 16-character span ID",
    "timestamp": "RFC3339 UTC timestamp",
    "durationMs": 0.0
  }
}
```

Errors set `success` to false, `data` to null, and include a stable code, safe message,
`retryable`, and optional field-level details. The service returns `x-request-id` and W3C
`traceparent` headers. Raw upstream, Reqwest, storage, and credential details are never returned.

## Routes

| Method | Route                                                                | Purpose                                                   |
| ------ | -------------------------------------------------------------------- | --------------------------------------------------------- |
| GET    | `/health`                                                            | Process liveness only.                                    |
| GET    | `/ready`                                                             | Internal readiness plus degraded provider status.         |
| GET    | `/v1/providers`                                                      | Provider availability.                                    |
| GET    | `/v1/modpacks`                                                       | Multi-provider search.                                    |
| GET    | `/v1/modpacks/:provider/:projectId`                                  | Normalized project detail.                                |
| GET    | `/v1/modpacks/:provider/:projectId/versions`                         | Filtered version list.                                    |
| GET    | `/v1/modpacks/:provider/:projectId/versions/:versionId`              | Resolved version manifest.                                |
| POST   | `/v1/modpacks/:provider/:projectId/versions/:versionId/install-plan` | Deterministic client install plan.                        |
| GET    | `/v1/modpacks/:provider/:projectId/update`                           | Same-target update check.                                 |
| GET    | `/v1/modpacks/categories`                                            | Normalized categories.                                    |
| GET    | `/v1/minecraft/versions`                                             | Release catalog.                                          |
| GET    | `/v1/loaders`                                                        | Loader kinds and versions.                                |
| GET    | `/v1/mods`                                                           | Exact-target CurseForge/Modrinth mod search.              |
| POST   | `/v1/mods/resolve`                                                   | Resolve provider references for installed-file display.   |
| GET    | `/v1/mods/:provider/:projectId/versions`                             | Compatible mod versions.                                  |
| POST   | `/v1/mods/:provider/:projectId/install-plan`                         | Mod plus required-dependency plan.                        |
| GET    | `/v1/content/:kind`                                                  | Modrinth resource-pack, shader-pack, or data-pack search. |
| GET    | `/v1/content/:kind/modrinth/:projectId/versions`                     | Compatible content versions.                              |
| POST   | `/v1/content/:kind/modrinth/:projectId/install-plan`                 | Content installation plan.                                |
| POST   | `/v1/import-plan`                                                    | Resolve a CurseForge ZIP or Modrinth `.mrpack` manifest.  |
| GET    | `/v1/launcher/updates/:target/:architecture/:currentVersion`         | Signed staged update response.                            |
| GET    | `/v1/launcher/config`                                                | Cached emergency switches and rollout configuration.      |
| POST   | `/v1/telemetry/events`                                               | Allowlisted anonymous product-event relay.                |
| POST   | `/v1/support/reports`                                                | Bounded reviewed support archive upload.                  |

## Search and pagination

Search accepts `q`, `provider`, `minecraft_version`, `loader`, `category`, `sort`, `cursor`,
`page`, and `limit` where applicable. `limit` defaults to 20 and is capped at 50; pages are capped
at 10,000. A request may use either the opaque cursor or `page`, never both. Multi-provider search
runs providers concurrently, preserves provider-qualified identity, and reports each provider as
`ok` or `unavailable`; one provider failure does not discard other results.

## Trust and caching

- IDs and query values are bounded before reaching adapters.
- Install paths are normalized relative paths; traversal, roots, drive prefixes, and duplicate
  destinations are rejected.
- Every download needs a cryptographic hash. The launcher independently validates paths, target
  compatibility, size, and hash before commit.
- Read metadata uses short public caching. Resolved manifests use longer public caching. Install
  plans, resolution, telemetry, support, and errors use `no-store`.
- Upstream calls have connection/request timeouts, bounded transient retries, host-aware redirect
  checks, rate limits, and memory caches.

The Rust types in `crates/modpack-api-contracts` are the canonical schema. Provider adapters absorb
upstream changes; `/v1` field or enum changes require compatibility review.
