use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
#[cfg(target_os = "macos")]
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use tauri::http::{header, Request, Response, StatusCode};

const ICON_SCHEME: &str = "appicon";
const ICON_CACHE_LIMIT: usize = 96;
const ICON_CACHE_VERSION: &str = "5";
const ICON_RASTER_SCALE: u16 = 2;

#[derive(Debug, Clone, Copy)]
pub struct PathOpenerCandidate {
    pub id: &'static str,
    pub label: &'static str,
    pub app_name: Option<&'static str>,
    pub cli_names: &'static [&'static str],
    pub bundle_cli_paths: &'static [&'static str],
}

#[derive(Debug, Clone, Copy)]
pub enum IconSize {
    Small,
    Default,
    Large,
}

impl IconSize {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Default => "default",
            Self::Large => "large",
        }
    }

    fn from_query(value: Option<&str>) -> Self {
        match value {
            Some("small") => Self::Small,
            Some("large") => Self::Large,
            _ => Self::Default,
        }
    }

    #[cfg(any(target_os = "macos", test))]
    fn css_pixels(self) -> u16 {
        match self {
            Self::Small => 20,
            Self::Default => 32,
            Self::Large => 64,
        }
    }

    #[cfg(any(target_os = "macos", test))]
    fn raster_pixels(self) -> u16 {
        self.css_pixels() * ICON_RASTER_SCALE
    }
}

pub const PATH_OPENER_CANDIDATES: &[PathOpenerCandidate] = &[
    PathOpenerCandidate {
        id: "vscode",
        label: "VS Code",
        app_name: Some("Visual Studio Code"),
        cli_names: &["code"],
        bundle_cli_paths: &["Contents/Resources/app/bin/code"],
    },
    PathOpenerCandidate {
        id: "cursor",
        label: "Cursor",
        app_name: Some("Cursor"),
        cli_names: &["cursor"],
        bundle_cli_paths: &["Contents/Resources/app/bin/cursor"],
    },
    PathOpenerCandidate {
        id: "zed",
        label: "Zed",
        app_name: Some("Zed"),
        cli_names: &["zed"],
        bundle_cli_paths: &["Contents/MacOS/cli", "Contents/MacOS/zed"],
    },
    PathOpenerCandidate {
        id: "finder",
        label: "Finder",
        app_name: Some("Finder"),
        cli_names: &[],
        bundle_cli_paths: &[],
    },
    PathOpenerCandidate {
        id: "terminal",
        label: "Terminal",
        app_name: Some("Terminal"),
        cli_names: &[],
        bundle_cli_paths: &[],
    },
    PathOpenerCandidate {
        id: "warp",
        label: "Warp",
        app_name: Some("Warp"),
        cli_names: &[],
        bundle_cli_paths: &[],
    },
    PathOpenerCandidate {
        id: "xcode",
        label: "Xcode",
        app_name: Some("Xcode"),
        cli_names: &["xed"],
        bundle_cli_paths: &["Contents/Developer/usr/bin/xed"],
    },
];

#[derive(Clone)]
struct CachedIcon {
    mime: &'static str,
    bytes: Vec<u8>,
}

#[derive(Default)]
struct IconLru {
    map: HashMap<String, CachedIcon>,
    order: VecDeque<String>,
}

static ICON_CACHE: OnceLock<Mutex<IconLru>> = OnceLock::new();
static ICON_LOAD_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

pub fn candidates() -> &'static [PathOpenerCandidate] {
    PATH_OPENER_CANDIDATES
}

pub fn find_candidate(token: &str) -> Option<&'static PathOpenerCandidate> {
    PATH_OPENER_CANDIDATES.iter().find(|candidate| {
        candidate.id == token
            || candidate.label == token
            || candidate.app_name.is_some_and(|app_name| app_name == token)
    })
}

