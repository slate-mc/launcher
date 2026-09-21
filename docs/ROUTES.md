# Route contract

This document records the stable navigation and HTTP route surfaces implemented by slate. Route
names are product contracts: links may be bookmarked, reopened after an update, and referenced by
diagnostic actions.

## Desktop routes

| Route                             | Surface              | Notes                                                                        |
| --------------------------------- | -------------------- | ---------------------------------------------------------------------------- |
| `/`                               | Redirect             | Redirects to `/home`.                                                        |
| `/home`                           | Home                 | Continue playing, instance switcher, library summary, servers, and activity. |
| `/onboarding`                     | First run            | Account, storage, Java, and first-instance flow.                             |
| `/library`                        | Instance library     | Filtering, import, and instance management.                                  |
| `/library/new`                    | New instance         | Vanilla, Fabric, and NeoForge creation with catalog-backed versions.         |
| `/instances/:instanceId/overview` | Instance overview    | Install, launch, stop, update, session history, logs, and trash.             |
| `/instances/:instanceId/content`  | Instance content     | Mods, resource packs, shader packs, and data packs.                          |
| `/instances/:instanceId/settings` | Instance settings    | Profile, launch, Java, performance, game, and lifecycle settings.            |
| `/discover`                       | Modpack discovery    | Normalized CurseForge, Modrinth, and FTB search.                             |
| `/discover/:provider/:projectId`  | Modpack detail       | Description, versions, runtime requirements, and installation.               |
| `/servers`                        | Saved servers        | Status, MOTD formatting, compatible-instance selection, and quick join.      |
| `/activity`                       | Running games        | Active supervised Minecraft processes.                                       |
| `/downloads`                      | Installation queue   | Durable install state, ordering, pause, cancel, retry, and progress.         |
| `/accounts`                       | Minecraft accounts   | Microsoft sign-in, refresh, default selection, and removal.                  |
| `/settings/general`               | Launcher preferences | Theme, motion, updates, and download controls.                               |
| `/settings/storage`               | Storage              | Cache cleanup, retained trash, restore, and permanent deletion.              |
| `/settings/privacy`               | Privacy              | Separate product-analytics and crash-sharing consent.                        |
| `/help`                           | Support              | Recovery links and reviewed support-report submission/export.                |

Unknown routes render the in-app not-found surface. Instance routes resolve the ID through Rust;
missing or trashed instances never fall back to arbitrary filesystem paths.

## Navigation rules

- The top workspace bar owns Home, Library, Discover, and Servers. Downloads, Activity, Settings,
  and Accounts are global utilities.
- Instance tabs preserve `instanceId` and switch only the final route segment.
- Crash-assistant actions may link only to Content, Settings, or Accounts.
- Preview mode uses the same routes and DTO validation as native mode, but labels sample data and
  does not claim to launch, install, or authenticate.
- New routes must be registered in `apps/desktop/src/app/router.tsx` and added to the route-level
  accessibility suite.

## Service routes

`/health` and `/ready` are unversioned operational probes. Public product routes are under `/v1`.
The complete request and response contract is in [API.md](API.md).
