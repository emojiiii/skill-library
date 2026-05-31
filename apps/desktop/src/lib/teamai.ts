import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type RiskLevel = "low" | "medium" | "high" | "critical";

export interface SkillManifest {
  schemaVersion: number;
  id: string;
  type: "skill";
  name: string;
  description: string;
  version: string;
  targets: string[];
  permissions: string[];
  tags: string[];
  risk?: {
    level: RiskLevel;
    notes?: string;
  };
}

export interface ManifestIssue {
  field: string;
  message: string;
}

export interface SkillAsset {
  path: string;
  manifest: SkillManifest;
  warnings: ManifestIssue[];
}

export interface AppStateSummary {
  home: string;
  config: string;
  subscriptions: string;
  workspaces: string;
}

export interface DiagnosticsExport {
  exportedAt: string;
  outputDir: string;
  appHome: string;
  subscriptions: number;
  workspaces: number;
  logs: string[];
  notes: string[];
}

export interface Workspace {
  provider: string;
  owner: string;
  repo: string;
  full_name: string;
  default_branch: string;
  visibility: string;
  permission: string;
  html_url?: string | null;
}

export interface WorkspaceSkillScan {
  workspace: Workspace;
  skills: SkillAsset[];
}

export interface StoredWorkspace extends Workspace {
  webhook?: {
    id: string;
    callback_url?: string | null;
    events: string[];
    registered_at: string;
  } | null;
  added_at: string;
}

export interface WorkspacesFile {
  workspaces: StoredWorkspace[];
}

export interface MarkdownDocument {
  path: string;
  content: string;
}

export interface SkillVersion {
  name: string;
  sha: string;
}

export interface WorkspaceDetail {
  workspace: Workspace;
  skills: SkillAsset[];
  readme?: MarkdownDocument | null;
  versions: SkillVersion[];
}

export interface SkillDetail {
  workspace: Workspace;
  asset: SkillAsset;
  readme?: MarkdownDocument | null;
  skill_markdown?: MarkdownDocument | null;
  versions: SkillVersion[];
  ref_name?: string | null;
}

export interface AuthStatus {
  githubLogin?: string | null;
  githubScopes: string[];
  credentialStore: string;
  warning?: string | null;
}

export interface GitHubLoginResult {
  login: string;
  scopes: string[];
  credentialStore: string;
  warning: string;
}

export interface GitHubDeviceStartResult {
  clientId: string;
  deviceCode: string;
  userCode: string;
  verificationUri: string;
  verificationUriComplete?: string | null;
  expiresAt: number;
  interval: number;
  scopes: string[];
}

export type GitHubDevicePollResult =
  | { status: "pending" }
  | { status: "slowDown"; interval: number }
  | { status: "authorized"; login: GitHubLoginResult };

export interface DeepLinkPayload {
  url: string;
  action: string;
  workspace?: {
    provider: string;
    owner: string;
    repo: string;
  } | null;
  assetId?: string | null;
  version?: string | null;
  targets: string[];
  query: Record<string, string>;
}

export interface Invitation {
  id: string;
  login_or_email: string;
  state: string;
}

export interface WorkspaceMember {
  login: string;
  role: "admin" | "maintain" | "write" | "triage" | "read" | "none";
  avatar_url?: string | null;
}

export interface Subscription {
  workspace: {
    provider: string;
    owner: string;
    repo: string;
  };
  asset_id: string;
  channel: string;
  version?: string;
  update: "auto-patch" | "auto-minor" | "manual" | "pin";
  targets: {
    claude_code: boolean;
    cursor: boolean;
    codex: boolean;
    custom: string[];
  };
  subscribed_at?: string;
}

export interface SubscriptionsFile {
  subscriptions: Subscription[];
}

export interface InstallReport {
  manifest: SkillManifest;
  installed: Array<{
    target: string;
    path: string;
    installed_at: string;
  }>;
  skipped: string[];
}

export interface InstallMetadata {
  id: string;
  name: string;
  version: string;
  installed_at: string;
  source: string;
  target: string;
  managed_by: string;
}

export interface InstalledTargetGroup {
  target: string;
  skills: InstallMetadata[];
}

export interface LocalAgentEntry {
  id: string;
  name: string;
  path: string;
  hasManifest: boolean;
  hasSkillMd: boolean;
  managed: boolean;
  version?: string | null;
  description?: string | null;
}

