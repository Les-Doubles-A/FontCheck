#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;

use serde::{Deserialize, Serialize};
use tao::{
    dpi::{LogicalSize, Size},
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy},
    window::{Icon, WindowBuilder},
};
use wry::{
    http::{header::CONTENT_TYPE, Response},
    WebContext, WebViewBuilder,
};

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const GITHUB_REPO: &str = "Les-Doubles-A/FontCheck";

const INDEX_HTML: &[u8] = include_bytes!("../index.html");
const STYLE_CSS: &[u8] = include_bytes!("../style.css");
const APP_JS: &[u8] = include_bytes!("../app.js");
const LOGO_PNG: &[u8] = include_bytes!("../FontCheck.png");
const LOGO_ICO: &[u8] = include_bytes!("../FontCheck.ico");

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UpdateInfo {
    available: bool,
    current_version: String,
    latest_version: String,
    release_url: String,
    download_url: Option<String>,
    notes: String,
    error: Option<String>,
}

#[derive(Debug, Clone)]
enum UserEvent {
    FontsRefreshed(Vec<String>),
    UpdateChecked(UpdateInfo, bool),
    UpdateInstallResult(Result<(), String>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppStateData {
    fonts: Vec<String>,
    licenses: BTreeMap<String, String>,
    favorites: Vec<String>,
    version: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
enum IpcCommand {
    RefreshFonts,
    SetLicense { family: String, status: String },
    ToggleFavorite { family: String },
    CheckUpdate { manual: bool },
    InstallUpdate { download_url: String, release_url: String },
    OpenExternal { url: String },
}

struct SharedState {
    licenses: BTreeMap<String, String>,
    favorites: BTreeSet<String>,
}

fn data_dir() -> PathBuf {
    let base = env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| env::var_os("USERPROFILE").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("FontCheck");
    let _ = fs::create_dir_all(&dir);
    dir
}

fn licenses_path() -> PathBuf {
    data_dir().join("licenses.json")
}

fn favorites_path() -> PathBuf {
    data_dir().join("favorites.json")
}

fn load_licenses() -> BTreeMap<String, String> {
    let valid = ["inconnu", "personnel", "commercial", "libre"];
    let Ok(raw) = fs::read_to_string(licenses_path()) else {
        return BTreeMap::new();
    };
    let Ok(map) = serde_json::from_str::<BTreeMap<String, String>>(&raw) else {
        return BTreeMap::new();
    };
    map.into_iter()
        .filter(|(_, v)| valid.contains(&v.as_str()) && v != "inconnu")
        .collect()
}

fn save_licenses(map: &BTreeMap<String, String>) {
    if let Ok(json) = serde_json::to_string_pretty(map) {
        let _ = fs::write(licenses_path(), json);
    }
}

fn load_favorites() -> BTreeSet<String> {
    let Ok(raw) = fs::read_to_string(favorites_path()) else {
        return BTreeSet::new();
    };
    serde_json::from_str::<Vec<String>>(&raw)
        .unwrap_or_default()
        .into_iter()
        .collect()
}

fn save_favorites(set: &BTreeSet<String>) {
    let list: Vec<&String> = set.iter().collect();
    if let Ok(json) = serde_json::to_string_pretty(&list) {
        let _ = fs::write(favorites_path(), json);
    }
}

/// Fast native Windows font enumeration combining GDI `EnumFontFamiliesExW` and Registry keys
fn enumerate_windows_fonts() -> Vec<String> {
    let mut families: BTreeMap<String, String> = BTreeMap::new();

    let mut insert_family = |raw: String| {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('@') {
            return;
        }
        let lower = trimmed.to_lowercase();
        families.entry(lower).or_insert_with(|| trimmed.to_string());
    };

    // 1. GDI EnumFontFamiliesExW
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::Graphics::Gdi::{
            CreateCompatibleDC, DeleteDC, EnumFontFamiliesExW, DEFAULT_CHARSET, LOGFONTW, TEXTMETRICW,
        };

        unsafe extern "system" fn enum_proc(
            lpelfe: *const LOGFONTW,
            _lpntme: *const TEXTMETRICW,
            _font_type: u32,
            lparam: isize,
        ) -> i32 {
            if !lpelfe.is_null() {
                let lf = unsafe { &*lpelfe };
                let len = lf
                    .lfFaceName
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(lf.lfFaceName.len());
                let name = String::from_utf16_lossy(&lf.lfFaceName[..len]);
                let vec = unsafe { &mut *(lparam as *mut Vec<String>) };
                vec.push(name);
            }
            1
        }

        let hdc = CreateCompatibleDC(std::ptr::null_mut());
        if !hdc.is_null() {
            let mut gdi_names: Vec<String> = Vec::with_capacity(512);
            let mut lf: LOGFONTW = std::mem::zeroed();
            lf.lfCharSet = DEFAULT_CHARSET as u8;
            EnumFontFamiliesExW(
                hdc,
                &lf,
                Some(enum_proc),
                &mut gdi_names as *mut Vec<String> as isize,
                0,
            );
            DeleteDC(hdc);
            for name in gdi_names {
                insert_family(name);
            }
        }
    }

    // 2. Windows Registry (HKLM + HKCU)
    #[cfg(windows)]
    {
        use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
        use winreg::RegKey;

        let font_subkey = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts";
        for root in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
            if let Ok(key) = RegKey::predef(root).open_subkey(font_subkey) {
                for item in key.enum_values().flatten() {
                    let label = item.0;
                    let mut family = label
                        .strip_suffix(" (TrueType)")
                        .or_else(|| label.strip_suffix(" (OpenType)"))
                        .unwrap_or(&label)
                        .to_string();
                    for suffix in [" Bold Italic", " Bold", " Italic", " Regular"] {
                        if let Some(stripped) = family.strip_suffix(suffix) {
                            family = stripped.to_string();
                        }
                    }
                    insert_family(family);
                }
            }
        }
    }

    let mut result: Vec<String> = families.into_values().collect();
    result.sort_by_cached_key(|s| s.to_lowercase());
    result
}

fn parse_version(v: &str) -> Vec<u32> {
    let cleaned = v.trim().trim_start_matches(['v', 'V']);
    let core = cleaned.split(['-', '+']).next().unwrap_or(cleaned);
    core.split('.')
        .map(|part| part.parse::<u32>().unwrap_or(0))
        .collect()
}

fn is_newer_version(current: &str, latest: &str) -> bool {
    let cur = parse_version(current);
    let lat = parse_version(latest);
    let max_len = cur.len().max(lat.len());
    for i in 0..max_len {
        let c = *cur.get(i).unwrap_or(&0);
        let l = *lat.get(i).unwrap_or(&0);
        if l > c {
            return true;
        }
        if l < c {
            return false;
        }
    }
    false
}

fn check_github_update() -> UpdateInfo {
    let api_url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest");
    let response = ureq::get(&api_url)
        .set("User-Agent", &format!("FontCheck/{APP_VERSION}"))
        .set("Accept", "application/vnd.github+json")
        .timeout(std::time::Duration::from_secs(8))
        .call();

    match response {
        Ok(resp) => {
            if let Ok(json) = resp.into_json::<serde_json::Value>() {
                let tag_name = json["tag_name"]
                    .as_str()
                    .unwrap_or(APP_VERSION)
                    .trim_start_matches(['v', 'V'])
                    .to_string();
                let html_url = json["html_url"]
                    .as_str()
                    .map(String::from)
                    .unwrap_or_else(|| format!("https://github.com/{GITHUB_REPO}/releases"));
                let notes = json["body"].as_str().unwrap_or("").to_string();

                let mut download_url: Option<String> = None;
                if let Some(assets) = json["assets"].as_array() {
                    for asset in assets {
                        let name = asset["name"].as_str().unwrap_or("").to_lowercase();
                        if name.ends_with(".exe") && !name.contains("setup") {
                            if let Some(url) = asset["browser_download_url"].as_str() {
                                download_url = Some(url.to_string());
                                break;
                            }
                        }
                    }
                }

                let available = is_newer_version(APP_VERSION, &tag_name);
                UpdateInfo {
                    available,
                    current_version: APP_VERSION.to_string(),
                    latest_version: tag_name,
                    release_url: html_url,
                    download_url,
                    notes,
                    error: None,
                }
            } else {
                UpdateInfo {
                    available: false,
                    current_version: APP_VERSION.to_string(),
                    latest_version: APP_VERSION.to_string(),
                    release_url: format!("https://github.com/{GITHUB_REPO}/releases"),
                    download_url: None,
                    notes: String::new(),
                    error: Some("Réponse GitHub invalide".to_string()),
                }
            }
        }
        Err(ureq::Error::Status(404, _)) => {
            // Fallback: check tags if no formal release is published yet
            let tags_url = format!("https://api.github.com/repos/{GITHUB_REPO}/tags");
            if let Ok(resp) = ureq::get(&tags_url)
                .set("User-Agent", &format!("FontCheck/{APP_VERSION}"))
                .timeout(std::time::Duration::from_secs(6))
                .call()
            {
                if let Ok(tags) = resp.into_json::<serde_json::Value>() {
                    if let Some(first) = tags.as_array().and_then(|a| a.first()) {
                        if let Some(tag_raw) = first["name"].as_str() {
                            let tag = tag_raw.trim_start_matches(['v', 'V']).to_string();
                            let available = is_newer_version(APP_VERSION, &tag);
                            return UpdateInfo {
                                available,
                                current_version: APP_VERSION.to_string(),
                                latest_version: tag,
                                release_url: format!(
                                    "https://github.com/{GITHUB_REPO}/releases/tag/{tag_raw}"
                                ),
                                download_url: None,
                                notes: String::new(),
                                error: None,
                            };
                        }
                    }
                }
            }
            UpdateInfo {
                available: false,
                current_version: APP_VERSION.to_string(),
                latest_version: APP_VERSION.to_string(),
                release_url: format!("https://github.com/{GITHUB_REPO}/releases"),
                download_url: None,
                notes: String::new(),
                error: None,
            }
        }
        Err(e) => UpdateInfo {
            available: false,
            current_version: APP_VERSION.to_string(),
            latest_version: APP_VERSION.to_string(),
            release_url: format!("https://github.com/{GITHUB_REPO}/releases"),
            download_url: None,
            notes: String::new(),
            error: Some(format!("Erreur réseau : {e}")),
        },
    }
}

fn download_and_install_update(download_url: &str) -> Result<(), String> {
    let resp = ureq::get(download_url)
        .set("User-Agent", &format!("FontCheck/{APP_VERSION}"))
        .timeout(std::time::Duration::from_secs(60))
        .call()
        .map_err(|e| format!("Échec du téléchargement : {e}"))?;

    let mut bytes = Vec::with_capacity(4 * 1024 * 1024);
    resp.into_reader()
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Erreur de lecture du fichier : {e}"))?;

    if bytes.len() < 1024 || &bytes[0..2] != b"MZ" {
        return Err("Le fichier téléchargé n'est pas un exécutable Windows valide.".to_string());
    }

    let current_exe =
        env::current_exe().map_err(|e| format!("Impossible de localiser l'exécutable : {e}"))?;
    let update_exe = current_exe.with_extension("new.exe");
    let backup_exe = current_exe.with_extension("old.exe");

    fs::write(&update_exe, &bytes)
        .map_err(|e| format!("Impossible d'écrire la mise à jour : {e}"))?;

    // On Windows, a running .exe can be renamed even while locked!
    let _ = fs::remove_file(&backup_exe);
    if fs::rename(&current_exe, &backup_exe).is_ok() {
        if let Err(e) = fs::rename(&update_exe, &current_exe) {
            let _ = fs::rename(&backup_exe, &current_exe);
            return Err(format!("Impossible de remplacer l'exécutable : {e}"));
        }
        // Relaunch the newly updated binary
        let _ = Command::new(&current_exe).spawn();
        std::process::exit(0);
    } else {
        // Fallback via cmd script if rename was blocked
        let batch_path = data_dir().join("update_fontcheck.cmd");
        let cur_str = current_exe.to_string_lossy();
        let new_str = update_exe.to_string_lossy();
        let script = format!(
            "@echo off\r\ntimeout /t 1 /nobreak >nul\r\nmove /y \"{new_str}\" \"{cur_str}\" >nul\r\nstart \"\" \"{cur_str}\"\r\ndel \"%~f0\"\r\n"
        );
        fs::write(&batch_path, script)
            .map_err(|e| format!("Erreur lors de la création du script de mise à jour : {e}"))?;
        let _ = Command::new("cmd")
            .args(["/C", &batch_path.to_string_lossy()])
            .spawn();
        std::process::exit(0);
    }
}

fn open_url_in_browser(url: &str) {
    if url.starts_with("https://") || url.starts_with("http://") {
        let _ = Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .spawn();
    }
}

fn load_window_icon() -> Option<Icon> {
    // Parse the first image or best icon from FontCheck.ico or decode raw RGBA if needed.
    // Our FontCheck.ico contains standard BMP/PNG frames; let's create a 32x32 RGBA icon or parse ICO.
    // Let's parse uncompressed 32-bit BGRA frame from FontCheck.ico if available.
    parse_ico_to_rgba(LOGO_ICO).and_then(|(rgba, w, h)| Icon::from_rgba(rgba, w, h).ok())
}

fn parse_ico_to_rgba(ico: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    if ico.len() < 6 || ico[0..4] != [0, 0, 1, 0] {
        return None;
    }
    let count = u16::from_le_bytes([ico[4], ico[5]]) as usize;
    let mut best_idx = 0;
    let mut best_width = 0u32;

    for i in 0..count {
        let entry_offset = 6 + i * 16;
        if entry_offset + 16 > ico.len() {
            break;
        }
        let w = if ico[entry_offset] == 0 {
            256
        } else {
            ico[entry_offset] as u32
        };
        if w >= best_width && w <= 64 {
            best_width = w;
            best_idx = i;
        }
    }

    let entry_offset = 6 + best_idx * 16;
    if entry_offset + 16 > ico.len() {
        return None;
    }
    let width = if ico[entry_offset] == 0 {
        256
    } else {
        ico[entry_offset] as u32
    };
    let height = if ico[entry_offset + 1] == 0 {
        256
    } else {
        ico[entry_offset + 1] as u32
    };
    let size = u32::from_le_bytes(ico[entry_offset + 8..entry_offset + 12].try_into().ok()?) as usize;
    let offset =
        u32::from_le_bytes(ico[entry_offset + 12..entry_offset + 16].try_into().ok()?) as usize;

    if offset + size > ico.len() || size < 40 {
        return None;
    }
    let data = &ico[offset..offset + size];
    // Check if BITMAPINFOHEADER (40 bytes) with 32 bpp
    let header_size = u32::from_le_bytes(data[0..4].try_into().ok()?) as usize;
    let bpp = u16::from_le_bytes(data[14..16].try_into().ok()?);
    if header_size == 40 && bpp == 32 {
        let pixel_bytes = (width * height * 4) as usize;
        if data.len() >= 40 + pixel_bytes {
            let mut rgba = vec![0u8; pixel_bytes];
            // DIB is bottom-up BGRA
            for y in 0..height {
                let src_row = (height - 1 - y) as usize * (width as usize * 4);
                let dst_row = y as usize * (width as usize * 4);
                for x in 0..width as usize {
                    let s = 40 + src_row + x * 4;
                    let d = dst_row + x * 4;
                    rgba[d] = data[s + 2];     // R
                    rgba[d + 1] = data[s + 1]; // G
                    rgba[d + 2] = data[s];     // B
                    rgba[d + 3] = data[s + 3]; // A
                }
            }
            return Some((rgba, width, height));
        }
    }
    None
}

fn main() {
    // Clean up any leftover .old.exe from a previous self-update
    if let Ok(exe) = env::current_exe() {
        let old = exe.with_extension("old.exe");
        if old.exists() {
            let _ = fs::remove_file(old);
        }
    }

    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
        let app_id: Vec<u16> = "FontCheck.App\0".encode_utf16().collect();
        let _ = SetCurrentProcessExplicitAppUserModelID(app_id.as_ptr());
    }