pub fn icon_url(id: &str, size: IconSize) -> String {
    #[cfg(target_os = "windows")]
    {
        format!(
            "http://{ICON_SCHEME}.localhost/{id}?size={}&scale={ICON_RASTER_SCALE}&v={ICON_CACHE_VERSION}",
            size.as_str()
        )
    }

    #[cfg(not(target_os = "windows"))]
    {
        format!(
            "{ICON_SCHEME}://localhost/{id}?size={}&scale={ICON_RASTER_SCALE}&v={ICON_CACHE_VERSION}",
            size.as_str()
        )
    }
}

pub fn handle_icon_request(request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    let path = request.uri().path().trim_start_matches('/');
    let id = path.split('/').next().unwrap_or_default();
    let size = IconSize::from_query(query_param(request.uri().query(), "size").as_deref());
    let key = format!("{id}:{}:{ICON_RASTER_SCALE}x", size.as_str());

    if let Some(icon) = cache_get(&key) {
        return icon_response(icon);
    }

    let load_lock = ICON_LOAD_LOCK.get_or_init(|| Mutex::new(()));
    let _load_guard = load_lock.lock().ok();
    if let Some(icon) = cache_get(&key) {
        return icon_response(icon);
    }

    match load_icon(id, size) {
        Some(icon) => {
            cache_put(key, icon.clone());
            icon_response(icon)
        }
        None => error_response(StatusCode::NOT_FOUND, "icon not found"),
    }
}

pub fn find_app_bundle(app_name: &str) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let bundle = format!("{app_name}.app");
        let mut paths = vec![
            PathBuf::from("/Applications").join(&bundle),
            PathBuf::from("/System/Applications").join(&bundle),
            PathBuf::from("/System/Applications/Utilities").join(&bundle),
            PathBuf::from("/System/Library/CoreServices").join(&bundle),
        ];
        if let Some(home) = dirs::home_dir() {
            paths.push(home.join("Applications").join(&bundle));
        }
        paths.into_iter().find(|path| path.exists())
    }

    #[cfg(target_os = "windows")]
    {
        let candidate = candidate_by_app_name(app_name)?;
        find_windows_app_dir(candidate)
    }

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        let _ = app_name;
        None
    }
}

fn load_icon(id: &str, size: IconSize) -> Option<CachedIcon> {
    let candidate = find_candidate(id)?;
    let app_name = candidate.app_name?;
    let bundle_path = find_app_bundle(app_name)?;
    platform_icon(candidate, &bundle_path, size)
}

fn platform_icon(
    candidate: &PathOpenerCandidate,
    bundle_path: &Path,
    size: IconSize,
) -> Option<CachedIcon> {
    #[cfg(target_os = "macos")]
    {
        let _ = candidate;
        macos_icon(bundle_path, size)
    }

    #[cfg(target_os = "windows")]
    {
        let _ = size;
        windows_icon(candidate, bundle_path)
    }

    #[cfg(target_os = "linux")]
    {
        let _ = (candidate, bundle_path, size);
        None
    }
}

#[cfg(target_os = "windows")]
fn candidate_by_app_name(app_name: &str) -> Option<&'static PathOpenerCandidate> {
    PATH_OPENER_CANDIDATES
        .iter()
        .find(|candidate| candidate.app_name == Some(app_name))
}

#[cfg(target_os = "windows")]
pub fn find_candidate_app_exe(candidate: &PathOpenerCandidate) -> Option<PathBuf> {
    let app_name = candidate.app_name?;
    let app_dir = find_app_bundle(app_name)?;
    windows_exe_names(candidate)
        .iter()
        .map(|name| app_dir.join(name))
        .find(|path| path.is_file())
}

#[cfg(target_os = "windows")]
fn find_windows_app_dir(candidate: &PathOpenerCandidate) -> Option<PathBuf> {
    find_windows_app_dir_from_path(candidate)
        .or_else(|| find_windows_app_dir_from_known_roots(candidate))
}