export interface LocalAgentRoot {
  id: string;
  label: string;
  kind: string;
  path: string;
  exists: boolean;
  entries: LocalAgentEntry[];
}

export interface PublishPreview {
  package: {
    manifest: SkillManifest;
    source_path: string;
    source_hash: string;
    risk_level: RiskLevel;
    file_count: number;
    total_bytes: number;
    created_at: string;
  };
  policy: PublishPolicyResult;
  request?: {
    branch_name: string;
    title: string;
    body: string;
  };
}

export interface PublishPolicyResult {
  decision: "allow_auto_merge" | "require_review" | "reject";
  schema_passed: boolean;
  auto_merge_allowed: boolean;
  reasons: string[];
  risk_level: RiskLevel;
  dangerous_permissions: string[];
  script_files: string[];
  large_files: Array<{
    path: string;
    bytes: number;
  }>;
}

// Kept as types in case future UI needs them; data sources will be GitHub PR API.
export interface PublishRequestRecord {
  id: string;
  workspace: string;
  skillId: string;
  skillVersion: string;
  sourceUser: string;
  sourcePath?: string | null;
  sourceHash: string;
  pullRequest?: {
    number: number;
    title: string;
    htmlUrl: string;
    state: string;
  } | null;
  policy: PublishPolicyResult;
  state: "open" | "merged" | "closed" | "rejected" | "waiting_review";
  autoMerged: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface PublishPolicyCheckRecord {
  id: string;
  workspace: string;
  skillId?: string | null;
  skillVersion?: string | null;
  sourceHash?: string | null;
  policy: PublishPolicyResult;
  decision: "allow_auto_merge" | "require_review" | "reject";
  createdAt: string;
}

export interface NotificationEvent {
  id: string;
  kind: "workspace_updated";
  provider: "github";
  repository: string;
  ref: string | null;
  after: string | null;
  sourceEvent: string;
  delivery: string | null;
  receivedAt: string;
}

export interface InvitationRecord {
  id: string;
  provider: string;
  workspace: string;
  invitee: string;
  role: string;
  providerInvitationId?: string | null;
  state: "pending" | "accepted" | "declined" | "expired";
  onboardingStatus?: "invited" | "needs_github_account" | "needs_provider_acceptance" | "workspace_ready";
  createdAt: string;
  updatedAt: string;
}

export interface ChangedFile {
  filename: string;
  status: string;
  patch?: string | null;
}

export interface SemanticChange {
  path: string;
  kind: "added" | "removed" | "changed";
  value?: unknown;
  before?: unknown;
  after?: unknown;
  risk?: RiskLevel | null;
}

export interface SkillComparison {
  workspace: {
    provider: string;
    owner: string;
    repo: string;
  };
  skillPath: string;
  from: string;
  to: string;
  files: ChangedFile[];
  semantic: SemanticChange[];
}

// ----------------------------------------------------------------------------
// Runtime detection — Tauri exposes __TAURI_INTERNALS__ on window. In a plain
// browser we never want to fake auth or fabricate skills; reads return empty
// shapes and writes throw a helpful error instead.
// ----------------------------------------------------------------------------

export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

const desktopOnly = (op: string): never => {
  throw new Error(
    `${op} requires the Team AI Hub desktop app (Tauri). Run \`pnpm dev\` (which calls \`tauri dev\`) instead of opening the Vite dev URL in a browser.`,
  );
};

// ---------------------------------------------------------------------------
// Deep link
// ---------------------------------------------------------------------------

export async function getDeepLinkState(): Promise<DeepLinkPayload | null> {
  if (!isTauri) return null;
  return invoke("get_deep_link_state");
}

export async function onDeepLink(
  handler: (payload: DeepLinkPayload) => void,
): Promise<() => void> {
  if (!isTauri) return () => undefined;
  return listen<DeepLinkPayload>("teamai://deep-link", (event) => handler(event.payload));
}

// ---------------------------------------------------------------------------
// App init / diagnostics
// ---------------------------------------------------------------------------

export async function appInit(): Promise<AppStateSummary> {
  if (!isTauri) {
    return {
      home: "browser preview",
      config: "browser preview",
      subscriptions: "browser preview",
      workspaces: "browser preview",
    };
  }
  return invoke("app_init");
}

export async function exportDiagnostics(): Promise<DiagnosticsExport> {
  if (!isTauri) return desktopOnly("Export diagnostics");
  return invoke("export_diagnostics");
}

export async function openLogsFolder(): Promise<void> {
  if (!isTauri) return desktopOnly("Open logs");
  return invoke("open_logs_folder");
}

// ---------------------------------------------------------------------------
// Auth
// ---------------------------------------------------------------------------

export async function getAuthStatus(): Promise<AuthStatus> {
  if (!isTauri) {
    return {
      githubLogin: null,
      githubScopes: [],
      credentialStore: "browser",
      warning: "Browser preview cannot persist GitHub credentials. Launch the desktop app to sign in.",
    };
  }
  return invoke("get_auth_status");
}

export async function loginGithubToken(token: string): Promise<GitHubLoginResult> {
  if (!isTauri) return desktopOnly("Save GitHub token");
  return invoke("login_github_token", { token });
}

export async function startGithubDeviceFlow(clientId?: string): Promise<GitHubDeviceStartResult> {
  if (!isTauri) return desktopOnly("Start GitHub device flow");
  return invoke("start_github_device_flow", { clientId });
}

export async function pollGithubDeviceFlow(args: {
  clientId: string;
  deviceCode: string;
}): Promise<GitHubDevicePollResult> {
  if (!isTauri) return desktopOnly("Poll GitHub device flow");
  return invoke("poll_github_device_flow", args);
}

// ---------------------------------------------------------------------------
// Workspaces
// ---------------------------------------------------------------------------

export async function scanWorkspace(path: string): Promise<SkillAsset[]> {
  if (!isTauri) return [];
  return invoke("scan_workspace", { path });
}

export async function scanGithubWorkspace(args: {
  workspace: string;
  token?: string;
}): Promise<WorkspaceSkillScan> {
  if (!isTauri) return desktopOnly("Scan GitHub workspace");
  return invoke("scan_github_workspace", args);
}

/** Streaming variant — emits `workspace-scan-progress` events as batches complete. */
export async function scanGithubWorkspaceStreaming(args: {
  workspace: string;
  token?: string;
}): Promise<WorkspaceSkillScan> {
  if (!isTauri) return desktopOnly("Scan GitHub workspace");
  return invoke("scan_github_workspace_streaming", args);
}

/** Listen for incremental scan progress events. */
export async function onScanProgress(
  handler: (skills: SkillAsset[]) => void,
): Promise<() => void> {
  if (!isTauri) return () => undefined;
  return listen<SkillAsset[]>("workspace-scan-progress", (event) => handler(event.payload));
}

export async function getWorkspaceDetail(args: {
  workspace: string;
  token?: string;
}): Promise<WorkspaceDetail> {
  if (!isTauri) return desktopOnly("Load workspace detail");
  return invoke("get_workspace_detail", args);
}

export async function getSkillDetail(args: {
  workspace: string;
  skillPath: string;
  refName?: string;
  token?: string;
}): Promise<SkillDetail> {
  if (!isTauri) return desktopOnly("Load skill detail");
  return invoke("get_skill_detail", args);
}

export async function compareSkillVersions(args: {
  workspace: string;
  skillPath: string;
  from: string;
  to: string;
  token?: string;
}): Promise<SkillComparison> {
  if (!isTauri) return desktopOnly("Compare skill versions");
  return invoke("compare_skill_versions", args);
}

export async function listGithubWorkspaces(query?: string): Promise<Workspace[]> {
  if (!isTauri) return [];
  return invoke("list_github_workspaces", { query });
}

export async function listWorkspaces(): Promise<WorkspacesFile> {
  if (!isTauri) return { workspaces: [] };
  return invoke("list_workspaces");
}

export async function addWorkspace(args: {
  workspace: string;
  token?: string;
  webhookUrl?: string;
  webhookSecret?: string;
  webhookEvents?: string[];
}): Promise<StoredWorkspace> {
  if (!isTauri) return desktopOnly("Add workspace");
  return invoke("add_workspace", args);
}

export async function inviteGithubCollaborator(args: {
  workspace: string;
  login: string;
  role: string;
  token?: string;
}): Promise<Invitation> {
  if (!isTauri) return desktopOnly("Invite collaborator");
  return invoke("invite_github_collaborator", args);
}

export async function listWorkspaceMembers(args: {
  workspace: string;
  token?: string;
}): Promise<WorkspaceMember[]> {
  if (!isTauri) return [];
  return invoke("list_workspace_members", args);
}

// ---------------------------------------------------------------------------
// Workspace sync (SHA-based change detection)
// ---------------------------------------------------------------------------

export interface WorkspaceHeadInfo {
  sha: string;
  branch: string;
  committedAt?: string | null;
}

export interface WorkspaceChangedPaths {
  baseSha: string;
  headSha: string;
  changedSkillPaths: string[];
  totalChangedFiles: number;
}

/** Cheapest possible check: returns the HEAD SHA of the workspace's default branch. */
export async function checkWorkspaceHead(workspace: string): Promise<WorkspaceHeadInfo> {
  if (!isTauri) return desktopOnly("Check workspace head");
  return invoke("check_workspace_head", { workspace });
}

/** Given a base SHA and head SHA, returns which skill paths have changed files. */
export async function diffWorkspaceSince(args: {
  workspace: string;
  baseSha: string;
  headSha: string;
  skillPaths: string[];
}): Promise<WorkspaceChangedPaths> {
  if (!isTauri) return desktopOnly("Diff workspace since");
  return invoke("diff_workspace_since", args);
}

// ---------------------------------------------------------------------------
// Branch listing
// ---------------------------------------------------------------------------

export interface BranchInfo {
  name: string;
  isDefault: boolean;
}

/** Lists branches for a workspace repository. */
export async function listWorkspaceBranches(workspace: string): Promise<BranchInfo[]> {
  if (!isTauri) return [];
  return invoke("list_workspace_branches", { workspace });
}

// ---------------------------------------------------------------------------
// Skill file tree & single file read
// ---------------------------------------------------------------------------

export interface SkillFileEntry {
  path: string;
  relativePath: string;
  kind: "file" | "directory";
  size?: number | null;
}

export interface FileContent {
  path: string;
  content: string;
  sha: string;
  encoding: string;
  isBinary: boolean;
}

/** Lists all files inside a skill directory (recursive). */
export async function listSkillFiles(args: {
  workspace: string;
  skillPath: string;
  refName?: string;
}): Promise<SkillFileEntry[]> {
  if (!isTauri) return [];
  return invoke("list_skill_files", args);
}

/** Reads a single file from the workspace repo. */
export async function readSkillFile(args: {
  workspace: string;
  filePath: string;
  refName?: string;
}): Promise<FileContent> {
  if (!isTauri) return desktopOnly("Read skill file");
  return invoke("read_skill_file", args);
}

// ---------------------------------------------------------------------------
// GitHub Discussions (likes + comments)
// ---------------------------------------------------------------------------

export interface ReactionGroup {
  content: string;
  count: number;
  viewerHasReacted: boolean;
}

export interface DiscussionInfo {
  id: string;
  number: number;
  title: string;
  url: string;
  body: string;
  bodyAuthor: string;
  bodyAuthorAvatar?: string | null;
  upvotes: number;
  commentCount: number;
  createdAt: string;
  hasDiscussions: boolean;
  reactions: ReactionGroup[];
}

export interface DiscussionComment {
  id: string;
  author: string;
  authorAvatar?: string | null;
  body: string;
  createdAt: string;
  upvotes: number;
  reactions: ReactionGroup[];
}

export interface DiscussionsStatus {
  enabled: boolean;
  discussions: DiscussionInfo[];
}

/** Check if Discussions are enabled and list skill discussions. */
export async function listSkillDiscussions(args: {
  workspace: string;
  skillIds: string[];
}): Promise<DiscussionsStatus> {
  if (!isTauri) return { enabled: false, discussions: [] };
  return invoke("list_skill_discussions", args);
}

/** Get a single discussion by number (used with cached mapping). */
export async function getDiscussionByNumber(args: {
  workspace: string;
  discussionNumber: number;
}): Promise<DiscussionInfo | null> {
  if (!isTauri) return null;
  return invoke("get_discussion_by_number", args);
}

/** Get comments for a specific discussion. */
export async function getDiscussionComments(args: {
  workspace: string;
  discussionNumber: number;
}): Promise<DiscussionComment[]> {
  if (!isTauri) return [];
  return invoke("get_discussion_comments", args);
}

/** Add a comment to a discussion. */
export async function addDiscussionComment(args: {
  workspace: string;
  discussionId: string;
  body: string;
}): Promise<DiscussionComment> {
  if (!isTauri) return desktopOnly("Add discussion comment");
  return invoke("add_discussion_comment", args);
}

/** Add a reaction to a discussion (or toggle it on). */
export async function toggleDiscussionReaction(args: {
  workspace: string;
  discussionId: string;
  content: string;
}): Promise<boolean> {
  if (!isTauri) return desktopOnly("Toggle discussion reaction");
  return invoke("toggle_discussion_reaction", args);
}

/** Remove a reaction from a discussion. */
export async function removeDiscussionReaction(args: {
  workspace: string;
  discussionId: string;
  content: string;
}): Promise<boolean> {
  if (!isTauri) return desktopOnly("Remove discussion reaction");
  return invoke("remove_discussion_reaction", args);
}

/** Create a discussion for a skill (with race-condition re-check). */
export async function createSkillDiscussion(args: {
  workspace: string;
  skillId: string;
  skillPath?: string;
  body?: string;
}): Promise<DiscussionInfo> {
  if (!isTauri) return desktopOnly("Create skill discussion");
  return invoke("create_skill_discussion", args);
}

// ---------------------------------------------------------------------------
// Subscriptions / installs
// ---------------------------------------------------------------------------

export async function readSubscriptions(): Promise<SubscriptionsFile> {
  if (!isTauri) return { subscriptions: [] };
  return invoke("read_subscriptions");
}

export async function subscribeWorkspaceSkill(args: {
  workspace: string;
  assetId: string;
  version?: string;
  targets: string[];
}): Promise<SubscriptionsFile> {
  if (!isTauri) return desktopOnly("Subscribe");
  return invoke("subscribe_workspace_skill", args);
}

export interface SyncItemReport {
  asset_id: string;
  version?: string | null;
  error?: string | null;
}

export interface SyncReport {
  synced_at: string;
  items: SyncItemReport[];
}

/**
 * Download + install all subscribed skills (GitHub archive → extract → install
 * → pin lockfile). This is the real remote-install path; `installSkill` only
 * handles local directories. Pass allowRisky=true to install medium+-risk
 * skills the user has already confirmed.
 */
export async function syncNow(allowRisky = false): Promise<SyncReport> {
  if (!isTauri) return desktopOnly("Sync skills");
  return invoke("sync_now", { allowRisky });
}

export async function installSkill(
  source: string,
  targets: string[],
  confirmedRisk = false,
): Promise<InstallReport> {
  if (!isTauri) return desktopOnly("Install skill");
  return invoke("install_skill", { source, targets, confirmedRisk });
}

/** Remove a skill from specified runtime targets. */
export async function removeSkill(skillId: string, targets: string[]): Promise<string[]> {
  if (!isTauri) return desktopOnly("Remove skill");
  return invoke("remove_skill", { skillId, targets });
}

export async function listInstalledTargets(targets?: string[]): Promise<InstalledTargetGroup[]> {
  if (!isTauri) return [];
  return invoke("list_installed_targets", { targets });
}

export async function listLocalAgentRoots(): Promise<LocalAgentRoot[]> {
  if (!isTauri) return [];
  return invoke("list_local_agent_roots");
}

// ---------------------------------------------------------------------------
// SQLite-backed skill management
// ---------------------------------------------------------------------------

export interface ManagedSkillTarget {
  runtime: string;
  enabled: boolean;
  targetPath: string;
}

export interface ManagedSkill {
  id: string;
  name: string;
  description: string;
  version: string;
  sourceWorkspace: string;
  sourcePath: string;
  sourceBranch: string;
  localPath: string;
  linkMode: string;
  baselineHash: string;
  isModified: boolean;
  installedAt: string;
  updatedAt: string;
  /** 'downloading' | 'installed' | 'error' */
  installStatus: "downloading" | "installed" | "error";
  /** 0..=100, or -1 when the stream length is unknown (indeterminate bar). */
  downloadProgress: number;
  downloadError: string;
  /** '' (never reviewed) | 'safe' | 'caution' | 'danger' */
  reviewVerdict: "" | "safe" | "caution" | "danger";
  reviewSummary: string;
  reviewFindings: AiReviewFinding[];
  /** RFC3339 timestamp of the last review, or '' if never reviewed. */
  reviewedAt: string;
  /** True when a review exists but the skill changed since (verdict is stale). */
  reviewStale: boolean;
  targets: ManagedSkillTarget[];
}

export interface UnmanagedSkillInfo {
  id: string;
  name: string;
  path: string;
  foundIn: string[];
}

export interface SupportedRuntime {
  id: string;
  label: string;
  globalPath: string;
  exists: boolean;
}

export interface CacheSizeInfo {
  workspace: string;
  count: number;
  totalBytes: number;
}

/** List all supported runtimes and whether their directories exist. */
export async function dbListRuntimes(): Promise<SupportedRuntime[]> {
  if (!isTauri) return [];
  return invoke("db_list_runtimes");
}

/** List all managed skills from SQLite with their target states. */
export async function dbListSkills(): Promise<ManagedSkill[]> {
  if (!isTauri) return [];
  return invoke("db_list_skills");
}

/** Enable a skill for a specific runtime (create symlink/copy). */
export async function dbEnableSkill(skillId: string, runtime: string): Promise<void> {
  if (!isTauri) return desktopOnly("Enable skill");
  return invoke("db_enable_skill", { skillId, runtime });
}

/** Disable a skill for a specific runtime (remove symlink/copy). */
export async function dbDisableSkill(skillId: string, runtime: string): Promise<void> {
  if (!isTauri) return desktopOnly("Disable skill");
  return invoke("db_disable_skill", { skillId, runtime });
}

/** Scan all IDE directories for skills not managed by us. */
export async function dbScanUnmanaged(): Promise<UnmanagedSkillInfo[]> {
  if (!isTauri) return [];
  return invoke("db_scan_unmanaged");
}

/** Import an unmanaged skill into our data directory and register it. */
export async function dbImportSkill(skillId: string, sourcePath: string, linkMode?: string): Promise<ManagedSkill> {
  if (!isTauri) return desktopOnly("Import skill");
  return invoke("db_import_skill", { skillId, sourcePath, linkMode });
}

export interface SkillDownloadProgress {
  skillId: string;
  status: "downloading" | "installed" | "error";
  /** 0..=100, or -1 when the stream length is unknown (indeterminate bar). */
  progress: number;
  error?: string | null;
}

/**
 * Start an asynchronous download + install of a remote skill. Returns
 * immediately after recording a 'downloading' row; progress arrives via
 * `onSkillDownloadProgress`. Empty `targets` = download locally, deploy nowhere.
 *
 * Throws a coded error ("already_downloading" / "already_installed") when the
 * skill is already present, so the caller can show a notice instead of a
 * redundant download.
 */
export async function downloadSkillAsync(args: {
  workspace: string;
  assetId: string;
  skillPath?: string;
  version?: string;
  name?: string;
  description?: string;
  targets: string[];
  linkMode?: string;
}): Promise<void> {
  if (!isTauri) return desktopOnly("Download skill");
  return invoke("download_skill_async", args);
}

/** Listen for async download progress events. */
export async function onSkillDownloadProgress(
  handler: (progress: SkillDownloadProgress) => void,
): Promise<() => void> {
  if (!isTauri) return () => undefined;
  return listen<SkillDownloadProgress>("skill-download-progress", (event) => handler(event.payload));
}

/** Get cache size breakdown by workspace (from SQLite). */
export async function dbCacheStats(): Promise<CacheSizeInfo[]> {
  if (!isTauri) return [];
  return invoke("db_cache_stats");
}

/** Clear cache for a specific workspace or all. */
export async function dbClearCache(workspace?: string): Promise<void> {
  if (!isTauri) return;
  return invoke("db_clear_cache", { workspace });
}

/** Get a cache entry by key. Returns JSON string (base64-decoded) or null. */
export async function dbCacheGet(key: string): Promise<string | null> {
  if (!isTauri) return null;
  return invoke("db_cache_get", { key });
}

/** Put a cache entry. Data is base64-encoded JSON string. */
export async function dbCachePut(key: string, workspace: string, data: string): Promise<void> {
  if (!isTauri) return;
  return invoke("db_cache_put", { key, workspace, data });
}

/** Delete a single cache entry by key. */
export async function dbCacheDelete(key: string): Promise<void> {
  if (!isTauri) return;
  return invoke("db_cache_delete", { key });
}

/** Delete all cache entries whose key starts with the given prefix. */
export async function dbCacheDeletePrefix(prefix: string): Promise<number> {
  if (!isTauri) return 0;
  return invoke("db_cache_delete_prefix", { prefix });
}

// ---------------------------------------------------------------------------
// Filesystem-based remote file cache (~/.team-ai-hub/remote/)
// ---------------------------------------------------------------------------

export interface CachedFileResult {
  content: string;
  isBinary: boolean;
}

/** Write a file to the remote cache directory. */
export async function remoteCachePutFile(
  workspace: string,
  refName: string,
  filePath: string,
  data: string,
  isBinary: boolean,
): Promise<void> {
  if (!isTauri) return;
  return invoke("remote_cache_put_file", { workspace, refName, filePath, data, isBinary });
}

/** Read a file from the remote cache directory. Returns null if not cached. */
export async function remoteCacheGetFile(
  workspace: string,
  refName: string,
  filePath: string,
): Promise<CachedFileResult | null> {
  if (!isTauri) return null;
  return invoke("remote_cache_get_file", { workspace, refName, filePath });
}

/** Delete all cached files for a specific skill path within a workspace. */
export async function remoteCacheDeleteSkill(workspace: string, skillPath: string): Promise<void> {
  if (!isTauri) return;
  return invoke("remote_cache_delete_skill", { workspace, skillPath });
}

/** Delete all cached files for a workspace. */
export async function remoteCacheDeleteWorkspace(workspace: string): Promise<void> {
  if (!isTauri) return;
  return invoke("remote_cache_delete_workspace", { workspace });
}

export interface RemoteCacheStat {
  workspace: string;
  totalBytes: number;
  fileCount: number;
}

/** Get cache size stats for the remote file cache. */
export async function remoteCacheStats(): Promise<RemoteCacheStat[]> {
  if (!isTauri) return [];
  return invoke("remote_cache_stats");
}

/** Check all managed skills for local modifications. Returns IDs of modified skills. */
export async function dbCheckModifications(): Promise<string[]> {
  if (!isTauri) return [];
  return invoke("db_check_modifications");
}

/** Unmanage a skill: remove from registry, restore real files to IDE directories. */
export async function dbUnmanageSkill(skillId: string): Promise<void> {
  if (!isTauri) return desktopOnly("Unmanage skill");
  return invoke("db_unmanage_skill", { skillId });
}

/** Open the Team AI Hub data directory in the system file manager. */
export async function openDataDir(): Promise<void> {
  if (!isTauri) return desktopOnly("Open data dir");
  return invoke("open_data_dir");
}

// ---------------------------------------------------------------------------
// Publish — local skill or skill-from-workspace, both go to a target workspace.
// ---------------------------------------------------------------------------

export async function previewPublish(args: {
  source: string;
  workspace?: string;
  user?: string;
}): Promise<PublishPreview> {
  if (!isTauri) return desktopOnly("Preview publish");
  return invoke("preview_publish", args);
}

export interface PullRequestSummary {
  number: number;
  title: string;
  htmlUrl: string;
  state: string;
}

export interface PublishResult {
  package: PublishPreview["package"];
  policy: PublishPolicyResult;
  request: NonNullable<PublishPreview["request"]>;
  pullRequest: PullRequestSummary;
  targetWorkspace: string;
  uploadedFiles: string[];
}

export async function previewPublishFromWorkspace(args: {
  sourceWorkspace: string;
  skillPath: string;
  sourceRef?: string;
  targetWorkspace: string;
  renameTo?: string;
  user?: string;
}): Promise<PublishPreview> {
  if (!isTauri) return desktopOnly("Preview cross-workspace sync");
  return invoke("preview_publish_from_workspace", args);
}

export async function publishSkillToWorkspace(args: {
  sourceWorkspace: string;
  skillPath: string;
  sourceRef?: string;
  targetWorkspace: string;
  renameTo?: string;
  user?: string;
  confirmedRisk?: boolean;
}): Promise<PublishResult> {
  if (!isTauri) return desktopOnly("Sync skill to workspace");
  return invoke("publish_skill_to_workspace", args);
}

// ---------------------------------------------------------------------------
// Governance — Pull requests, repository events, repository invitations.
// All read live from GitHub through the Rust provider; no API server needed.
// ---------------------------------------------------------------------------

export interface WorkspacePullRequest {
  number: number;
  title: string;
  html_url: string;
  state: string;
  draft: boolean;
  merged: boolean;
  author: string | null;
  head_ref: string;
  base_ref: string;
  created_at: string;
  updated_at: string;
  body: string | null;
}

export interface WorkspaceEvent {
  id: string;
  event_type: string;
  actor: string | null;
  created_at: string;
  summary: string;
  html_url: string | null;
}

export interface RepositoryInvitation {
  id: number;
  repository_full_name: string;
  inviter: string | null;
  permissions: string;
  html_url: string;
  created_at: string;
  expired: boolean;
}

export type PullRequestQueryState = "open" | "closed" | "all";

export async function listWorkspacePullRequests(
  workspace: string,
  state: PullRequestQueryState = "open",
): Promise<WorkspacePullRequest[]> {
  if (!isTauri) return [];
  return invoke("list_workspace_pull_requests", { workspace, state });
}

export async function listWorkspaceEvents(workspace: string): Promise<WorkspaceEvent[]> {
  if (!isTauri) return [];
  return invoke("list_workspace_events", { workspace });
}

export async function listRepositoryInvitations(): Promise<RepositoryInvitation[]> {
  if (!isTauri) return [];
  return invoke("list_repository_invitations");
}

export async function acceptRepositoryInvitation(invitationId: number): Promise<void> {
  if (!isTauri) return desktopOnly("Accept repository invitation");
  return invoke("accept_repository_invitation", { invitationId });
}

export interface SkillCommit {
  sha: string;
  short_sha: string;
  message: string;
  author: string | null;
  author_email: string | null;
  authored_at: string;
  html_url: string;
}

export async function listSkillCommits(args: {
  workspace: string;
  skillPath: string;
  refName?: string;
  limit?: number;
}): Promise<SkillCommit[]> {
  if (!isTauri) return [];
  return invoke("list_skill_commits", args);
}

// ---------------------------------------------------------------------------
// AI risk review
// ---------------------------------------------------------------------------

export interface AiReviewFinding {
  severity: "info" | "warning" | "danger";
  detail: string;
}

export interface AiReviewResult {
  verdict: "safe" | "caution" | "danger";
  summary: string;
  findings: AiReviewFinding[];
}

export interface AiReviewRequest {
  provider: string;
  baseUrl: string;
  model: string;
  /** Workspace ref ("owner/repo") — backend downloads the skill from here. */
  workspace: string;
  /** In-repo skill directory path. */
  skillPath: string;
  /** Optional git ref (branch/tag/sha); defaults to the repo's default branch. */
  refName?: string;
  skillName: string;
  permissions?: string[];
}

/** Store the AI provider API key in the OS keychain. */
export async function saveAiKey(key: string): Promise<void> {
  if (!isTauri) return desktopOnly("Save AI key");
  return invoke("save_ai_key", { key });
}

/** Remove the stored AI provider API key. */
export async function deleteAiKey(): Promise<void> {
  if (!isTauri) return desktopOnly("Delete AI key");
  return invoke("delete_ai_key");
}

/** Whether an AI API key is currently stored (never returns the key itself). */
export async function hasAiKey(): Promise<boolean> {
  if (!isTauri) return false;
  return invoke("has_ai_key");
}

/** Run an AI safety review of a skill's entire source tree against the configured provider. */
export async function reviewSkill(request: AiReviewRequest): Promise<AiReviewResult> {
  if (!isTauri) return desktopOnly("AI review");
  return invoke("review_skill", { request });
}

/**
 * Review an already-installed ("My Skills") skill straight from its local copy
 * on disk — no GitHub download. The verdict + findings are cached back into
 * SQLite (visible on the next dbListSkills) and also returned here.
 */
export async function reviewLocalSkill(args: {
  skillId: string;
  provider: string;
  baseUrl: string;
  model: string;
}): Promise<AiReviewResult> {
  if (!isTauri) return desktopOnly("AI review");
  return invoke("review_local_skill", args);
}
