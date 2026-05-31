use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};
use std::fs;
use sha2::{Sha256, Digest};

const SCHEMA_VERSION: u32 = 3;

/// All supported agent/IDE runtimes and their global skill directories.
pub struct RuntimeInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub global_path: &'static str, // relative to home dir
}

pub const SUPPORTED_RUNTIMES: &[RuntimeInfo] = &[
    RuntimeInfo { id: "claude-code", label: "Claude Code", global_path: ".claude/skills" },
    RuntimeInfo { id: "cursor", label: "Cursor", global_path: ".cursor/skills" },
    RuntimeInfo { id: "codex", label: "Codex", global_path: ".codex/skills" },
    RuntimeInfo { id: "gemini-cli", label: "Gemini CLI", global_path: ".gemini/skills" },
    RuntimeInfo { id: "github-copilot", label: "GitHub Copilot", global_path: ".copilot/skills" },
    RuntimeInfo { id: "windsurf", label: "Windsurf", global_path: ".codeium/windsurf/skills" },
    RuntimeInfo { id: "opencode", label: "OpenCode", global_path: ".config/opencode/skills" },
    RuntimeInfo { id: "kiro-cli", label: "Kiro CLI", global_path: ".kiro/skills" },
    RuntimeInfo { id: "roo", label: "Roo Code", global_path: ".roo/skills" },
    RuntimeInfo { id: "continue", label: "Continue", global_path: ".continue/skills" },
    RuntimeInfo { id: "hermes-agent", label: "Hermes Agent", global_path: ".hermes/skills" },
    RuntimeInfo { id: "trae", label: "Trae", global_path: ".trae/skills" },
    RuntimeInfo { id: "cline", label: "Cline", global_path: ".agents/skills" },
    RuntimeInfo { id: "goose", label: "Goose", global_path: ".config/goose/skills" },
    RuntimeInfo { id: "devin", label: "Devin", global_path: ".config/devin/skills" },
];

pub struct Database {
    conn: Connection,
}

