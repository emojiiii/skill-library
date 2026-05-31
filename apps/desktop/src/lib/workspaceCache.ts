import {
  dbCacheGet,
  dbCachePut,
  dbCacheDelete,
  dbCacheDeletePrefix,
  dbCacheStats,
  dbClearCache,
  remoteCachePutFile,
  remoteCacheGetFile,
  remoteCacheDeleteSkill,
  remoteCacheDeleteWorkspace,
  remoteCacheStats,
  type CachedFileResult,
  type RemoteCacheStat,
} from "./teamai";

const CACHE_PREFIX = "teamai-ws-cache:";
const TREE_PREFIX = "teamai-tree:";
const DETAIL_PREFIX = "teamai-detail:";

// ---------------------------------------------------------------------------
// Helpers: encode/decode JSON ↔ base64 for SQLite BLOB storage (metadata only)
// ---------------------------------------------------------------------------

function toBase64(obj: unknown): string {
  const json = JSON.stringify(obj);
  const bytes = new TextEncoder().encode(json);
  let binary = "";
  for (let i = 0; i < bytes.length; i++) {
    binary += String.fromCharCode(bytes[i]);
  }
  return btoa(binary);
}

function fromBase64<T>(b64: string): T {
  const binary = atob(b64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  const json = new TextDecoder().decode(bytes);
  return JSON.parse(json) as T;
}

// ---------------------------------------------------------------------------
// Workspace-level cache (HEAD SHA tracking) — stored in SQLite
// ---------------------------------------------------------------------------

export interface SkillCacheEntry {
  data: unknown;
  commitSha: string;
  fetchedAt: number;
}

export interface WorkspaceCache {
  workspace: string;
  headSha: string;
  branch: string;
  checkedAt: number;
  skills: Record<string, SkillCacheEntry>;
}

function cacheKey(workspace: string): string {
  return `${CACHE_PREFIX}${workspace}`;
}

export async function loadWorkspaceCache(workspace: string): Promise<WorkspaceCache | null> {
  try {
    const raw = await dbCacheGet(cacheKey(workspace));
    if (!raw) return null;
    return fromBase64<WorkspaceCache>(raw);
  } catch {
    return null;
  }
}

export async function saveWorkspaceCache(cache: WorkspaceCache): Promise<void> {
  try {
    await dbCachePut(cacheKey(cache.workspace), cache.workspace, toBase64(cache));
  } catch { /* non-critical */ }
}

export async function clearWorkspaceCache(workspace: string): Promise<void> {
  try { await dbCacheDelete(cacheKey(workspace)); } catch { /* ignore */ }
}

export async function getSkillFromCache(workspace: string, skillPath: string): Promise<SkillCacheEntry | null> {
  const cache = await loadWorkspaceCache(workspace);
  return cache?.skills[skillPath] ?? null;
}

export async function putSkillInCache(workspace: string, skillPath: string, entry: SkillCacheEntry, headSha?: string): Promise<void> {
  let cache = await loadWorkspaceCache(workspace);
  if (!cache) {
    cache = { workspace, headSha: headSha ?? "", branch: "", checkedAt: Date.now(), skills: {} };
  }
  cache.skills[skillPath] = entry;
  if (headSha) { cache.headSha = headSha; cache.checkedAt = Date.now(); }
  await saveWorkspaceCache(cache);
}

export async function invalidateSkillsInCache(workspace: string, skillPaths: string[]): Promise<void> {
  const cache = await loadWorkspaceCache(workspace);
  if (cache) {
    for (const path of skillPaths) {
      delete cache.skills[path];
    }
    await saveWorkspaceCache(cache);
  }
  // Also invalidate file trees and file contents for these skills
  for (const skillPath of skillPaths) {
    await clearFileTree(workspace, skillPath);
    await clearFilesForSkill(workspace, skillPath);
  }
}

export async function updateCacheHead(workspace: string, headSha: string, branch: string): Promise<void> {
  let cache = await loadWorkspaceCache(workspace);
  if (!cache) {
    cache = { workspace, headSha, branch, checkedAt: Date.now(), skills: {} };
  } else {
    cache.headSha = headSha;
    cache.branch = branch;
    cache.checkedAt = Date.now();
  }
  await saveWorkspaceCache(cache);
}

// ---------------------------------------------------------------------------
// File tree cache (per skill path) — stored in SQLite (small JSON metadata)
// ---------------------------------------------------------------------------

export interface CachedFileTree {
  workspace: string;
  skillPath: string;
  refName: string;
  files: unknown[];
  fetchedAt: number;
}

function treeKey(workspace: string, skillPath: string, refName?: string): string {
  return `${TREE_PREFIX}${workspace}:${skillPath}:${refName ?? "HEAD"}`;
}

export async function getFileTreeFromCache(
  workspace: string,
  skillPath: string,
  refName?: string,
): Promise<CachedFileTree | null> {
  try {
    const raw = await dbCacheGet(treeKey(workspace, skillPath, refName));
    if (!raw) return null;
    return fromBase64<CachedFileTree>(raw);
  } catch {
    return null;
  }
}

export async function putFileTreeInCache(
  workspace: string,
  skillPath: string,
  refName: string | undefined,
  files: unknown[],
): Promise<void> {
  try {
    const entry: CachedFileTree = {
      workspace,
      skillPath,
      refName: refName ?? "HEAD",
      files,
      fetchedAt: Date.now(),
    };
    await dbCachePut(treeKey(workspace, skillPath, refName), workspace, toBase64(entry));
  } catch { /* non-critical */ }
}

export async function clearFileTree(workspace: string, skillPath: string): Promise<void> {
  try {
    const prefix = `${TREE_PREFIX}${workspace}:${skillPath}:`;
    await dbCacheDeletePrefix(prefix);
  } catch { /* ignore */ }
}

// ---------------------------------------------------------------------------
// Skill detail cache (per skill path) — stored in SQLite. Lets clicking a
// skill render instantly from cache while a fresh copy is fetched in the
// background. Small JSON (manifest + readme + skill markdown + versions).
// ---------------------------------------------------------------------------

function detailKey(workspace: string, skillPath: string, refName?: string): string {
  return `${DETAIL_PREFIX}${workspace}:${skillPath}:${refName ?? "HEAD"}`;
}

export async function getSkillDetailFromCache<T>(
  workspace: string,
  skillPath: string,
  refName?: string,
): Promise<T | null> {
  try {
    const raw = await dbCacheGet(detailKey(workspace, skillPath, refName));
    if (!raw) return null;
    return fromBase64<T>(raw);
  } catch {
    return null;
  }
}

export async function putSkillDetailInCache(
  workspace: string,
  skillPath: string,
  refName: string | undefined,
  detail: unknown,
): Promise<void> {
  try {
    await dbCachePut(detailKey(workspace, skillPath, refName), workspace, toBase64(detail));
  } catch { /* non-critical */ }
}

export async function clearSkillDetail(workspace: string, skillPath: string): Promise<void> {
  try {
    const prefix = `${DETAIL_PREFIX}${workspace}:${skillPath}:`;
    await dbCacheDeletePrefix(prefix);
  } catch { /* ignore */ }
}

// ---------------------------------------------------------------------------
// File content cache — stored as real files on disk (~/.team-ai-hub/remote/)
// ---------------------------------------------------------------------------

export interface CachedFileContent {
  workspace: string;
  filePath: string;
  refName: string;
  content: string;
  isBinary: boolean;
  fetchedAt: number;
}

export async function getFileContentFromCache(
  workspace: string,
  filePath: string,
  refName?: string,
): Promise<CachedFileContent | null> {
  try {
    const ref = refName ?? "HEAD";
    const result: CachedFileResult | null = await remoteCacheGetFile(workspace, ref, filePath);
    if (!result) return null;
    return {
      workspace,
      filePath,
      refName: ref,
      content: result.content,
      isBinary: result.isBinary,
      fetchedAt: 0, // not tracked for filesystem cache
    };
  } catch {
    return null;
  }
}

export async function putFileContentInCache(
  workspace: string,
  filePath: string,
  refName: string | undefined,
  content: unknown,
): Promise<void> {
  try {
    const ref = refName ?? "HEAD";
    // content is typically a FileContent object from the API: { content, isBinary, encoding, ... }
    const fileData = content as { content?: string; isBinary?: boolean; is_binary?: boolean };
    const data = fileData.content ?? (typeof content === "string" ? content : JSON.stringify(content));
    const isBinary = fileData.isBinary ?? fileData.is_binary ?? false;
    await remoteCachePutFile(workspace, ref, filePath, data, isBinary);
  } catch { /* non-critical */ }
}

export async function clearFilesForSkill(workspace: string, skillPath: string): Promise<void> {
  try {
    await remoteCacheDeleteSkill(workspace, skillPath);
  } catch { /* ignore */ }
}

// ---------------------------------------------------------------------------
// Clear all cache for a workspace
// ---------------------------------------------------------------------------

export async function clearAllCacheForWorkspace(workspace: string): Promise<void> {
  try {
    // Clear SQLite metadata (HEAD SHA, file trees)
    await dbClearCache(workspace);
    // Clear filesystem file cache
    await remoteCacheDeleteWorkspace(workspace);
  } catch { /* ignore */ }
}

// ---------------------------------------------------------------------------
// Cache size analytics
// ---------------------------------------------------------------------------

export interface WorkspaceCacheSize {
  workspace: string;
  totalBytes: number;
  fileCount: number;
}

/** Get cache size breakdown by workspace (combines SQLite metadata + filesystem files). */
export async function getCacheSizeByWorkspace(): Promise<{ workspaces: WorkspaceCacheSize[]; totalBytes: number }> {
  try {
    // Get SQLite metadata cache stats
    const sqliteStats = await dbCacheStats();
    // Get filesystem file cache stats
    const fileStats = await remoteCacheStats();

    // Merge by workspace
    const merged = new Map<string, { bytes: number; count: number }>();

    for (const s of sqliteStats) {
      const existing = merged.get(s.workspace) ?? { bytes: 0, count: 0 };
      existing.bytes += s.totalBytes;
      existing.count += s.count;
      merged.set(s.workspace, existing);
    }

    for (const s of fileStats) {
      const existing = merged.get(s.workspace) ?? { bytes: 0, count: 0 };
      existing.bytes += s.totalBytes;
      existing.count += s.fileCount;
      merged.set(s.workspace, existing);
    }

    const workspaces: WorkspaceCacheSize[] = [];
    let totalBytes = 0;
    for (const [workspace, { bytes, count }] of merged) {
      workspaces.push({ workspace, totalBytes: bytes, fileCount: count });
      totalBytes += bytes;
    }
    workspaces.sort((a, b) => b.totalBytes - a.totalBytes);

    return { workspaces, totalBytes };
  } catch {
    return { workspaces: [], totalBytes: 0 };
  }
}

// ---------------------------------------------------------------------------
// Discussions enabled status cache (per workspace)
// ---------------------------------------------------------------------------

const DISCUSSIONS_PREFIX = "teamai-discussions-enabled:";

function discussionsKey(workspace: string): string {
  return `${DISCUSSIONS_PREFIX}${workspace}`;
}

/** Get cached discussions enabled status for a workspace. Returns null if not cached. */
export async function getDiscussionsEnabledCache(workspace: string): Promise<boolean | null> {
  try {
    const raw = await dbCacheGet(discussionsKey(workspace));
    if (!raw) return null;
    return raw === "1";
  } catch {
    return null;
  }
}

/** Cache whether discussions are enabled for a workspace. */
export async function setDiscussionsEnabledCache(workspace: string, enabled: boolean): Promise<void> {
  try {
    await dbCachePut(discussionsKey(workspace), workspace, enabled ? "1" : "0");
  } catch { /* non-critical */ }
}

// ---------------------------------------------------------------------------
// Skills list cache (per workspace) — avoids skeleton flash on startup
// ---------------------------------------------------------------------------

const SKILLS_LIST_PREFIX = "teamai-skills-list:";

function skillsListKey(workspace: string): string {
  return `${SKILLS_LIST_PREFIX}${workspace}`;
}

/** Get cached skills list for a workspace. Returns null if not cached. */
export async function getSkillsListCache(workspace: string): Promise<unknown[] | null> {
  try {
    const raw = await dbCacheGet(skillsListKey(workspace));
    if (!raw) return null;
    return fromBase64<unknown[]>(raw);
  } catch {
    return null;
  }
}

/** Cache the skills list for a workspace. */
export async function setSkillsListCache(workspace: string, skills: unknown[]): Promise<void> {
  try {
    await dbCachePut(skillsListKey(workspace), workspace, toBase64(skills));
  } catch { /* non-critical */ }
}

// ---------------------------------------------------------------------------
// Discussion mapping cache (skillId → discussion ID/number)
// Avoids scanning all 100 discussions every time we open comments tab
// ---------------------------------------------------------------------------

const DISCUSSION_MAP_PREFIX = "teamai-discussion-map:";

/** Cache TTL: 24 hours — after this, we re-validate via full scan */
const DISCUSSION_MAP_TTL_MS = 24 * 60 * 60 * 1000;

export interface CachedDiscussionMapping {
  /** null means "we checked and no discussion exists for this skill" (negative cache) */
  discussionId: string | null;
  discussionNumber: number | null;
  cachedAt: number;
}

function discussionMapKey(workspace: string, skillId: string): string {
  return `${DISCUSSION_MAP_PREFIX}${workspace}:${skillId}`;
}

/** Get cached discussion mapping for a skill. Returns null if not cached or expired. */
export async function getDiscussionMappingCache(
  workspace: string,
  skillId: string,
): Promise<CachedDiscussionMapping | null> {
  try {
    const raw = await dbCacheGet(discussionMapKey(workspace, skillId));
    if (!raw) return null;
    const mapping = fromBase64<CachedDiscussionMapping>(raw);
    // Check TTL — expired cache is treated as miss
    if (Date.now() - mapping.cachedAt > DISCUSSION_MAP_TTL_MS) {
      return null;
    }
    return mapping;
  } catch {
    return null;
  }
}

/** Cache the discussion mapping for a skill. Pass null id/number for negative cache. */
export async function setDiscussionMappingCache(
  workspace: string,
  skillId: string,
  mapping: CachedDiscussionMapping,
): Promise<void> {
  try {
    await dbCachePut(discussionMapKey(workspace, skillId), workspace, toBase64(mapping));
  } catch { /* non-critical */ }
}

/** Clear discussion mapping cache for a skill (e.g. when discussion is deleted or stale). */
export async function clearDiscussionMappingCache(
  workspace: string,
  skillId: string,
): Promise<void> {
  try {
    await dbCacheDelete(discussionMapKey(workspace, skillId));
  } catch { /* ignore */ }
}

/** Clear all cache data (SQLite metadata + filesystem files). */
export async function clearAllCache(): Promise<void> {
  try {
    await dbClearCache();
    // Get all workspaces from remote cache and delete them
    const stats = await remoteCacheStats();
    for (const s of stats) {
      await remoteCacheDeleteWorkspace(s.workspace);
    }
  } catch { /* ignore */ }
}