#[cfg(target_os = "windows")]
fn find_windows_app_dir_from_path(candidate: &PathOpenerCandidate) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for root in std::env::split_paths(&path_var) {
        if !windows_cli_root_matches(candidate, &root) {
            continue;
        }
        let mut current = Some(root.as_path());
        for _ in 0..5 {
            let Some(path) = current else {
                break;
            };
            if windows_app_dir_has_exe(candidate, path) {
                return Some(path.to_path_buf());
            }
            current = path.parent();
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn find_windows_app_dir_from_known_roots(candidate: &PathOpenerCandidate) -> Option<PathBuf> {
    let mut roots = Vec::new();
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        roots.push(PathBuf::from(local_app_data).join("Programs"));
    }
    if let Some(program_files) = std::env::var_os("ProgramFiles") {
        roots.push(PathBuf::from(program_files));
    }
    if let Some(program_files_x86) = std::env::var_os("ProgramFiles(x86)") {
        roots.push(PathBuf::from(program_files_x86));
    }

    for root in roots {
        for dir_name in windows_app_dir_names(candidate) {
            let path = root.join(dir_name);
            if windows_app_dir_has_exe(candidate, &path) {
                return Some(path);
            }
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn windows_cli_root_matches(candidate: &PathOpenerCandidate, root: &Path) -> bool {
    candidate.cli_names.iter().any(|name| {
        root.join(name).exists()
            || windows_launchable_suffixes()
                .iter()
                .any(|suffix| root.join(format!("{name}{suffix}")).exists())
    })
}

#[cfg(target_os = "windows")]
fn windows_app_dir_has_exe(candidate: &PathOpenerCandidate, path: &Path) -> bool {
    windows_exe_names(candidate)
        .iter()
        .any(|name| path.join(name).is_file())
}

#[cfg(target_os = "windows")]
fn windows_app_dir_names(candidate: &PathOpenerCandidate) -> &'static [&'static str] {
    match candidate.id {
        "vscode" => &["Microsoft VS Code"],
        "cursor" => &["Cursor"],
        "zed" => &["Zed"],
        _ => &[],
    }
}

#[cfg(target_os = "windows")]
fn windows_exe_names(candidate: &PathOpenerCandidate) -> &'static [&'static str] {
    match candidate.id {
        "vscode" => &["Code.exe"],
        "cursor" => &["Cursor.exe"],
        "zed" => &["Zed.exe"],
        _ => &[],
    }
}

#[cfg(target_os = "windows")]
pub fn windows_launchable_suffixes() -> &'static [&'static str] {
    &[".exe", ".cmd", ".bat", ".com"]
}

#[cfg(target_os = "windows")]
fn windows_icon(candidate: &PathOpenerCandidate, app_dir: &Path) -> Option<CachedIcon> {
    for path in windows_manifest_icon_paths(candidate, app_dir) {
        if path.is_file() {
            return cached_icon_from_path(&path);
        }
    }

    for relative_path in windows_icon_paths(candidate) {
        let path = app_dir.join(relative_path);
        if path.is_file() {
            return cached_icon_from_path(&path);
        }
    }

    for resources_dir in windows_icon_resource_dirs(app_dir) {
        if let Some(icon) = fs::read_dir(resources_dir)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(|entry| entry.ok())
            .find_map(|entry| cached_icon_from_path(&entry.path()))
        {
            return Some(icon);
        }
    }

    None
}

#[cfg(target_os = "windows")]
fn windows_icon_paths(candidate: &PathOpenerCandidate) -> &'static [&'static str] {
    match candidate.id {
        "vscode" => &[
            "resources/app/resources/win32/code_70x70.png",
            "resources/app/resources/win32/code_150x150.png",
            "resources/app/resources/win32/code.ico",
        ],
        "cursor" => &[
            "resources/app/resources/win32/cursor_70x70.png",
            "resources/app/resources/win32/cursor_150x150.png",
            "resources/app/resources/win32/cursor.ico",
        ],
        "zed" => &["resources/app/resources/win32/zed.ico"],
        _ => &[],
    }
}