impl Database {
    /// Open or create the database at the given path.
    pub fn open(db_path: &Path) -> rusqlite::Result<Self> {
        if let Some(parent) = db_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let conn = Connection::open(db_path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> rusqlite::Result<()> {
        let version: u32 = self.conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap_or(0);

        if version < 1 {
            self.conn.execute_batch(
                "
                CREATE TABLE IF NOT EXISTS skills (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    description TEXT DEFAULT '',
                    version TEXT DEFAULT '0.1.0',
                    source_workspace TEXT DEFAULT '',
                    source_path TEXT DEFAULT '',
                    source_branch TEXT DEFAULT '',
                    local_path TEXT NOT NULL,
                    link_mode TEXT DEFAULT 'symlink',
                    baseline_hash TEXT DEFAULT '',
                    published_hash TEXT DEFAULT '',
                    mtime_fingerprint TEXT DEFAULT '',
                    is_modified INTEGER DEFAULT 0,
                    installed_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS skill_targets (
                    skill_id TEXT NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
                    runtime TEXT NOT NULL,
                    enabled INTEGER DEFAULT 1,
                    target_path TEXT DEFAULT '',
                    PRIMARY KEY (skill_id, runtime)
                );

                CREATE TABLE IF NOT EXISTS subscriptions (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    workspace TEXT NOT NULL,
                    skill_id TEXT NOT NULL,
                    branch TEXT DEFAULT '',
                    channel TEXT DEFAULT 'stable',
                    version TEXT DEFAULT '',
                    update_policy TEXT DEFAULT 'manual',
                    subscribed_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS cache_entries (
                    key TEXT PRIMARY KEY,
                    workspace TEXT NOT NULL,
                    data BLOB,
                    fetched_at INTEGER NOT NULL
                );

                CREATE INDEX IF NOT EXISTS idx_cache_workspace ON cache_entries(workspace);
                CREATE INDEX IF NOT EXISTS idx_skill_targets_runtime ON skill_targets(runtime);
                CREATE INDEX IF NOT EXISTS idx_subscriptions_workspace ON subscriptions(workspace, skill_id);
                "
            )?;
            self.conn.execute_batch("PRAGMA user_version = 1;")?;
        }

        if version < 2 {
            // Download lifecycle state. A skill row may now exist before its
            // files are on disk (status='downloading'), so My Skills can show a
            // live progress bar and survive a restart. Existing rows predate the
            // async download path and are therefore already on disk → 'installed'.
            //   install_status: 'downloading' | 'installed' | 'error'
            //   download_progress: 0..=100 (best-effort; -1 = indeterminate)
            //   download_error: last failure message when status='error'
            self.conn.execute_batch(
                "
                ALTER TABLE skills ADD COLUMN install_status TEXT NOT NULL DEFAULT 'installed';
                ALTER TABLE skills ADD COLUMN download_progress INTEGER NOT NULL DEFAULT 100;
                ALTER TABLE skills ADD COLUMN download_error TEXT NOT NULL DEFAULT '';
                ",
            )?;
            self.conn.execute_batch("PRAGMA user_version = 2;")?;
        }

        if version < 3 {
            // Cached AI safety-review result, per skill. Stored alongside the
            // skill so it's cleaned up on delete (no orphan rows). reviewed_hash
            // is the content hash at review time: when it no longer matches the
            // skill's current hash, the cached verdict is shown as "stale" and
            // the user is nudged to re-review.
            //   review_verdict: '' | 'safe' | 'caution' | 'danger'
            //   review_summary: one/two-sentence summary
            //   review_findings_json: JSON array of {severity, detail}
            //   reviewed_at: RFC3339 timestamp of the last review ('' = never)
            //   reviewed_hash: content hash the review was run against
            self.conn.execute_batch(
                "
                ALTER TABLE skills ADD COLUMN review_verdict TEXT NOT NULL DEFAULT '';
                ALTER TABLE skills ADD COLUMN review_summary TEXT NOT NULL DEFAULT '';
                ALTER TABLE skills ADD COLUMN review_findings_json TEXT NOT NULL DEFAULT '';
                ALTER TABLE skills ADD COLUMN reviewed_at TEXT NOT NULL DEFAULT '';
                ALTER TABLE skills ADD COLUMN reviewed_hash TEXT NOT NULL DEFAULT '';
                ",
            )?;
            self.conn
                .execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION};"))?;
        }

        Ok(())
    }

    // -------------------------------------------------------------------------
    // Skills CRUD
    // -------------------------------------------------------------------------

    pub fn insert_skill(
        &self,
        id: &str,
        name: &str,
        description: &str,
        version: &str,
        source_workspace: &str,
        source_path: &str,
        source_branch: &str,
        local_path: &str,
        link_mode: &str,
        baseline_hash: &str,
    ) -> rusqlite::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let mtime = collect_mtime_fingerprint(Path::new(local_path));
        self.conn.execute(
            "INSERT OR REPLACE INTO skills (id, name, description, version, source_workspace, source_path, source_branch, local_path, link_mode, baseline_hash, published_hash, mtime_fingerprint, is_modified, installed_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10, ?11, 0, ?12, ?12)",
            params![id, name, description, version, source_workspace, source_path, source_branch, local_path, link_mode, baseline_hash, &mtime, &now],
        )?;
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Download lifecycle (async remote install)
    // -------------------------------------------------------------------------

    /// Insert (or reset) a skill row in the 'downloading' state *before* its
    /// files exist on disk, so My Skills can show a live progress bar
    /// immediately and the intent survives a restart. Real metadata (version,
    /// hashes) is filled in by [`finish_download`] on success.
    #[allow(clippy::too_many_arguments)]
    pub fn begin_download(
        &self,
        id: &str,
        name: &str,
        description: &str,
        version: &str,
        source_workspace: &str,
        source_path: &str,
        source_branch: &str,
        local_path: &str,
        link_mode: &str,
    ) -> rusqlite::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT OR REPLACE INTO skills
                (id, name, description, version, source_workspace, source_path, source_branch, local_path, link_mode, baseline_hash, published_hash, mtime_fingerprint, is_modified, installed_at, updated_at, install_status, download_progress, download_error)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, '', '', '', 0, ?10, ?10, 'downloading', 0, '')",
            params![id, name, description, version, source_workspace, source_path, source_branch, local_path, link_mode, &now],
        )?;
        Ok(())
    }

