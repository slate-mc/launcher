# Compatibility policy

Compatibility is resolved and enforced by Rust. The renderer displays catalog results but never
guesses whether a Minecraft, loader, Java, mod, pack, or server combination is valid.

## Supported launch targets

Fresh installation and launch planning support Vanilla, Fabric, and NeoForge. Forge and Quilt are
represented in provider data so upstream metadata is not misclassified, but the launcher rejects
them at the install boundary until their installers are implemented.

Minecraft versions come from the Mojang catalog. Fabric and NeoForge versions come from their
official metadata and are filtered by the exact selected Minecraft version. Loader version is
required for modded instances and absent for Vanilla.

## Java selection

| Minecraft target            | Java major |
| --------------------------- | ---------- |
| `1.16.x` and older          | 8          |
| `1.17.x`                    | 16         |
| `1.18.x` through `1.20.4`   | 17         |
| `1.20.5` and later `1.x`    | 21         |
| Year-based `26.x` and later | 25         |

The Mojang version metadata remains authoritative during base-game installation. The table is used
for provider install-plan validation and custom-Java preflight. Managed runtimes live under the
slate storage root by major/platform/architecture/release and are probed before use. A detected or
custom Java executable must match the required major and architecture.

## Content target locking

Mod and content discovery sends the instance's exact Minecraft version, loader kind, and loader
version. Before filesystem changes, the desktop verifies install-plan schema 1 and requires:

- exact Minecraft version equality;
- exact loader family equality;
- exact loader version equality, including `none` for Vanilla;
- exact required Java major equality;
- safe unique relative destinations and cryptographic hashes.

Required dependencies are resolved transitively. An already installed exact dependency is reused;
a mismatched installed dependency fails safely rather than producing duplicate JARs. Server-only
files are omitted from client plans. Optional files are installed only when selected or explicitly
defaulted by the provider.

## Servers

Server protocol information is guidance, not a launch gate. The UI compares a server's reported
protocol/range with available instances, warns when a target appears incompatible, and still lets
the player select an instance. Networks may accept protocol versions beyond the value returned by
their current status response, so slate does not invent or force a compatibility override.

## Change policy

Compatibility logic belongs in isolated Rust functions with boundary tests. A new loader or Java
rule must update provider normalization, install-plan validation, catalog filtering, settings
preflight, smoke coverage, and this document in the same change.
