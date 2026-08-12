mod cookies;

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use tauri::webview::{DownloadEvent, NewWindowResponse};
use tauri::{
    AppHandle, Listener, Manager, RunEvent, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_opener::OpenerExt;
use tauri_tray_base::{
    apply_window_settings, install_state, on_window_event, set_on_before_quit, setup_tray,
    with_common_plugins, TrayBaseOptions, TrayClickAction, TrayExtraItem, TraySetupOptions,
    MAIN_WINDOW_LABEL,
};
use url::Url;

const INSTAGRAM_URL: &str = "https://www.instagram.com";
const APP_NAME: &str = "Instagram";

/// Desktop Chrome UA — Instagram's web UI expects a modern Chromium client.
const CHROME_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

fn instagram_url() -> Url {
    Url::parse(INSTAGRAM_URL).expect("INSTAGRAM_URL")
}

fn unique_download_path(path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }
    let parent = path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("download");
    let ext = path.extension().and_then(|s| s.to_str());
    for i in 1..10_000 {
        let name = match ext {
            Some(e) => format!("{stem} ({i}).{e}"),
            None => format!("{stem} ({i})"),
        };
        let candidate = parent.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }
    path
}

fn downloads_destination(app: &AppHandle, suggested: &Path, url: &Url) -> PathBuf {
    let dir = app.path().download_dir().unwrap_or_else(|_| {
        std::env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Downloads")
    });
    let _ = std::fs::create_dir_all(&dir);

    let name = suggested
        .file_name()
        .map(|n| n.to_os_string())
        .filter(|n| !n.is_empty())
        .or_else(|| {
            url.path_segments()
                .and_then(|mut parts| parts.next_back())
                .filter(|s| !s.is_empty() && *s != "/")
                .map(OsString::from)
        })
        .unwrap_or_else(|| OsString::from("download"));

    unique_download_path(dir.join(name))
}

fn notify_download(app: &AppHandle, path: Option<&Path>, success: bool) {
    let file_name = path
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("file");
    let (title, body) = if success {
        (
            "Instagram download complete".to_string(),
            format!("{file_name} saved to Downloads"),
        )
    } else {
        (
            "Instagram download failed".to_string(),
            format!("Could not download {file_name}"),
        )
    };
    let _ = app
        .notification()
        .builder()
        .title(title)
        .body(body)
        .show();
}

fn persist_cookies(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let app2 = app.clone();
        let window2 = window.clone();
        let _ = std::thread::Builder::new()
            .name("cookie-persist".into())
            .spawn(move || {
                if let Err(e) = cookies::save_persisted_cookies(&app2, &window2) {
                    eprintln!("[cookies] persist: {e}");
                }
            });
    }
}

fn refresh_page(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let _ = window.navigate(instagram_url());
    }
}

fn build_main_window(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let app_data = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    let webview_data = app_data.join("webview");
    let _ = std::fs::create_dir_all(&webview_data);

    let app_for_open = app.clone();
    let app_for_download = app.clone();

    WebviewWindowBuilder::new(app, MAIN_WINDOW_LABEL, WebviewUrl::External(instagram_url()))
        .title(APP_NAME)
        .inner_size(1280.0, 900.0)
        .resizable(true)
        .visible(false)
        .user_agent(CHROME_UA)
        .data_directory(webview_data)
        .on_download(move |_webview, event| match event {
            DownloadEvent::Requested { url, destination } => {
                *destination = downloads_destination(&app_for_download, destination, &url);
                true
            }
            DownloadEvent::Finished { path, success, .. } => {
                notify_download(&app_for_download, path.as_deref(), success);
                true
            }
            _ => true,
        })
        .on_new_window(move |url, _features| {
            let href = url.as_str().to_string();
            if href != "about:blank#blocked" && !href.starts_with("about:blank") {
                let _ = app_for_open.opener().open_url(href, None::<&str>);
            }
            NewWindowResponse::Deny
        })
        .build()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = with_common_plugins(tauri::Builder::default())
        .plugin(tauri_plugin_notification::init());

    builder
        .setup(|app| {
            let handle = app.handle().clone();

            install_state(
                &handle,
                TrayBaseOptions {
                    app_name: APP_NAME.into(),
                    show_hide: false,
                    show_always_on_top: false,
                    tray_on_click: TrayClickAction::Toggle,
                    extra_tray_items: vec![TrayExtraItem {
                        id: "refresh".into(),
                        label: "Refresh".into(),
                    }],
                    ..Default::default()
                },
            )?;

            let window = build_main_window(&handle)?;

            setup_tray(
                &handle,
                TraySetupOptions {
                    tooltip: Some(APP_NAME.into()),
                },
            )?;

            apply_window_settings(&handle);

            let quit_app = handle.clone();
            set_on_before_quit(&handle, move || {
                persist_cookies(&quit_app);
            });

            let restore_app = handle.clone();
            let restore_win = window.clone();
            std::thread::spawn(move || {
                match cookies::load_persisted_cookies(&restore_app, &restore_win) {
                    Ok(true) => eprintln!("[cookies] restored persisted Instagram cookies"),
                    Ok(false) => eprintln!("[cookies] no persisted cookies"),
                    Err(e) => eprintln!("[cookies] restore: {e}"),
                }
                cookies::start_cookie_poller(restore_app, restore_win);
            });

            let refresh_handle = handle.clone();
            handle.listen("tray:action", move |event| {
                if event.payload().trim_matches('"') == "refresh" {
                    refresh_page(&refresh_handle);
                }
            });

            Ok(())
        })
        .on_window_event(|window, event| on_window_event(window, &event))
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| {
            if let RunEvent::ExitRequested { .. } = event {
                // Cookies persist via before-quit; do not block the UI thread here.
            }
        });
}
