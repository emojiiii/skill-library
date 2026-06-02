use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use tauri::http::{header, Request, Response, StatusCode};

const ICON_SCHEME: &str = "appicon";
const ICON_CACHE_LIMIT: usize = 96;
const ICON_CACHE_VERSION: &str = "2";
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

    fn css_pixels(self) -> u16 {
        match self {
            Self::Small => 20,
            Self::Default => 32,
            Self::Large => 64,
        }
    }

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
    format!(
        "{ICON_SCHEME}://localhost/{id}?size={}&scale={ICON_RASTER_SCALE}&v={ICON_CACHE_VERSION}",
        size.as_str()
    )
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

    #[cfg(not(target_os = "macos"))]
    {
        let _ = app_name;
        None
    }
}

fn load_icon(id: &str, size: IconSize) -> Option<CachedIcon> {
    let candidate = find_candidate(id)?;
    let app_name = candidate.app_name?;
    let bundle_path = find_app_bundle(app_name)?;
    platform_icon(&bundle_path, size)
}

fn platform_icon(bundle_path: &Path, size: IconSize) -> Option<CachedIcon> {
    #[cfg(target_os = "macos")]
    {
        macos_icon(bundle_path, size)
    }

    #[cfg(target_os = "windows")]
    {
        let _ = (bundle_path, size);
        None
    }

    #[cfg(target_os = "linux")]
    {
        let _ = (bundle_path, size);
        None
    }
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
        assert_eq!(
            icon_url("vscode", IconSize::Small),
            "appicon://localhost/vscode?size=small&scale=2&v=2"
        );
        assert_eq!(
            icon_url("vscode", IconSize::Default),
            "appicon://localhost/vscode?size=default&scale=2&v=2"
        );
        assert_eq!(
            icon_url("vscode", IconSize::Large),
            "appicon://localhost/vscode?size=large&scale=2&v=2"
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
}
