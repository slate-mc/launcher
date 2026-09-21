# Native installation acceptance

The installer smoke executable exercises the same metadata, download, verification, managed Java,
loader, modpack-content, and launch-planning code used by the desktop application. It refuses to use
a non-empty root so every run can start from clean application and storage directories.

## Base game matrix

Run this on every supported operating system and CPU architecture:

```powershell
$acceptanceRoot = Join-Path $env:TEMP 'slate-acceptance-1.21.1'
cargo run -p slate-installer --example install_smoke -- $acceptanceRoot 1.21.1 auto auto
```

Use a new path for each run. `auto` resolves the current recommended compatible Fabric and NeoForge
versions, then the executable installs and verifies Vanilla, Fabric, and NeoForge in separate
instances while sharing only slate's verified artifact cache.

## Exact modpack release

Start the local content API, then append an exact provider, project, and version identity:

```powershell
$acceptanceRoot = Join-Path $env:TEMP 'slate-acceptance-pack'
cargo run -p slate-installer --example install_smoke -- `
  $acceptanceRoot 1.21.1 auto auto `
  http://127.0.0.1:8080 curseforge PROJECT_ID VERSION_ID
```

Acceptance runs must pin an exact modpack version. This keeps failures reproducible even when a
provider publishes a new release.

The executable writes `acceptance-result.json` under the selected root after every completed target.
The report remains marked incomplete until the full requested matrix succeeds, so CI and release
operators can distinguish a partial run from a passing run.

## Passing criteria

For each target, the executable must finish with a zero exit code after it:

- installs the compatible managed Java runtime under the clean root;
- downloads or reuses only verified Minecraft, loader, library, asset, and native artifacts;
- loads the committed installed-revision manifest and verifies its digest;
- resolves a complete launch plan and confirms every required launch artifact exists;
- verifies the installed launch artifacts against the committed manifest; and
- for a modpack, downloads and verifies its resolved content plan and confirms content reached the
  instance.

This harness does not sign into Microsoft or open a game window. Authenticated process launch remains
a desktop acceptance step because it requires an owned Minecraft account and interactive consent.

## Recorded Windows evidence

On September 20, 2026, the clean-root base matrix passed for Minecraft 1.21.1 with automatically
resolved Fabric and NeoForge versions. Vanilla verified 73 launch artifacts, Fabric verified 81,
and NeoForge verified 109 on managed Java 21.

The same harness then passed CurseForge project `925200`, exact version `8764211` (All the Mods 10
8.1): the API resolved NeoForge `21.1.249`, 492 downloads totaling 1,607,320,308 bytes, two extract
actions, and the installer committed 3,935 content files before verifying all 109 launch artifacts.

Installer tests also force pending content transactions through both rollback and commit paths.
Interrupted downloads remove their partial file on task cancellation, and content applied to the
game directory automatically restores the previous files unless the desktop database commit
succeeds.