    let fonts = enumerate_windows_fonts();
    let licenses = load_licenses();
    let favorites = load_favorites();

    let initial_state = AppStateData {
        fonts,
        licenses: licenses.clone(),
        favorites: favorites.iter().cloned().collect(),
        version: APP_VERSION.to_string(),
    };

    let shared = Arc::new(Mutex::new(SharedState {
        licenses,
        favorites,
    }));

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy: EventLoopProxy<UserEvent> = event_loop.create_proxy();

    let window = WindowBuilder::new()
        .with_title("FontCheck")
        .with_inner_size(Size::Logical(LogicalSize::new(1160.0, 780.0)))
        .with_min_inner_size(Size::Logical(LogicalSize::new(760.0, 530.0)))
        .with_window_icon(load_window_icon())
        .build(&event_loop)
        .expect("Failed to create window");

    let init_json = serde_json::to_string(&initial_state).unwrap_or_else(|_| "{}".to_string());
    let init_script = format!("window.__FONTCHECK_INIT__ = {init_json};");

    let mut web_context = WebContext::new(Some(data_dir().join("webview")));

    let shared_ipc = Arc::clone(&shared);
    let proxy_ipc = proxy.clone();

    let webview = WebViewBuilder::new_with_web_context(&mut web_context)
        .with_initialization_script(&init_script)
        .with_custom_protocol("fontcheck".into(), move |_id, request| {
            let path = request.uri().path();
            let (content, mime) = match path {
                "/" | "/index.html" => (INDEX_HTML, "text/html; charset=utf-8"),
                "/style.css" => (STYLE_CSS, "text/css; charset=utf-8"),
                "/app.js" => (APP_JS, "application/javascript; charset=utf-8"),
                "/FontCheck.png" => (LOGO_PNG, "image/png"),
                "/FontCheck.ico" => (LOGO_ICO, "image/x-icon"),
                _ => (b"Not Found" as &[u8], "text/plain; charset=utf-8"),
            };
            Response::builder()
                .header(CONTENT_TYPE, mime)
                .body(std::borrow::Cow::Borrowed(content))
                .unwrap()
        })
        .with_ipc_handler(move |req| {
            let body = req.body();
            let Ok(cmd) = serde_json::from_str::<IpcCommand>(body) else {
                return;
            };
            match cmd {
                IpcCommand::RefreshFonts => {
                    let p = proxy_ipc.clone();
                    thread::spawn(move || {
                        let fonts = enumerate_windows_fonts();
                        let _ = p.send_event(UserEvent::FontsRefreshed(fonts));
                    });
                }
                IpcCommand::SetLicense { family, status } => {
                    if let Ok(mut guard) = shared_ipc.lock() {
                        if status == "inconnu" {
                            guard.licenses.remove(&family);
                        } else {
                            guard.licenses.insert(family, status);
                        }
                        save_licenses(&guard.licenses);
                    }
                }
                IpcCommand::ToggleFavorite { family } => {
                    if let Ok(mut guard) = shared_ipc.lock() {
                        if !guard.favorites.remove(&family) {
                            guard.favorites.insert(family);
                        }
                        save_favorites(&guard.favorites);
                    }
                }
                IpcCommand::CheckUpdate { manual } => {
                    let p = proxy_ipc.clone();
                    thread::spawn(move || {
                        let info = check_github_update();
                        let _ = p.send_event(UserEvent::UpdateChecked(info, manual));
                    });
                }
                IpcCommand::InstallUpdate {
                    download_url,
                    release_url,
                } => {
                    if download_url.is_empty() {
                        open_url_in_browser(&release_url);
                    } else {
                        let p = proxy_ipc.clone();
                        thread::spawn(move || {
                            let res = download_and_install_update(&download_url);
                            let _ = p.send_event(UserEvent::UpdateInstallResult(res));
                        });
                    }
                }
                IpcCommand::OpenExternal { url } => {
                    open_url_in_browser(&url);
                }
            }
        })
        .with_url("fontcheck://localhost/index.html")
        .build(&window)
        .expect("Failed to build WebView");