#[cfg(target_os = "windows")]
fn windows_manifest_icon_paths(candidate: &PathOpenerCandidate, app_dir: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for manifest_name in windows_visual_manifest_names(candidate) {
        let Ok(manifest) = fs::read_to_string(app_dir.join(manifest_name)) else {
            continue;
        };
        for attr in ["Square70x70Logo", "Square44x44Logo", "Square150x150Logo"] {
            let Some(value) = manifest_attr(&manifest, attr) else {
                continue;
            };
            let path = PathBuf::from(value);
            let path = if path.is_absolute() {
                path
            } else {
                app_dir.join(path)
            };
            if !paths.contains(&path) {
                paths.push(path);
            }
        }
    }
    paths
}

#[cfg(target_os = "windows")]
fn windows_visual_manifest_names(candidate: &PathOpenerCandidate) -> &'static [&'static str] {
    match candidate.id {
        "vscode" => &["Code.VisualElementsManifest.xml"],
        "cursor" => &["Cursor.VisualElementsManifest.xml"],
        "zed" => &["Zed.VisualElementsManifest.xml"],
        _ => &[],
    }
}

#[cfg(target_os = "windows")]
fn manifest_attr(xml: &str, attr: &str) -> Option<String> {
    let needle = format!("{attr}=");
    let mut offset = 0;
    while let Some(relative_index) = xml[offset..].find(&needle) {
        let index = offset + relative_index;
        if !xml[..index]
            .chars()
            .next_back()
            .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == ':')
        {
            let start = index + needle.len();
            let rest = xml[start..].trim_start();
            let quote = rest.chars().next()?;
            if quote != '"' && quote != '\'' {
                return None;
            }
            let value_start = quote.len_utf8();
            let value_end = rest[value_start..].find(quote)? + value_start;
            let value = rest[value_start..value_end].trim();
            return (!value.is_empty()).then(|| value.to_owned());
        }
        offset = index + needle.len();
    }
    None
}

#[cfg(target_os = "windows")]
fn windows_icon_resource_dirs(app_dir: &Path) -> Vec<PathBuf> {
    let relative = Path::new("resources")
        .join("app")
        .join("resources")
        .join("win32");
    let mut dirs = vec![app_dir.join(&relative)];
    if let Ok(entries) = fs::read_dir(app_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path.join(&relative));
            }
        }
    }
    dirs
}

#[cfg(target_os = "windows")]
fn cached_icon_from_path(path: &Path) -> Option<CachedIcon> {
    let mime = match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("ico") => "image/x-icon",
        Some(ext) if ext.eq_ignore_ascii_case("png") => "image/png",
        _ => return None,
    };
    Some(CachedIcon {
        mime,
        bytes: fs::read(path).ok()?,
    })
}

#[cfg(target_os = "macos")]
fn macos_icon(bundle_path: &Path, size: IconSize) -> Option<CachedIcon> {
    let cache_dir = std::env::temp_dir()
        .join("skill-library-app-icons")
        .join(format!("v{ICON_CACHE_VERSION}"))
        .join(format!("{}@{ICON_RASTER_SCALE}x", size.as_str()));
    fs::create_dir_all(&cache_dir).ok()?;
    let bundle_file_name = bundle_path.file_name()?.to_string_lossy().to_string();
    let expected_path = cache_dir.join(format!("{bundle_file_name}.png"));
    if !expected_path.exists() {
        let icon_source = macos_icon_source(bundle_path)?;
        let status = Command::new("/usr/bin/sips")
            .arg("-z")
            .arg(size.raster_pixels().to_string())
            .arg(size.raster_pixels().to_string())
            .arg(&icon_source)
            .args(["-s", "format", "png", "--out"])
            .arg(&expected_path)
            .status()
            .ok()?;
        if !status.success() {
            return None;
        }
    }
    Some(CachedIcon {
        mime: "image/png",
        bytes: fs::read(expected_path).ok()?,
    })
}