    /// Update download progress (0..=100, or -1 for an indeterminate stream).
    /// No-op once the row leaves the 'downloading' state.
    pub fn set_download_progress(&self, id: &str, progress: i64) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE skills SET download_progress = ?1 WHERE id = ?2 AND install_status = 'downloading'",
            params![progress, id],
        )?;
        Ok(())
    }

    /// Mark a download as finished: files are on disk, fill in the baseline hash
    /// and flip to 'installed'.
    pub fn finish_download(
        &self,
        id: &str,
        version: &str,
        local_path: &str,
        baseline_hash: &str,
    ) -> rusqlite::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let mtime = collect_mtime_fingerprint(Path::new(local_path));
        self.conn.execute(
            "UPDATE skills SET version = ?1, local_path = ?2, baseline_hash = ?3, published_hash = ?3, mtime_fingerprint = ?4, is_modified = 0, install_status = 'installed', download_progress = 100, download_error = '', updated_at = ?5 WHERE id = ?6",
            params![version, local_path, baseline_hash, &mtime, &now, id],
        )?;
        Ok(())
    }

    /// Mark a download as failed. The row is kept (status='error') so the user
    /// sees the failure and can retry, instead of the entry vanishing.
    pub fn fail_download(&self, id: &str, error: &str) -> rusqlite::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE skills SET install_status = 'error', download_error = ?1, updated_at = ?2 WHERE id = ?3",
            params![error, &now, id],
        )?;
        Ok(())
    }

    /// On startup, flip any lingering 'downloading' rows to 'error'. A tarball
    /// download can't resume across restarts, so an in-flight download that died
    /// with the app must surface as interrupted (retryable) rather than a bar
    /// stuck forever. Returns how many rows were reconciled.
    pub fn reconcile_interrupted_downloads(&self) -> rusqlite::Result<usize> {
        let now = chrono::Utc::now().to_rfc3339();
        let count = self.conn.execute(
            "UPDATE skills SET install_status = 'error', download_error = 'download interrupted (app was closed)', updated_at = ?1 WHERE install_status = 'downloading'",
            params![&now],
        )?;
        Ok(count)
    }

    // -------------------------------------------------------------------------
    // AI safety review cache
    // -------------------------------------------------------------------------

    /// Persist an AI review result for a skill, stamped with the content hash it
    /// was run against so we can later detect a stale verdict.
    pub fn save_review(
        &self,
        id: &str,
        verdict: &str,
        summary: &str,
        findings_json: &str,
        reviewed_hash: &str,
    ) -> rusqlite::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE skills SET review_verdict = ?1, review_summary = ?2, review_findings_json = ?3, reviewed_at = ?4, reviewed_hash = ?5 WHERE id = ?6",
            params![verdict, summary, findings_json, &now, reviewed_hash, id],
        )?;
        Ok(())
    }

    /// Clear any cached review for a skill (back to the "never reviewed" state).
    pub fn clear_review(&self, id: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE skills SET review_verdict = '', review_summary = '', review_findings_json = '', reviewed_at = '', reviewed_hash = '' WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn list_skills(&self) -> rusqlite::Result<Vec<SkillRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, description, version, source_workspace, source_path, source_branch, local_path, link_mode, baseline_hash, published_hash, is_modified, installed_at, updated_at, install_status, download_progress, download_error, review_verdict, review_summary, review_findings_json, reviewed_at, reviewed_hash FROM skills ORDER BY name"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(SkillRow {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                version: row.get(3)?,
                source_workspace: row.get(4)?,
                source_path: row.get(5)?,
                source_branch: row.get(6)?,
                local_path: row.get(7)?,
                link_mode: row.get(8)?,
                baseline_hash: row.get(9)?,
                published_hash: row.get(10)?,
                is_modified: row.get::<_, i32>(11)? != 0,
                installed_at: row.get(12)?,
                updated_at: row.get(13)?,
                install_status: row.get(14)?,
                download_progress: row.get(15)?,
                download_error: row.get(16)?,
                review_verdict: row.get(17)?,
                review_summary: row.get(18)?,
                review_findings_json: row.get(19)?,
                reviewed_at: row.get(20)?,
                reviewed_hash: row.get(21)?,
            })
        })?;
        rows.collect()
    }

    pub fn get_skill(&self, id: &str) -> rusqlite::Result<Option<SkillRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, description, version, source_workspace, source_path, source_branch, local_path, link_mode, baseline_hash, published_hash, is_modified, installed_at, updated_at, install_status, download_progress, download_error, review_verdict, review_summary, review_findings_json, reviewed_at, reviewed_hash FROM skills WHERE id = ?1"
        )?;
        let mut rows = stmt.query_map(params![id], |row| {
            Ok(SkillRow {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                version: row.get(3)?,
                source_workspace: row.get(4)?,
                source_path: row.get(5)?,
                source_branch: row.get(6)?,
                local_path: row.get(7)?,
                link_mode: row.get(8)?,
                baseline_hash: row.get(9)?,
                published_hash: row.get(10)?,
                is_modified: row.get::<_, i32>(11)? != 0,
                installed_at: row.get(12)?,
                updated_at: row.get(13)?,
                install_status: row.get(14)?,
                download_progress: row.get(15)?,
                download_error: row.get(16)?,
                review_verdict: row.get(17)?,
                review_summary: row.get(18)?,
                review_findings_json: row.get(19)?,
                reviewed_at: row.get(20)?,
                reviewed_hash: row.get(21)?,
            })
        })?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    pub fn remove_skill(&self, id: &str) -> rusqlite::Result<()> {
        self.conn.execute("DELETE FROM skills WHERE id = ?1", params![id])?;
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Skill targets (runtime enable/disable)
    // -------------------------------------------------------------------------

    pub fn set_target_enabled(&self, skill_id: &str, runtime: &str, enabled: bool, target_path: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO skill_targets (skill_id, runtime, enabled, target_path) VALUES (?1, ?2, ?3, ?4)",
            params![skill_id, runtime, enabled as i32, target_path],
        )?;
        Ok(())
    }

    pub fn get_targets_for_skill(&self, skill_id: &str) -> rusqlite::Result<Vec<SkillTargetRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT skill_id, runtime, enabled, target_path FROM skill_targets WHERE skill_id = ?1"
        )?;
        let rows = stmt.query_map(params![skill_id], |row| {
            Ok(SkillTargetRow {
                skill_id: row.get(0)?,
                runtime: row.get(1)?,
                enabled: row.get::<_, i32>(2)? != 0,
                target_path: row.get(3)?,
            })
        })?;
        rows.collect()
    }

    pub fn get_all_targets(&self) -> rusqlite::Result<Vec<SkillTargetRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT skill_id, runtime, enabled, target_path FROM skill_targets ORDER BY skill_id, runtime"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(SkillTargetRow {
                skill_id: row.get(0)?,
                runtime: row.get(1)?,
                enabled: row.get::<_, i32>(2)? != 0,
                target_path: row.get(3)?,
            })
        })?;
        rows.collect()
    }

    // -------------------------------------------------------------------------
    // Cache operations
    // -------------------------------------------------------------------------

    pub fn get_cache(&self, key: &str) -> rusqlite::Result<Option<Vec<u8>>> {
        let mut stmt = self.conn.prepare("SELECT data FROM cache_entries WHERE key = ?1")?;
        let mut rows = stmt.query_map(params![key], |row| row.get::<_, Vec<u8>>(0))?;
        match rows.next() {
            Some(data) => Ok(Some(data?)),
            None => Ok(None),
        }
    }

    pub fn put_cache(&self, key: &str, workspace: &str, data: &[u8]) -> rusqlite::Result<()> {
        let now = chrono::Utc::now().timestamp();
        self.conn.execute(
            "INSERT OR REPLACE INTO cache_entries (key, workspace, data, fetched_at) VALUES (?1, ?2, ?3, ?4)",
            params![key, workspace, data, now],
        )?;
        Ok(())
    }

    pub fn clear_cache_for_workspace(&self, workspace: &str) -> rusqlite::Result<()> {
        self.conn.execute("DELETE FROM cache_entries WHERE workspace = ?1", params![workspace])?;
        Ok(())
    }

    pub fn clear_all_cache(&self) -> rusqlite::Result<()> {
        self.conn.execute("DELETE FROM cache_entries", [])?;
        Ok(())
    }

    pub fn delete_cache(&self, key: &str) -> rusqlite::Result<()> {
        self.conn.execute("DELETE FROM cache_entries WHERE key = ?1", params![key])?;
        Ok(())
    }

    pub fn delete_cache_by_prefix(&self, prefix: &str) -> rusqlite::Result<usize> {
        let count = self.conn.execute(
            "DELETE FROM cache_entries WHERE key LIKE ?1",
            params![format!("{}%", prefix)],
        )?;
        Ok(count)
    }

    pub fn cache_size_by_workspace(&self) -> rusqlite::Result<Vec<CacheSizeRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT workspace, COUNT(*) as count, SUM(LENGTH(data)) as total_bytes FROM cache_entries GROUP BY workspace ORDER BY total_bytes DESC"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(CacheSizeRow {
                workspace: row.get(0)?,
                count: row.get(1)?,
                total_bytes: row.get::<_, i64>(2).unwrap_or(0),
            })
        })?;
        rows.collect()
    }
}

