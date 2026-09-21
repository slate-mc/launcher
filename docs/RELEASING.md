# Desktop release procedure

slate desktop releases are signed, channel-aware, and staged. Publishing a GitHub artifact alone
does not make it available to clients; the release API manifest and rollout percentage control
distribution.

## Preconditions

1. Version, changelog, and channel are intentional. Stable versions must not contain SemVer
   prerelease identifiers; Beta may.
2. Run the full Rust, contracts, frontend, Playwright, and native acceptance gates documented in
   [TESTING.md](TESTING.md) and [NATIVE_ACCEPTANCE.md](NATIVE_ACCEPTANCE.md).
3. Verify updater signing keys, public key, Sentry project settings, and source-map/debug-file
   upload configuration in the release environment. Never print signing material.
4. Confirm production API readiness, object-storage lifecycle/access policy, and channel manifest
   destinations.

## Build and draft

Dispatch `.github/workflows/release-desktop.yml` with release notes and `stable` or `beta`. The
matrix builds Windows, Linux, macOS aarch64, and macOS x86_64 using Rust 1.95 and Node 24. The
workflow writes a release-only Tauri configuration, signs updater artifacts, creates a draft GitHub
release, uploads renderer source maps, and uploads native debug information when Sentry is
configured.

Inspect every draft artifact and signature before publication. Beta releases are GitHub
prereleases. Stable and Beta use independent manifest URLs.

## Publish and stage

1. Publish the reviewed GitHub release.
2. Publish the matching static signed manifest to the configured channel origin.
3. Start the release API rollout at a small percentage. Eligibility is deterministic from the
   anonymous local cohort, channel, and update version.
4. Verify update discovery, download, signature verification, installation, restart, and retained
   attempt history on each supported target.
5. Increase rollout only after crash rate, API latency, install outcomes, and support reports are
   healthy.

The release API rejects unsupported targets, missing signatures, credential-bearing/non-HTTPS
download URLs, stable prerelease manifests, and versions that do not advance the installed
version. Redirects are restricted to the configured origin and GitHub release hosts.

## Halt and recovery

- Set the affected channel rollout to zero to stop new offers without changing clients.
- Correct or remove a bad manifest before resuming. Do not reuse a version number for different
  bytes.
- Automatic downgrades are prohibited. Recovery uses a newer signed build unless a separately
  reviewed manual rollback procedure is approved.
- Preserve the failed release, symbols, logs, and request IDs long enough to investigate.
- Complete the release record with root cause, affected cohorts, remediation, and the test added to
  prevent recurrence.