#[cfg(target_os = "macos")]
fn macos_icon_source(bundle_path: &Path) -> Option<PathBuf> {
    let resources_dir = bundle_path.join("Contents").join("Resources");
    if let Some(icon_file) = macos_info_plist_icon_file(bundle_path) {
        let path = resources_dir.join(icon_file);
        if path.is_file() {
            return Some(path);
        }
    }

    let bundle_stem = bundle_path.file_stem()?.to_string_lossy();
    let preferred = [
        format!("{bundle_stem}.icns"),
        "AppIcon.icns".to_owned(),
        "app.icns".to_owned(),
        "icon.icns".to_owned(),
    ];
    for file_name in preferred {
        let path = resources_dir.join(file_name);
        if path.is_file() {
            return Some(path);
        }
    }

    fs::read_dir(resources_dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .find_map(|entry| {
            let path = entry.path();
            (path.extension().and_then(|ext| ext.to_str()) == Some("icns")).then_some(path)
        })
}

#[cfg(target_os = "macos")]
fn macos_info_plist_icon_file(bundle_path: &Path) -> Option<String> {
    let plist = fs::read_to_string(bundle_path.join("Contents").join("Info.plist")).ok()?;
    let key_index = plist.find("<key>CFBundleIconFile</key>")?;
    let rest = &plist[key_index..];
    let string_start = rest.find("<string>")? + "<string>".len();
    let string_end = rest[string_start..].find("</string>")? + string_start;
    let value = rest[string_start..string_end].trim();
    if value.is_empty() {
        return None;
    }
    if value.ends_with(".icns") {
        Some(value.to_owned())
    } else {
        Some(format!("{value}.icns"))
    }
}

fn icon_response(icon: CachedIcon) -> Response<Vec<u8>> {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, icon.mime)
        .header(header::CACHE_CONTROL, "max-age=604800")
        .body(icon.bytes)
        .unwrap()
}

fn error_response(status: StatusCode, message: &'static str) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/plain")
        .body(message.as_bytes().to_vec())
        .unwrap()
}

fn cache_get(key: &str) -> Option<CachedIcon> {
    let cache = ICON_CACHE.get_or_init(|| Mutex::new(IconLru::default()));
    let mut cache = cache.lock().ok()?;
    let icon = cache.map.get(key)?.clone();
    cache.order.retain(|entry| entry != key);
    cache.order.push_back(key.to_owned());
    Some(icon)
}

fn cache_put(key: String, icon: CachedIcon) {
    let cache = ICON_CACHE.get_or_init(|| Mutex::new(IconLru::default()));
    let Ok(mut cache) = cache.lock() else {
        return;
    };
    if cache.map.contains_key(&key) {
        cache.order.retain(|entry| entry != &key);
    }
    cache.map.insert(key.clone(), icon);
    cache.order.push_back(key);
    while cache.order.len() > ICON_CACHE_LIMIT {
        if let Some(oldest) = cache.order.pop_front() {
            cache.map.remove(&oldest);
        }
    }
}