// -------------------------------------------------------------------------
// Row types
// -------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SkillRow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub source_workspace: String,
    pub source_path: String,
    pub source_branch: String,
    pub local_path: String,
    pub link_mode: String,
    pub baseline_hash: String,
    pub published_hash: String,
    pub is_modified: bool,
    pub installed_at: String,
    pub updated_at: String,
    pub install_status: String,
    pub download_progress: i64,
    pub download_error: String,
    pub review_verdict: String,
    pub review_summary: String,
    pub review_findings_json: String,
    pub reviewed_at: String,
    pub reviewed_hash: String,
}

#[derive(Debug, Clone)]
pub struct SkillTargetRow {
    pub skill_id: String,
    pub runtime: String,
    pub enabled: bool,
    pub target_path: String,
}

#[derive(Debug, Clone)]
pub struct CacheSizeRow {
    pub workspace: String,
    pub count: i64,
    pub total_bytes: i64,
}

// -------------------------------------------------------------------------
// Content hash & modification detection (mtime pre-check + hash)
// -------------------------------------------------------------------------

/// Collect mtime fingerprint of a directory: "path:mtime_secs\n" for all files, sorted.
pub fn collect_mtime_fingerprint(dir: &Path) -> String {
    let mut entries: Vec<(String, u64)> = Vec::new();

    fn walk(dir: &Path, base: &Path, out: &mut Vec<(String, u64)>) {
        let Ok(read) = fs::read_dir(dir) else { return };
        for entry in read.flatten() {
            let path = entry.path();
            if path.file_name().map(|n| n.to_string_lossy().starts_with('.')).unwrap_or(false) {
                continue;
            }
            if path.is_dir() {
                walk(&path, base, out);
            } else {
                let rel = path.strip_prefix(base).unwrap_or(&path).to_string_lossy().to_string();
                let mtime = path.metadata()
                    .and_then(|m| m.modified())
                    .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs())
                    .unwrap_or(0);
                out.push((rel, mtime));
            }
        }
    }

    walk(dir, dir, &mut entries);
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries.iter().map(|(p, t)| format!("{p}:{t}")).collect::<Vec<_>>().join("\n")
}

