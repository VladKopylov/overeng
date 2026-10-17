mod notion;
mod settings;

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, ShortcutState};

use notion::client::{NotionClient, NOTION_BASE_URL};
use notion::{Card, DataSourceOption, NotionError};
use settings::{Settings, SettingsView};

const CYCLE_SHORTCUT: &str = "ctrl+shift+F9";

const SETTINGS_SHORTCUT: &str = "ctrl+shift+F8";

const CYCLE_EVENT: &str = "cycle-phrase";

const CARDS_UPDATED_EVENT: &str = "cards-updated";

const MAX_LOG_BYTES: u64 = 1024 * 1024;

fn timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let day_secs = secs % 86_400;
    format!(
        "{}.{:03} ({:02}:{:02}:{:02} UTC)",
        secs,
        now.subsec_millis(),
        day_secs / 3600,
        (day_secs % 3600) / 60,
        day_secs % 60
    )
}

fn write_log(app: &AppHandle, msg: &str) {
    let line = format!("[{}] {}\n", timestamp(), msg);
    print!("{line}");

    let dir = match app.path().app_log_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("не знайшов папку логів: {e}");
            return;
        }
    };
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("не створив папку логів {}: {e}", dir.display());
        return;
    }
    let path = dir.join("overlay-debug.log");
    match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(mut file) => {
            if let Err(e) = file.write_all(line.as_bytes()) {
                eprintln!("не записав у {}: {e}", path.display());
            }
        }
        Err(e) => eprintln!("не відкрив {}: {e}", path.display()),
    }
}

fn rotate_log(app: &AppHandle) {
    let Ok(dir) = app.path().app_log_dir() else {
        return;
    };
    let path = dir.join("overlay-debug.log");
    if let Ok(meta) = std::fs::metadata(&path) {
        if meta.len() > MAX_LOG_BYTES {
            let _ = std::fs::rename(&path, dir.join("overlay-debug.log.old"));
        }
    }
}

#[tauri::command]
fn debug_log(app: AppHandle, msg: String) {
    write_log(&app, &format!("[js] {msg}"));
}

#[tauri::command]
fn reassert_topmost(app: AppHandle, window: tauri::WebviewWindow) -> Result<(), String> {
    match reassert_topmost_impl(&window) {
        Ok(had_noactivate) => {
            write_log(
                &app,
                &format!("reassert_topmost ok (WS_EX_NOACTIVATE вже був: {had_noactivate})"),
            );
            Ok(())
        }
        Err(e) => {
            write_log(&app, &format!("reassert_topmost ПОМИЛКА: {e}"));
            Err(e)
        }
    }
}

#[cfg(windows)]
fn reassert_topmost_impl(window: &tauri::WebviewWindow) -> Result<bool, String> {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, HWND_TOPMOST,
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, WS_EX_NOACTIVATE,
    };

    let hwnd = window.hwnd().map_err(|e| e.to_string())?;

    unsafe {
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let noactivate = WS_EX_NOACTIVATE.0 as isize;
        let had_noactivate = ex_style & noactivate != 0;
        if !had_noactivate {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex_style | noactivate);
        }
        SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
        )
        .map_err(|e| e.to_string())?;
        Ok(had_noactivate)
    }
}

#[cfg(not(windows))]
fn reassert_topmost_impl(_window: &tauri::WebviewWindow) -> Result<bool, String> {
    Ok(false)
}

#[derive(Default)]
struct SyncState {
    running: AtomicBool,
}

#[derive(serde::Serialize)]
struct SyncResult {
    total: usize,
    visible: usize,
}

fn open_settings(app: &AppHandle) {
    let Some(win) = app.get_webview_window("settings") else {
        write_log(app, "УВАГА: вікно settings не знайдено");
        return;
    };
    if let Err(e) = win.show() {
        write_log(app, &format!("settings show ПОМИЛКА: {e}"));
        return;
    }
    let _ = win.unminimize();
    let _ = win.set_focus();
    write_log(app, "вікно settings відкрито");
}

#[tauri::command]
fn get_settings(app: AppHandle) -> Result<SettingsView, NotionError> {
    Ok(SettingsView {
        settings: settings::load(&app)?,
        has_token: settings::has_token(&app),
    })
}

#[tauri::command]
fn set_settings(app: AppHandle, new_settings: Settings) -> Result<(), NotionError> {
    settings::save(&app, &new_settings)?;
    write_log(&app, "налаштування збережено");
    let _ = app.emit_to("overlay", CARDS_UPDATED_EVENT, ());
    Ok(())
}

#[tauri::command]
fn set_token(app: AppHandle, token: String) -> Result<(), NotionError> {
    settings::set_token(&app, &token)?;
    write_log(
        &app,
        if token.trim().is_empty() {
            "токен видалено"
        } else {
            "токен збережено"
        },
    );
    Ok(())
}

fn notion_client(app: &AppHandle) -> Result<NotionClient, NotionError> {
    let token = settings::token(app).ok_or(NotionError::NoToken)?;
    NotionClient::new(token, NOTION_BASE_URL)
}

