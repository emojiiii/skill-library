import { createRootRoute, createRoute, createRouter } from "@tanstack/react-router";
import { RootLayout } from "./shell/RootLayout";
import { WorkspaceShell } from "./shell/WorkspaceShell";
import { WorkspaceSkillsRoute } from "./routes/WorkspaceSkillsRoute";
import { WorkspacePublishRoute } from "./routes/WorkspacePublishRoute";
import { WorkspaceInvitationsRoute } from "./routes/WorkspaceInvitationsRoute";
import { WorkspaceActivityRoute } from "./routes/WorkspaceActivityRoute";
import { SubscriptionsRoute } from "./routes/SubscriptionsRoute";
import { InstalledRoute } from "./routes/InstalledRoute";
import { CliRoute } from "./routes/CliRoute";

const rootRoute = createRootRoute({
  component: RootLayout,
});

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: () => null, // Auto-redirects to first workspace in RootLayout
});

// Workspace layout route: /workspace/$owner/$repo
const workspaceRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "workspace/$owner/$repo",
  component: WorkspaceShell,
});

// Workspace sub-pages
const workspaceIndexRoute = createRoute({
  getParentRoute: () => workspaceRoute,
  path: "/",
  component: () => {
    const { owner, repo } = workspaceIndexRoute.useParams();
    return <WorkspaceSkillsRoute key={`${owner}/${repo}`} />;
  },
});

const publishRoute = createRoute({
  getParentRoute: () => workspaceRoute,
  path: "publish",
  component: WorkspacePublishRoute,
});

const invitationsRoute = createRoute({
  getParentRoute: () => workspaceRoute,
  path: "invitations",
  component: WorkspaceInvitationsRoute,
});

const activityRoute = createRoute({
  getParentRoute: () => workspaceRoute,
  path: "activity",
  component: WorkspaceActivityRoute,
});

// Personal (top-level) routes
const subscriptionsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "subscriptions",
  component: SubscriptionsRoute,
});

const installedRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "installed",
  component: InstalledRoute,
});

const cliRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "cli",
  component: CliRoute,
});

const routeTree = rootRoute.addChildren([
  indexRoute,
  workspaceRoute.addChildren([
    workspaceIndexRoute,
    publishRoute,
    invitationsRoute,
    activityRoute,
  ]),
  subscriptionsRoute,
  installedRoute,
  cliRoute,
]);

export const router = createRouter({ routeTree });

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
