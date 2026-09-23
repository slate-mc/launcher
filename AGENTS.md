# Slate repository guidance

## Product boundary

Slate is a Minecraft launcher and a full in-game client platform in the same product category as
Lunar Client or Dawn Client. It is not a PvP-only client. Performance, HUD, accessibility, visual,
quality-of-life, profile, diagnostics, creator, and PvP capabilities share one verified module
system. PvP is one optional module family and preset.

## Kotlin client policy

- Write Slate Client code in Kotlin. Introduce Java only for a loader or tooling boundary that
  cannot be expressed safely in Kotlin.
- Follow Google Android Kotlin style: four-space indentation, explicit public visibility, no
  wildcard imports, and immutable values by default.
- Keep shared API, configuration, layout, diagnostics, and module logic loader-neutral. Isolate
  Minecraft/Fabric/NeoForge implementation code in exact-version adapter modules.
- Do not advertise an adapter as compatible until it compiles and runs against the exact Minecraft
  and loader version in acceptance tests.
- Run `.\gradlew.bat clientCheck` before committing Kotlin client changes. This enforces Spotless,
  Detekt, warning-free compilation, unit tests, and benchmark compilation.
- Slate Client owns the basic in-game interface shell: title and pause menus use Slate controls,
  and Right Shift opens the module control center. Keep these surfaces accessible, keyboard
  navigable, performant, and recognizably Slate without copying another client's assets.

Authoritative product and module boundaries live in `docs/PRODUCT.md`, `docs/CLIENT_MODULES.md`, and
`minecraft-client-launcher-spec.md`.
