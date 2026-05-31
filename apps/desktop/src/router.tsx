import { createRootRoute, createRoute, createRouter } from "@tanstack/react-router";
import { RootLayout } from "./shell/RootLayout";
import { WorkspaceShell } from "./shell/WorkspaceShell";
import { WorkspaceSkillsRoute } from "./routes/WorkspaceSkillsRoute";
import { WorkspacePublishRoute } from "./routes/WorkspacePublishRoute";
import { WorkspaceInvitationsRoute } from "./routes/WorkspaceInvitationsRoute";
import { WorkspaceActivityRoute } from "./routes/WorkspaceActivityRoute";
import { SubscriptionsRoute } from "./routes/SubscriptionsRoute";
import { CliRoute } from "./routes/CliRoute";
import { DiscoverRoute } from "./routes/DiscoverRoute";
import { MySkillsPage } from "./pages/MySkillsPage";

const rootRoute = createRootRoute({
  component: RootLayout,
});

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: () => null, // Auto-redirects to /discover in RootLayout
});

// Consumer-layer (anonymous) top-level routes
const discoverRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "discover",
  component: DiscoverRoute,
});

const mySkillsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "my-skills",
  component: MySkillsPage,
});

// Workspace layout: a PATHLESS layout route. The active workspace is a global
// selection (appStore.selectedWorkspace), NOT part of the URL — this is a
// desktop app, so there's no benefit to encoding it in the path and doing so
// caused the selection to reset on every personal page. WorkspaceShell provides
// workspace context (and remounts children on workspace switch) for all four
// workspace-scoped pages below.
const workspaceLayoutRoute = createRoute({
  getParentRoute: () => rootRoute,
  id: "workspace-layout",
  component: WorkspaceShell,
});

const skillsRoute = createRoute({
  getParentRoute: () => workspaceLayoutRoute,
  path: "skills",
  component: WorkspaceSkillsRoute,
});

const publishRoute = createRoute({
  getParentRoute: () => workspaceLayoutRoute,
  path: "publish",
  component: WorkspacePublishRoute,
});

const membersRoute = createRoute({
  getParentRoute: () => workspaceLayoutRoute,
  path: "members",
  component: WorkspaceInvitationsRoute,
});

const activityRoute = createRoute({
  getParentRoute: () => workspaceLayoutRoute,
  path: "activity",
  component: WorkspaceActivityRoute,
});

// Personal (top-level) routes
const subscriptionsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "subscriptions",
  component: SubscriptionsRoute,
});

const cliRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "cli",
  component: CliRoute,
});

const routeTree = rootRoute.addChildren([
  indexRoute,
  discoverRoute,
  mySkillsRoute,
  workspaceLayoutRoute.addChildren([
    skillsRoute,
    publishRoute,
    membersRoute,
    activityRoute,
  ]),
  subscriptionsRoute,
  cliRoute,
]);

export const router = createRouter({ routeTree });

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