/// Compute a SHA-256 hash of all files in a directory (sorted by path for determinism).
/// Only called when mtime fingerprint indicates a change.
pub fn compute_dir_hash(dir: &Path) -> String {
    let mut hasher = Sha256::new();
    let mut entries: Vec<PathBuf> = Vec::new();

    fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(read) = fs::read_dir(dir) else { return };
        for entry in read.flatten() {
            let path = entry.path();
            if path.file_name().map(|n| n.to_string_lossy().starts_with('.')).unwrap_or(false) {
                continue;
            }
            if path.is_dir() {
                collect_files(&path, out);
            } else {
                out.push(path);
            }
        }
    }

    collect_files(dir, &mut entries);
    entries.sort();

    for path in &entries {
        if let Ok(rel) = path.strip_prefix(dir) {
            hasher.update(rel.to_string_lossy().as_bytes());
        }
        if let Ok(content) = fs::read(path) {
            hasher.update(&content);
        }
    }

    format!("{:x}", hasher.finalize())
}

impl Database {
    /// Fast modification check using mtime fingerprint.
    /// Only computes full hash if mtime changed.
    pub fn check_modified(&self, id: &str, local_path: &str) -> rusqlite::Result<bool> {
        let skill = match self.get_skill(id)? {
            Some(s) => s,
            None => return Ok(false),
        };
        if skill.baseline_hash.is_empty() {
            return Ok(false);
        }

        // Step 1: Quick mtime check
        let current_mtime = collect_mtime_fingerprint(Path::new(local_path));
        let stored_mtime: Option<String> = self.conn
            .query_row(
                "SELECT mtime_fingerprint FROM skills WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .ok();

        if let Some(ref stored) = stored_mtime {
            if *stored == current_mtime {
                // mtime unchanged → definitely not modified
                return Ok(skill.is_modified);
            }
        }

        // Step 2: mtime changed → compute full hash to confirm
        let current_hash = compute_dir_hash(Path::new(local_path));
        let modified = current_hash != skill.baseline_hash;

        // Update stored mtime and is_modified
        let now = chrono::Utc::now().to_rfc3339();
        let _ = self.conn.execute(
            "UPDATE skills SET mtime_fingerprint = ?1, is_modified = ?2, updated_at = ?3 WHERE id = ?4",
            params![&current_mtime, modified as i32, &now, id],
        );

        Ok(modified)
    }

    /// Mark a skill as modified or not.
    pub fn set_modified(&self, id: &str, modified: bool) -> rusqlite::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE skills SET is_modified = ?1, updated_at = ?2 WHERE id = ?3",
            params![modified as i32, &now, id],
        )?;
        Ok(())
    }

    /// After publishing: update version, reset hash, clear modified flag.
    pub fn mark_published(&self, id: &str, new_version: &str) -> rusqlite::Result<()> {
        let skill = match self.get_skill(id)? {
            Some(s) => s,
            None => return Ok(()),
        };
        let current_hash = compute_dir_hash(Path::new(&skill.local_path));
        let mtime = collect_mtime_fingerprint(Path::new(&skill.local_path));
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE skills SET version = ?1, baseline_hash = ?2, published_hash = ?2, mtime_fingerprint = ?3, is_modified = 0, updated_at = ?4 WHERE id = ?5",
            params![new_version, &current_hash, &mtime, &now, id],
        )?;
        Ok(())
    }

    /// After receiving a subscription update: overwrite local files, reset hash.
    pub fn mark_updated_from_remote(&self, id: &str, new_version: &str) -> rusqlite::Result<()> {
        let skill = match self.get_skill(id)? {
            Some(s) => s,
            None => return Ok(()),
        };
        let current_hash = compute_dir_hash(Path::new(&skill.local_path));
        let mtime = collect_mtime_fingerprint(Path::new(&skill.local_path));
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE skills SET version = ?1, baseline_hash = ?2, published_hash = ?2, mtime_fingerprint = ?3, is_modified = 0, updated_at = ?4 WHERE id = ?5",
            params![new_version, &current_hash, &mtime, &now, id],
        )?;
        Ok(())
    }

    /// Unmanage a skill: remove from registry, restore real files to IDE directories.
    pub fn unmanage_skill(&self, id: &str) -> rusqlite::Result<Option<SkillRow>> {
        let skill = self.get_skill(id)?;
        if let Some(ref s) = skill {
            // Get all targets so caller can restore symlinks to real copies
            let _ = self.conn.execute("DELETE FROM skill_targets WHERE skill_id = ?1", params![id]);
            let _ = self.conn.execute("DELETE FROM skills WHERE id = ?1", params![id]);
            // Note: caller is responsible for filesystem operations (replace symlinks with copies)
        }
        Ok(skill)
    }

    /// Bump version string (semver-like).
    pub fn bump_version(current: &str, bump_type: &str) -> String {
        let parts: Vec<u32> = current
            .split('.')
            .filter_map(|s| s.parse().ok())
            .collect();
        let (major, minor, patch) = match parts.as_slice() {
            [a, b, c, ..] => (*a, *b, *c),
            [a, b] => (*a, *b, 0),
            [a] => (*a, 0, 0),
            _ => (0, 1, 0),
        };
        match bump_type {
            "major" => format!("{}.0.0", major + 1),
            "minor" => format!("{}.{}.0", major, minor + 1),
            _ => format!("{}.{}.{}", major, minor, patch + 1), // patch
        }
    }
}

