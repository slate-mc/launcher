import {
  createRootRoute,
  createRoute,
  createRouter,
  redirect,
} from "@tanstack/react-router";
import { AppShell } from "../components/AppShell";
import { RoutePlaceholder } from "../components/RoutePlaceholder";
import { HomePage } from "../features/home/HomePage";
import {
  DiscoverPage,
  ModpackDetailPage,
} from "../features/discover/DiscoverPages";
import {
  InstanceContentPage,
  InstanceOverviewPage,
  InstanceSettingsPage,
} from "../features/instances/InstancePages";
import { LibraryPage } from "../features/library/LibraryPage";
import { NewInstancePage } from "../features/library/NewInstancePage";
import { OnboardingPage } from "../features/onboarding/OnboardingPage";
import { SettingsPage } from "../features/settings/SettingsPage";
import { StorageSettingsPage } from "../features/settings/StorageSettingsPage";
import { ServersPage } from "../features/servers/ServersPage";
import {
  AccountsPage,
  ActivityPage,
  DownloadsPage,
  HelpPage,
} from "../features/system/SystemPages";

const rootRoute = createRootRoute({
  component: AppShell,
  notFoundComponent: () => (
    <RoutePlaceholder
      eyebrow="Not found"
      title="That page is not here."
      description="The address may be stale. Return home to keep managing your setup."
    />
  ),
});

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  beforeLoad: () => {
    throw redirect({ to: "/home" });
  },
});

const homeRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/home",
  component: HomePage,
});
const onboardingRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/onboarding",
  component: OnboardingPage,
});
const libraryRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/library",
  component: LibraryPage,
});
const newInstanceRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/library/new",
  component: NewInstancePage,
});
const instanceOverviewRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/instances/$instanceId/overview",
  component: InstanceOverviewPage,
});
const instanceContentRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/instances/$instanceId/content",
  component: InstanceContentPage,
});
const instanceSettingsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/instances/$instanceId/settings",
  component: InstanceSettingsPage,
});
const discoverRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/discover",
  component: DiscoverPage,
});
const modpackDetailRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/discover/$provider/$projectId",
  component: ModpackDetailPage,
});
const serversRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/servers",
  component: ServersPage,
});
const activityRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/activity",
  component: ActivityPage,
});
const downloadsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/downloads",
  component: DownloadsPage,
});
const accountsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/accounts",
  component: AccountsPage,
});
const settingsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/settings/general",
  component: SettingsPage,
});
const storageSettingsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/settings/storage",
  component: StorageSettingsPage,
});
const helpRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/help",
  component: HelpPage,
});

const routeTree = rootRoute.addChildren([
  indexRoute,
  onboardingRoute,
  homeRoute,
  libraryRoute,
  newInstanceRoute,
  instanceOverviewRoute,
  instanceContentRoute,
  instanceSettingsRoute,
  discoverRoute,
  modpackDetailRoute,
  serversRoute,
  downloadsRoute,
  activityRoute,
  accountsRoute,
  settingsRoute,
  storageSettingsRoute,
  helpRoute,
]);

export const router = createRouter({
  routeTree,
  defaultPreload: "intent",
  scrollRestoration: true,
});

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