fn query_param(query: Option<&str>, key: &str) -> Option<String> {
    query?.split('&').find_map(|part| {
        let mut pieces = part.splitn(2, '=');
        let name = pieces.next()?;
        let value = pieces.next().unwrap_or_default();
        (name == key).then(|| value.to_owned())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_url_uses_registered_scheme_and_size() {
        #[cfg(target_os = "windows")]
        let expected_base = "http://appicon.localhost";
        #[cfg(not(target_os = "windows"))]
        let expected_base = "appicon://localhost";

        assert_eq!(
            icon_url("vscode", IconSize::Small),
            format!("{expected_base}/vscode?size=small&scale=2&v=5")
        );
        assert_eq!(
            icon_url("vscode", IconSize::Default),
            format!("{expected_base}/vscode?size=default&scale=2&v=5")
        );
        assert_eq!(
            icon_url("vscode", IconSize::Large),
            format!("{expected_base}/vscode?size=large&scale=2&v=5")
        );
    }

    #[test]
    fn protocol_handler_returns_not_found_for_unknown_icon() {
        let request = Request::builder()
            .uri("appicon://localhost/not-installed?size=small")
            .body(Vec::new())
            .unwrap();
        let response = handle_icon_request(request);
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("text/plain")
        );
    }

    #[test]
    fn protocol_handler_parses_large_size() {
        assert_eq!(IconSize::from_query(Some("large")).css_pixels(), 64);
        assert_eq!(IconSize::from_query(Some("small")).css_pixels(), 20);
        assert_eq!(IconSize::from_query(Some("default")).css_pixels(), 32);
    }

    #[test]
    fn icon_sizes_generate_double_density_rasters() {
        assert_eq!(IconSize::Small.raster_pixels(), 40);
        assert_eq!(IconSize::Default.raster_pixels(), 64);
        assert_eq!(IconSize::Large.raster_pixels(), 128);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn protocol_handler_generates_real_macos_finder_icon() {
        let request = Request::builder()
            .uri(icon_url("finder", IconSize::Small))
            .body(Vec::new())
            .unwrap();
        let response = handle_icon_request(request);
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("image/png")
        );
        assert!(response.body().starts_with(b"\x89PNG\r\n\x1a\n"));
        assert_eq!(png_dimensions(response.body()), Some((40, 40)));
    }

    #[cfg(target_os = "macos")]
    fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
        if bytes.len() < 24 || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            return None;
        }
        let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
        let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
        Some((width, height))
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn protocol_handler_loads_windows_vscode_icon_from_path_install() {
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let _guard = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let app_dir = dir.path().join("Microsoft VS Code");
        let bin_dir = app_dir.join("bin");
        let version_dir = app_dir.join("8761a5560c");
        let icon_path = app_dir
            .join("8761a5560c")
            .join("resources")
            .join("app")
            .join("resources")
            .join("win32")
            .join("code_70x70.png");
        fs::create_dir_all(&bin_dir).unwrap();
        fs::create_dir_all(icon_path.parent().unwrap()).unwrap();
        fs::write(app_dir.join("Code.exe"), b"placeholder exe").unwrap();
        fs::write(
            app_dir.join("Code.VisualElementsManifest.xml"),
            r#"<Application>
                <VisualElements
                    Square150x150Logo="8761a5560c\resources\app\resources\win32\code_150x150.png"
                    Square70x70Logo="8761a5560c\resources\app\resources\win32\code_70x70.png" />
            </Application>"#,
        )
        .unwrap();
        fs::write(bin_dir.join("code.cmd"), b"@echo off\r\n").unwrap();
        fs::create_dir_all(
            version_dir
                .join("resources")
                .join("app")
                .join("resources")
                .join("win32"),
        )
        .unwrap();
        fs::write(&icon_path, b"\x89PNG\r\n\x1a\nfake png").unwrap();

        let old_path = std::env::var_os("PATH");
        let path_var = std::env::join_paths([bin_dir]).unwrap();
        std::env::set_var("PATH", path_var);
        let request = Request::builder()
            .uri(icon_url("vscode", IconSize::Small))
            .body(Vec::new())
            .unwrap();
        let response = handle_icon_request(request);
        if let Some(value) = old_path {
            std::env::set_var("PATH", value);
        } else {
            std::env::remove_var("PATH");
        }

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("image/png")
        );
        assert_eq!(response.body(), b"\x89PNG\r\n\x1a\nfake png");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_manifest_attr_does_not_match_attribute_name_suffix() {
        let manifest = r#"<Application>
            <VisualElements
                ShowNameOnSquare150x150Logo="on"
                Square150x150Logo="8761a5560c\resources\app\resources\win32\code_150x150.png" />
        </Application>"#;

        assert_eq!(
            manifest_attr(manifest, "Square150x150Logo").as_deref(),
            Some(r"8761a5560c\resources\app\resources\win32\code_150x150.png")
        );
    }
}
