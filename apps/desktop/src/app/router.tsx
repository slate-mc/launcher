import {
  createRootRoute,
  createRoute,
  createRouter,
  lazyRouteComponent,
  redirect,
} from "@tanstack/react-router";
import { AppShell } from "../components/AppShell";
import { RoutePlaceholder } from "../components/RoutePlaceholder";

const HomePage = lazyRouteComponent(
  () => import("../features/home/HomePage"),
  "HomePage",
);
const DiscoverPage = lazyRouteComponent(
  () => import("../features/discover/DiscoverPages"),
  "DiscoverPage",
);
const ModpackDetailPage = lazyRouteComponent(
  () => import("../features/discover/DiscoverPages"),
  "ModpackDetailPage",
);
const InstanceContentPage = lazyRouteComponent(
  () => import("../features/instances/InstancePages"),
  "InstanceContentPage",
);
const InstanceOverviewPage = lazyRouteComponent(
  () => import("../features/instances/InstancePages"),
  "InstanceOverviewPage",
);
const InstanceSettingsPage = lazyRouteComponent(
  () => import("../features/instances/InstancePages"),
  "InstanceSettingsPage",
);
const LibraryPage = lazyRouteComponent(
  () => import("../features/library/LibraryPage"),
  "LibraryPage",
);
const NewInstancePage = lazyRouteComponent(
  () => import("../features/library/NewInstancePage"),
  "NewInstancePage",
);
const OnboardingPage = lazyRouteComponent(
  () => import("../features/onboarding/OnboardingPage"),
  "OnboardingPage",
);
const SettingsPage = lazyRouteComponent(
  () => import("../features/settings/SettingsPage"),
  "SettingsPage",
);
const StorageSettingsPage = lazyRouteComponent(
  () => import("../features/settings/StorageSettingsPage"),
  "StorageSettingsPage",
);
const PrivacySettingsPage = lazyRouteComponent(
  () => import("../features/settings/PrivacySettingsPage"),
  "PrivacySettingsPage",
);
const ServersPage = lazyRouteComponent(
  () => import("../features/servers/ServersPage"),
  "ServersPage",
);
const AccountsPage = lazyRouteComponent(
  () => import("../features/system/SystemPages"),
  "AccountsPage",
);
const ActivityPage = lazyRouteComponent(
  () => import("../features/system/SystemPages"),
  "ActivityPage",
);
const DownloadsPage = lazyRouteComponent(
  () => import("../features/system/SystemPages"),
  "DownloadsPage",
);
const HelpPage = lazyRouteComponent(
  () => import("../features/system/SystemPages"),
  "HelpPage",
);

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
const privacySettingsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/settings/privacy",
  component: PrivacySettingsPage,
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
  privacySettingsRoute,
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
