// App.tsx is now a thin re-export file.
// The actual root layout lives in ./shell/RootLayout.tsx and is wired via the router.

export { formatError } from "./utils/format";
export { pageCopy, navRoutes, routeToPage } from "./utils/navigation";

// Re-export RootLayout as App for backward compat with any entry point that imports { App }
export { RootLayout as App } from "./shell/RootLayout";
