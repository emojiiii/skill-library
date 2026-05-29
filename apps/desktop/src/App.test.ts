import { describe, expect, it } from "vitest";
import { navRoutes, pageCopy, routeToPage } from "./utils/navigation";
import { formatError } from "./utils/format";

describe("desktop routed management pages", () => {
  it("maps routed URLs to the correct workbench page", () => {
    expect(routeToPage("/")).toBe("workspaces");
    expect(routeToPage("/workspace/acme/team-skills")).toBe("workspaces");
    expect(routeToPage("/workspace/acme/team-skills/publish")).toBe("publish");
    expect(routeToPage("/workspace/acme/team-skills/invitations")).toBe("invitations");
    expect(routeToPage("/workspace/acme/team-skills/activity")).toBe("activity");
    expect(routeToPage("/subscriptions")).toBe("subscriptions");
    expect(routeToPage("/installed")).toBe("installed");
    expect(routeToPage("/cli")).toBe("cli");
  });

  it("keeps the sidebar route contract aligned with page copy", () => {
    for (const route of navRoutes) {
      expect(pageCopy[route.page].title).toBeTruthy();
      expect(pageCopy[route.page].subtitle).toBeTruthy();
    }
    expect(pageCopy.invitations.subtitle).toContain("collaborators");
  });

  it("navRoutes covers all workspace-scoped and personal pages", () => {
    const pages = navRoutes.map((r) => r.page);
    expect(pages).toContain("workspaces");
    expect(pages).toContain("publish");
    expect(pages).toContain("invitations");
    expect(pages).toContain("activity");
    expect(pages).toContain("subscriptions");
    expect(pages).toContain("installed");
    expect(pages).toContain("cli");
  });

  it("formats structured command and API errors", () => {
    expect(formatError({ code: "missing_github_token", message: "GitHub token is required" })).toBe(
      "missing_github_token: GitHub token is required",
    );
    expect(
      formatError({
        ok: false,
        error: { code: "invalid_request", message: "The request body is invalid." },
      }),
    ).toBe("invalid_request: The request body is invalid.");
    expect(formatError(new Error("network unavailable"))).toBe("network unavailable");
  });
});
