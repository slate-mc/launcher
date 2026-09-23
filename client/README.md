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

Run every required quality gate from the repository root:

```powershell
./gradlew clientCheck
```

Run benchmarks separately so ordinary verification stays fast:

```powershell
./gradlew :client-benchmarks:jmh
```
