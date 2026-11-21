# Slate API on Fly.io

Run commands from the repository root. The Fly app is `slatemc-api` in the personal organization,
with its primary Machine in Dallas (`dfw`). Its public origin is `https://api.slatemc.app`.

```powershell
fly auth login
fly config validate
cargo test --locked -p slate-modpack-api -p slate-modpack-client
fly deploy --remote-only --ha=false
fly status -a slatemc-api
fly checks list -a slatemc-api
```

The Dockerfile builds the locked Rust workspace API package on Linux and runs it as an unprivileged
user. Fly terminates TLS and forwards HTTP to `0.0.0.0:8080`. One 512 MiB shared-CPU Machine stays
running; deploying with `--ha=false` avoids creating a second Machine by default. Caches are
in-memory and disposable, so this deployment needs no database or persistent volume.

## Cloudflare DNS

Use DNS-only records (gray cloud) for the API so requests go directly to Fly:

| Type | Name | Content |
| --- | --- | --- |
| A | api | 66.241.124.7 |
| AAAA | api | 2a09:8280:1::218:7b05:0 |

The hostname is already attached to Fly. Check automatic certificate issuance with:

```powershell
fly certs check api.slatemc.app -a slatemc-api
fly certs show api.slatemc.app -a slatemc-api
```

## Verify the public API

```powershell
./tools/smoke-api.ps1
```

`/health` checks the process without depending on providers; Fly uses it for routing. `/ready`
reports provider health. The smoke check also exercises provider discovery, Minecraft metadata,
modpack search, mod search, and launcher configuration through HTTPS.

## Optional services

Modpack browsing and installation do not require Sentry, PostHog, OpenTelemetry, or support-report
object storage. Leave those variables unset until their services exist. API logs go to Fly logs.
Without storage, launcher configuration disables support-report delivery. Telemetry events return
`accepted: false` when PostHog is unconfigured. Signed launcher updates require an actual published
release manifest; desktop release signing remains a separate release step.

`modpack-api.production.env.example`, `validate-production-services.ps1`, and the Production
readiness workflow cover the optional full operational stack. They are stricter than the core
API smoke check. Supply real secrets using `fly secrets set`; never deploy example secret values.

`SLATE_TRUST_FLY_PROXY=true` makes rate limits use Fly's `Fly-Client-IP` header. Enable it only
behind Fly Proxy. A local or direct deployment defaults to the socket peer address.

## Launcher installers and updates on R2

The public `slate-releases` bucket is in Cloudflare account
`d00075dfb63355bf04572756c74478c8`, served at `https://downloads.slatemc.app` with TLS 1.2 or newer.
It contains public release files only. Private support reports must use a separate private bucket.

| Object | Purpose | Cache policy |
| --- | --- | --- |
| `releases/<version>/<filename>` | Installers, updater bundles, and signatures | Immutable for one year |
| `releases/<version>/latest.json` | Versioned updater manifest | Immutable for one year |
| `channels/stable/latest.json` | Current stable updater manifest | Revalidate on every request |
| `channels/beta/latest.json` | Current beta updater manifest | Revalidate on every request |
| `channels/<channel>/downloads.json` | Installer URLs, sizes, and SHA-256 checksums | Revalidate on every request |
| `keys/updater.pub` | Public updater verification key | Public; embedded in release builds |

The launcher checks the Fly API, which reads the matching R2 channel manifest and returns the
signed artifact URL on the download domain. Missing channel manifests mean no release has been
published yet; the API returns HTTP 204. R2 upload credentials are never needed by users or Fly.

The Release desktop workflow builds all four platform targets into a GitHub draft, then downloads
the completed draft's artifacts and publishes them to R2. GitHub is build staging; the public
installer and updater URLs point to R2. It validates all signatures against the generated manifest,
checks immutable object hashes, uploads and verifies all versioned files, then advances the channel
manifest last. Existing version objects with different content cannot be overwritten. Increment
the version before rebuilding a published release.

GitHub Actions repository secrets:

- `SLATE_R2_ACCESS_KEY_ID` and `SLATE_R2_SECRET_ACCESS_KEY`: object read/write for this bucket only.
- `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: encrypted updater key.

Repository variables:

- `SLATE_R2_ENDPOINT`: `https://d00075dfb63355bf04572756c74478c8.r2.cloudflarestorage.com`
- `TAURI_UPDATER_PUBLIC_KEY`: the contents of `ops/updater.pub`.

The first signing key also has a user-protected encrypted local backup in
`%LOCALAPPDATA%\Slate\release-signing`; its password backup uses Windows DPAPI for the current user.
Preserve this signing identity for future releases so installed launchers can verify updates.

```powershell
node --test tools/publish-launcher-r2.test.mjs
gh workflow run release-desktop.yml -f channel=stable -f release_notes='Release notes here'
```

If the files built successfully but publishing needs a retry, use the same workflow with
`-f publish_only=true` to reuse the completed signed draft. This does not rebuild or replace
the versioned binaries.

Use the logged-in `cf` CLI for Cloudflare management. The checked-in `r2-release-policy.json`
documents the bucket-scoped token permissions and `r2-cors.json` documents the download CORS policy.

## Custom client paused

Normal desktop builds have no default Cargo features and exclude `slate-client`. The release
workflow no longer builds Kotlin artifacts, and Tauri does not bundle their directory. Old
custom-client instances stay on disk but receive an explicit disabled message when installed or
launched. Create a Modded instance for ordinary Fabric or NeoForge play. The experimental code
can be deliberately compiled with `--features slate-client` during future development.