#[tauri::command]
async fn resolve_data_source(
    app: AppHandle,
    input: String,
) -> Result<Vec<DataSourceOption>, NotionError> {
    let client = notion_client(&app)?;
    let client = &client;
    write_log(&app, "resolve_data_source: старт");
    let result = notion::sync::resolve_data_source(
        &input,
        |id| async move { client.get_database(&id).await },
        |id| async move { client.get_data_source(&id).await },
    )
    .await;
    match &result {
        Ok(options) => write_log(
            &app,
            &format!("resolve_data_source: знайдено {}", options.len()),
        ),
        Err(e) => write_log(&app, &format!("resolve_data_source ПОМИЛКА: {e}")),
    }
    result
}

async fn run_sync(app: &AppHandle) -> Result<SyncResult, NotionError> {
    let current = settings::load(app)?;
    let data_source_id = current
        .data_source_id
        .clone()
        .ok_or(NotionError::NoDataSource)?;
    let client = notion_client(app)?;

    let client_ref = &client;
    let ds_ref = data_source_id.as_str();
    let cards: Vec<Card> = notion::sync::fetch_all(
        move |cursor| client_ref.query(ds_ref, cursor),
        tokio::time::sleep,
        &current.mapping,
    )
    .await
    .map_err(|e| match e {
        NotionError::NotFound => NotionError::NotShared,
        other => other,
    })?;

    settings::write_cache(&settings::cache_path(app)?, &cards)?;
    let total = cards.len();
    let visible = settings::visible_cards(cards, &current.hidden_statuses).len();
    if let Err(e) = app.emit_to("overlay", CARDS_UPDATED_EVENT, ()) {
        write_log(app, &format!("emit {CARDS_UPDATED_EVENT} ПОМИЛКА: {e}"));
    }
    Ok(SyncResult { total, visible })
}

struct ResetOnDrop<'a>(&'a AtomicBool);

impl Drop for ResetOnDrop<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

async fn sync_with_log(
    app: &AppHandle,
    state: &SyncState,
    reason: &str,
) -> Result<SyncResult, NotionError> {
    if state.running.swap(true, Ordering::SeqCst) {
        return Err(NotionError::Busy);
    }
    let _reset = ResetOnDrop(&state.running);
    write_log(app, &format!("sync ({reason}): старт"));
    let result = run_sync(app).await;
    match &result {
        Ok(r) => write_log(
            app,
            &format!(
                "sync ({reason}): готово, карток {} (видно {})",
                r.total, r.visible
            ),
        ),
        Err(e) => write_log(app, &format!("sync ({reason}) ПОМИЛКА: {e}")),
    }
    result
}

#[tauri::command]
async fn sync_notion(
    app: AppHandle,
    state: State<'_, SyncState>,
) -> Result<SyncResult, NotionError> {
    sync_with_log(&app, &state, "вручну").await
}

#[tauri::command]
fn get_cards(app: AppHandle) -> Result<Vec<Card>, NotionError> {
    let hidden = settings::load(&app)?.hidden_statuses;
    let cards = settings::read_cache(&settings::cache_path(&app)?)?;
    Ok(settings::visible_cards(cards, &hidden))
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    write_log(&app, "вихід за кнопкою в налаштуваннях");
    app.exit(0);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    write_log(app, &format!("hotkey {shortcut:?} натиснуто"));
                    if shortcut.matches(Modifiers::CONTROL | Modifiers::SHIFT, Code::F8) {
                        open_settings(app);
                    } else if let Err(e) = app.emit_to("overlay", CYCLE_EVENT, ()) {
                        write_log(app, &format!("emit {CYCLE_EVENT} ПОМИЛКА: {e}"));
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_store::Builder::new().build())
        .manage(SyncState::default())
        .setup(|app| {
            let handle = app.handle();
            rotate_log(handle);
            write_log(handle, "=== старт застосунку ===");

            for shortcut in [CYCLE_SHORTCUT, SETTINGS_SHORTCUT] {
                match handle.global_shortcut().register(shortcut) {
                    Ok(()) => write_log(handle, &format!("hotkey {shortcut} зареєстровано")),
                    Err(e) => {
                        write_log(handle, &format!("hotkey {shortcut} НЕ зареєстровано: {e}"))
                    }
                }
            }

            if handle.get_webview_window("overlay").is_none() {
                write_log(handle, "УВАГА: вікно overlay не знайдено");
            }

            if let Some(win) = handle.get_webview_window("settings") {
                let to_hide = win.clone();
                win.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = to_hide.hide();
                    }
                });
            }

            if !settings::has_token(handle) {
                write_log(handle, "токена нема — відкриваю налаштування");
                open_settings(handle);
            } else if settings::load(handle)
                .map(|s| s.data_source_id.is_some())
                .unwrap_or(false)
            {
                let app_handle = handle.clone();
                tauri::async_runtime::spawn(async move {
                    let state = app_handle.state::<SyncState>();
                    let _ = sync_with_log(&app_handle, &state, "автосинк при старті").await;
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            debug_log,
            reassert_topmost,
            get_settings,
            set_settings,
            set_token,
            resolve_data_source,
            sync_notion,
            get_cards,
            quit_app
        ])
        .run(tauri::generate_context!())
        .expect("помилка під час запуску Tauri-застосунку");
}
