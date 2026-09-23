# Slate Client

Slate Client is slate's full Kotlin in-game client platform. It is not limited to PvP: HUD,
performance, accessibility, visual and quality-of-life tools, profile presets, diagnostics, and
future creator features all use the same verified module runtime. Shared projects contain no
Minecraft implementation classes. Loader and Minecraft-version hooks belong only in adapter
projects and must pass the native compatibility matrix before the launcher exposes them.

Projects:

- `api`: stable module, target, lifecycle, and handshake contracts
- `runtime`: dependency ordering and lifecycle supervision
- `config`: revision-safe configuration reconciliation
- `hud`: deterministic layout and editor history
- `diagnostics`: bounded, sanitized module evidence
- `modules/*`: performance, accessibility, visual, QoL, and PvP module families
- `profiles`: validated presets spanning every module family
- `adapters/fabric` and `adapters/neoforge`: isolated loader boundaries
- `benchmarks`: JMH performance gates for hot paths

The Fabric adapter currently produces a remapped client mod for the exact target Minecraft 1.21.1
and Fabric Loader 0.19.5. It boots the shared module supervisor and writes an atomic, process-bound
handshake under the game directory. That handshake deliberately remains `contract_only` until a
real game run and launcher-side validation pass. NeoForge remains a pure contract project.

Run every required quality gate from the repository root:

```powershell
./gradlew clientCheck
```

Run benchmarks separately so ordinary verification stays fast:

```powershell
./gradlew :client-benchmarks:jmh
```

Build the exact Fabric artifact with:

```powershell
./gradlew :client-adapter-fabric:build
```