// -------------------------------------------------------------------------
// Symlink / Copy operations
// -------------------------------------------------------------------------

/// Create a symlink or copy from source to target based on link_mode.
pub fn link_skill(source: &Path, target: &Path, link_mode: &str) -> std::io::Result<()> {
    if target.exists() || target.is_symlink() {
        // Already linked/copied
        return Ok(());
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    match link_mode {
        "copy" => copy_dir_recursive(source, target),
        _ => {
            // symlink (default)
            #[cfg(unix)]
            {
                std::os::unix::fs::symlink(source, target)
            }
            #[cfg(windows)]
            {
                // Windows: try junction first (no admin needed), fall back to copy
                if std::os::windows::fs::symlink_dir(source, target).is_err() {
                    copy_dir_recursive(source, target)
                } else {
                    Ok(())
                }
            }
        }
    }
}

/// Remove a symlink or copied directory.
pub fn unlink_skill(target: &Path) -> std::io::Result<()> {
    if target.is_symlink() {
        #[cfg(unix)]
        {
            fs::remove_file(target)?;
        }
        #[cfg(windows)]
        {
            fs::remove_dir(target)?;
        }
    } else if target.is_dir() {
        fs::remove_dir_all(target)?;
    }
    Ok(())
}

pub fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

/// Resolve the global skills directory for a runtime.
pub fn resolve_runtime_global_path(home: &Path, runtime_id: &str) -> Option<PathBuf> {
    SUPPORTED_RUNTIMES
        .iter()
        .find(|r| r.id == runtime_id)
        .map(|r| home.join(r.global_path))
}

/// Scan all runtime directories and return discovered skills not in our registry.
pub fn scan_unmanaged_skills(home: &Path, db: &Database) -> Vec<UnmanagedSkill> {
    let managed_ids: std::collections::HashSet<String> = db
        .list_skills()
        .unwrap_or_default()
        .into_iter()
        .map(|s| s.id)
        .collect();

    let mut found: std::collections::HashMap<String, UnmanagedSkill> = std::collections::HashMap::new();

    for runtime in SUPPORTED_RUNTIMES {
        let dir = home.join(runtime.global_path);
        if !dir.is_dir() {
            continue;
        }
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let id = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            if id.starts_with('.') || managed_ids.contains(&id) {
                continue;
            }
            let skill = found.entry(id.clone()).or_insert_with(|| UnmanagedSkill {
                id: id.clone(),
                name: humanize_name(&id),
                path: path.clone(),
                found_in: Vec::new(),
            });
            skill.found_in.push(runtime.id.to_owned());
        }
    }

    let mut result: Vec<UnmanagedSkill> = found.into_values().collect();
    result.sort_by(|a, b| a.name.cmp(&b.name));
    result
}

#[derive(Debug, Clone)]
pub struct UnmanagedSkill {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub found_in: Vec<String>,
}

fn humanize_name(id: &str) -> String {
    id.split(['-', '_'])
        .filter(|s| !s.is_empty())
        .map(|s| {
            let mut chars = s.chars();
            match chars.next() {
                Some(c) => format!("{}{}", c.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
