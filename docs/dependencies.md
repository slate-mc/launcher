# Dependencies

Direct dependency versions were reviewed on September 20, 2026 and are exact-pinned in the
workspace manifests. `Cargo.lock` and `yarn.lock` are the authoritative transitive resolutions.

| Dependency | Version | Purpose | License |
| --- | ---: | --- | --- |
| Rust toolchain | 1.95.0 | Workspace compiler, formatter, and Clippy baseline | Apache-2.0 / MIT components |
| directories | 6.0.0 | Platform-standard local data root discovery | MIT OR Apache-2.0 |
| axum | 0.8.9 | Public content, release, telemetry, and support-report HTTP API | MIT |
| reqwest | 0.13.5 | Rust HTTP clients with Rustls transport | MIT OR Apache-2.0 |
| keyring | 4.2.0 | Operating-system credential-vault access | MIT OR Apache-2.0 |
| moka | 0.12.16 | Bounded in-memory API caching | MIT OR Apache-2.0 |
| object_store | 0.14.2 | Private S3-compatible support-report storage | Apache-2.0 |
| OpenTelemetry crates | 0.32.0-0.33.0 | API traces, metrics, and logs | Apache-2.0 |
| Sentry Rust | 0.49.2 | Consented native/API crash reporting | Apache-2.0 |
| schemars | 1.2.2 | Rust-owned JSON Schema generation | MIT |
| secrecy | 0.10.3 | Accidental secret exposure reduction and zeroization wrapper | Apache-2.0 OR MIT |
| serde | 1.0.229 | Typed serialization | MIT OR Apache-2.0 |
| serde_json | 1.0.151 | IPC/schema JSON | MIT OR Apache-2.0 |
| sqlx | 0.9.0 | SQLite migrations, pooling, and typed database access | MIT OR Apache-2.0 |
| thiserror | 2.0.20 | Typed library errors | MIT OR Apache-2.0 |
| time | 0.3.51 | UTC/RFC3339 record timestamps; compatible with the selected Tauri line | MIT OR Apache-2.0 |
| tokio | 1.53.1 | Async test/runtime foundation | MIT |
| tower / tower-http | 0.5.3 / 0.7.1 | HTTP middleware, CORS, compression, request IDs, tracing | MIT |
| zip | 8.3.0 | Bounded loader, modpack, import/export, and support archives | MIT |
| sysinfo | 0.39.6 | Local memory and runtime recommendations | MIT |
| uuid | 1.26.1 | Stable local entity IDs | Apache-2.0 OR MIT |
| proptest | 1.11.0 | Domain invariant/property tests | MIT OR Apache-2.0 |
| tempfile | 3.27.0 | Isolated filesystem/database tests | MIT OR Apache-2.0 |
| Tauri | 2.11.4 | Desktop window, lifecycle, and command composition | Apache-2.0 OR MIT |
| tauri-build | 2.6.3 | Tauri build-time configuration | Apache-2.0 OR MIT |
| Tauri single-instance plugin | 2.4.5 | Prevent concurrent launcher processes over one data root | Apache-2.0 OR MIT |
| React / React DOM | 19.3.0 | Desktop application composition | MIT |
| Vite | 8.3.0 | Desktop SPA development and build | MIT |
| TypeScript | 6.0.3 | Strict frontend type checking; compatible with the selected lint toolchain | Apache-2.0 |
| Tailwind CSS / Vite adapter | 4.3.3 | CSS-first tokens and utilities | MIT |
| TanStack Query | 5.102.8 | Async command-derived state | MIT |
| TanStack Router | 1.170.35 | Typed launcher routing | MIT |
| TanStack Virtual | 3.14.13 | Bounded DOM rendering for long Minecraft logs | MIT |
| Radix Alert Dialog | 1.1.23 | Accessible destructive and interruption confirmations | MIT |
| React Markdown | 10.1.0 | Provider markdown rendering | MIT |
| rehype-sanitize | 6.0.0 | Sanitization for untrusted provider descriptions | MIT |
| Zod | 4.6.2 | Runtime command-boundary validation | MIT |
| Lucide React | 1.45.0 | Consistent interface iconography | ISC |
| Sentry React | 10.75.0 | Consented renderer crash reporting | MIT |
| Playwright | 1.63.0 | Route, visual, responsive, and interaction checks | Apache-2.0 |
| axe-core Playwright | 4.13.0 | Automated WCAG A/AA checks | MPL-2.0 |
| Vitest | 5.0.0 | Renderer unit and component tests | MIT |

The public source license is intentionally not declared because it remains an explicit product
decision. Release review must regenerate third-party notices from both lockfiles, verify every
transitive license and bundled native artifact, and scan for vulnerable or yanked dependencies.
