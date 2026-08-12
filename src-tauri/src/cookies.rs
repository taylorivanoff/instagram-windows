//! Instagram / Meta cookie persistence so sessions survive quit.
//! Session cookies (no expiry) get a ~180-day TTL.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use cookie::SameSite;
use tauri::webview::Cookie;
use tauri::{AppHandle, Manager, WebviewWindow};
use time::{Duration, OffsetDateTime};

const SESSION_COOKIE_TTL_SECS: i64 = 180 * 24 * 60 * 60;

static APPLYING: AtomicBool = AtomicBool::new(false);
static LAST_FINGERPRINT: Mutex<String> = Mutex::new(String::new());

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub secure: bool,
    #[serde(rename = "httpOnly")]
    pub http_only: bool,
    #[serde(rename = "expirationDate")]
    pub expiration_date: i64,
    #[serde(rename = "sameSite")]
    pub same_site: String,
}

pub fn cookie_file_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app_data_dir: {e}"))?;
    Ok(dir.join("cookies.json"))
}

pub fn is_instagram_family_domain(domain: &str) -> bool {
    let d = domain.to_ascii_lowercase();
    d.contains("instagram.com")
        || d.contains("facebook.com")
        || d.contains("fb.com")
        || d.contains("meta.com")
        || d.contains("cdninstagram.com")
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn ensure_parent(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }
    Ok(())
}

fn fingerprint(cookies: &[StoredCookie]) -> String {
    let mut parts: Vec<String> = cookies
        .iter()
        .map(|c| format!("{}\n{}\n{}", c.name, c.domain, c.value))
        .collect();
    parts.sort();
    parts.join("\0")
}

fn same_site_str(cookie: &Cookie<'_>) -> String {
    match cookie.same_site() {
        Some(SameSite::None) => "no_restriction".into(),
        Some(SameSite::Strict) => "strict".into(),
        Some(SameSite::Lax) | None => "lax".into(),
    }
}

fn serialize_cookie(cookie: &Cookie<'_>) -> Option<StoredCookie> {
    let domain = cookie.domain()?.to_string();
    if !is_instagram_family_domain(&domain) {
        return None;
    }

    let expiration_date = cookie
        .expires_datetime()
        .map(|dt| dt.unix_timestamp())
        .unwrap_or_else(|| now_unix() + SESSION_COOKIE_TTL_SECS);

    Some(StoredCookie {
        name: cookie.name().to_string(),
        value: cookie.value().to_string(),
        domain,
        path: cookie.path().unwrap_or("/").to_string(),
        secure: cookie.secure().unwrap_or(true),
        http_only: cookie.http_only().unwrap_or(false),
        expiration_date,
        same_site: same_site_str(cookie),
    })
}

fn to_tauri_cookie(stored: &StoredCookie) -> Cookie<'static> {
    let same_site = match stored.same_site.as_str() {
        "no_restriction" | "none" => SameSite::None,
        "strict" => SameSite::Strict,
        _ => SameSite::Lax,
    };

    let expires = OffsetDateTime::from_unix_timestamp(stored.expiration_date)
        .unwrap_or_else(|_| OffsetDateTime::now_utc() + Duration::days(180));

    Cookie::build((stored.name.clone(), stored.value.clone()))
        .domain(stored.domain.clone())
        .path(stored.path.clone())
        .secure(stored.secure)
        .http_only(stored.http_only)
        .same_site(same_site)
        .expires(expires)
        .build()
}

fn read_cookie_file(path: &Path) -> Option<Vec<StoredCookie>> {
    let raw = fs::read_to_string(path).ok()?;
    let cookies: Vec<StoredCookie> = serde_json::from_str(&raw).ok()?;
    if cookies.is_empty() {
        None
    } else {
        Some(cookies)
    }
}

pub fn load_persisted_cookies(app: &AppHandle, window: &WebviewWindow) -> Result<bool, String> {
    let path = cookie_file_path(app)?;
    let Some(cookies) = read_cookie_file(&path) else {
        return Ok(false);
    };

    APPLYING.store(true, Ordering::SeqCst);
    let result = (|| {
        let mut applied = 0usize;
        for stored in &cookies {
            if stored.name.is_empty() || stored.domain.is_empty() {
                continue;
            }
            let cookie = to_tauri_cookie(stored);
            match window.set_cookie(cookie) {
                Ok(()) => applied += 1,
                Err(e) => eprintln!("[cookies] set failed {}: {e}", stored.name),
            }
        }
        if let Ok(mut fp) = LAST_FINGERPRINT.lock() {
            *fp = fingerprint(&cookies);
        }
        Ok(applied > 0)
    })();
    APPLYING.store(false, Ordering::SeqCst);
    result
}

pub fn save_persisted_cookies(app: &AppHandle, window: &WebviewWindow) -> Result<(), String> {
    if APPLYING.load(Ordering::SeqCst) {
        return Ok(());
    }

    let all = window
        .cookies()
        .map_err(|e| format!("cookies(): {e}"))?;

    let cookies: Vec<StoredCookie> = all.iter().filter_map(serialize_cookie).collect();
    let next = fingerprint(&cookies);
    {
        let mut fp = LAST_FINGERPRINT.lock().map_err(|e| e.to_string())?;
        if *fp == next {
            return Ok(());
        }
        *fp = next;
    }

    let path = cookie_file_path(app)?;
    ensure_parent(&path)?;
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    let json = serde_json::to_string_pretty(&cookies).map_err(|e| e.to_string())?;
    fs::write(&tmp, json).map_err(|e| format!("write tmp: {e}"))?;
    fs::rename(&tmp, &path).map_err(|e| format!("rename: {e}"))?;
    Ok(())
}

pub fn start_cookie_poller(app: AppHandle, window: WebviewWindow) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(5));
            if APPLYING.load(Ordering::SeqCst) {
                continue;
            }
            if let Err(e) = save_persisted_cookies(&app, &window) {
                eprintln!("[cookies] poll: {e}");
            }
        }
    });
}
