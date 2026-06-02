import type { AiReviewFinding } from "./skill-library";
import type { ReviewCacheEntry } from "./workspaceCache";

export { formatRelativeTime } from "../utils/format";

export type ReviewVerdict = ReviewCacheEntry["verdict"];

export function isVerdict(value: unknown): value is ReviewVerdict {
  return value === "safe" || value === "caution" || value === "danger";
}

export function normalizeFindings(findings: unknown): AiReviewFinding[] {
  if (!Array.isArray(findings)) return [];
  return findings.flatMap((finding) => {
    if (!finding || typeof finding !== "object") return [];
    const item = finding as { severity?: string; detail?: string };
    if (!["info", "warning", "danger"].includes(item.severity ?? "")) return [];
    return [{ severity: item.severity as AiReviewFinding["severity"], detail: String(item.detail ?? "") }];
  });
}

/**
 * Parse the JSON stored in `.reviews/{skillId}.json`. Accepts both snake_case
 * (the on-disk wire format) and camelCase (defensive). Returns null when the
 * payload is missing a verdict or content hash. The returned entry is flagged
 * `synced: true` because it came from the shared repo.
 */
export function normalizeRemoteReview(raw: string): ReviewCacheEntry | null {
  try {
    const value = JSON.parse(raw) as {
      verdict?: string;
      summary?: string;
      findings?: AiReviewFinding[];
      content_hash?: string;
      contentHash?: string;
      reviewed_at?: string;
      reviewedAt?: string;
      reviewed_by?: string | null;
      reviewedBy?: string | null;
      model?: string;
    };
    if (!isVerdict(value.verdict)) return null;
    const contentHash = value.content_hash ?? value.contentHash ?? "";
    if (!contentHash) return null;
    return {
      verdict: value.verdict,
      summary: String(value.summary ?? ""),
      findings: normalizeFindings(value.findings),
      contentHash,
      reviewedAt: value.reviewed_at ?? value.reviewedAt ?? "",
      reviewedBy: value.reviewed_by ?? value.reviewedBy ?? null,
      model: String(value.model ?? ""),
      synced: true,
    };
  } catch {
    return null;
  }
}

/** Serialize a cache entry to the snake_case wire format stored in the repo. */
export function stringifyRemoteReview(entry: ReviewCacheEntry): string {
  return `${JSON.stringify(
    {
      content_hash: entry.contentHash,
      verdict: entry.verdict,
      summary: entry.summary,
      findings: entry.findings,
      reviewed_at: entry.reviewedAt,
      reviewed_by: entry.reviewedBy ?? null,
      model: entry.model,
    },
    null,
    2,
  )}\n`;
}

/**
 * React-query key holding a `{ skillId -> verdict }` map for a workspace,
 * warmed by the remote-review batch prefetch on workspace load. Cards read it
 * to render the "reviewed safe" badge without per-row network calls.
 */
export function reviewVerdictMapKey(workspace: string) {
  return ["review-verdict-map", workspace] as const;
}

export type ReviewVerdictMap = Record<string, ReviewVerdict>;