    // Automatically check for updates on startup in a background thread
    let startup_proxy = proxy.clone();
    thread::spawn(move || {
        thread::sleep(std::time::Duration::from_millis(900));
        let info = check_github_update();
        let _ = startup_proxy.send_event(UserEvent::UpdateChecked(info, false));
    });

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::NewEvents(StartCause::Init) => {}
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }
            Event::UserEvent(user_event) => match user_event {
                UserEvent::FontsRefreshed(fonts) => {
                    if let Ok(json) = serde_json::to_string(&fonts) {
                        let js =
                            format!("window.__onFontsRefreshed && window.__onFontsRefreshed({json});");
                        let _ = webview.evaluate_script(&js);
                    }
                }
                UserEvent::UpdateChecked(info, manual) => {
                    if let Ok(json) = serde_json::to_string(&info) {
                        let js = format!(
                            "window.__onUpdateChecked && window.__onUpdateChecked({json}, {manual});"
                        );
                        let _ = webview.evaluate_script(&js);
                    }
                }
                UserEvent::UpdateInstallResult(Err(err_msg)) => {
                    if let Ok(json) = serde_json::to_string(&err_msg) {
                        let js =
                            format!("window.__onUpdateError && window.__onUpdateError({json});");
                        let _ = webview.evaluate_script(&js);
                    }
                }
                UserEvent::UpdateInstallResult(Ok(())) => {}
            },
            _ => {}
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        assert!(is_newer_version("1.0.0", "1.0.1"));
        assert!(is_newer_version("1.0.0", "v1.1.0"));
        assert!(is_newer_version("1.0.0", "2.0.0"));
        assert!(!is_newer_version("1.0.0", "1.0.0"));
        assert!(!is_newer_version("1.2.0", "1.1.9"));
        assert!(is_newer_version("1.0", "1.0.1"));
    }

    #[test]
    fn test_enumerate_windows_fonts() {
        let fonts = enumerate_windows_fonts();
        assert!(!fonts.is_empty(), "Should detect installed Windows fonts");
        assert!(fonts.iter().any(|f| f.eq_ignore_ascii_case("Arial") || f.eq_ignore_ascii_case("Segoe UI")));
    }

    #[test]
    fn test_ico_parser() {
        let parsed = parse_ico_to_rgba(LOGO_ICO);
        assert!(parsed.is_some(), "Should parse embedded FontCheck.ico into RGBA");
    }
}

