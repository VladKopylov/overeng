mod layout;
mod notion;
mod settings;

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow, WindowEvent};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, ShortcutState};

use layout::{Corner, Layout, MonitorArea, Rect};
use notion::client::{NotionClient, NOTION_BASE_URL};
use notion::{Card, DataSourceOption, NotionError};
use settings::{Settings, SettingsView};

const CYCLE_SHORTCUT: &str = "ctrl+shift+F9";

const SETTINGS_SHORTCUT: &str = "ctrl+shift+F8";

const EXPAND_SHORTCUT: &str = "ctrl+shift+F10";

const VISIBLE_SHORTCUT: &str = "ctrl+shift+F11";

const EDIT_SHORTCUT: &str = "ctrl+shift+F12";

const TRANSLATION_SHORTCUT: &str = "ctrl+shift+F7";

const CYCLE_EVENT: &str = "cycle-phrase";

const EXPANDED_EVENT: &str = "overlay-expanded";

const VISIBLE_EVENT: &str = "overlay-visible";

const EDIT_EVENT: &str = "toggle-edit";

const TRANSLATION_EVENT: &str = "toggle-translation";

const HIDE_TRANSLATION_EVENT: &str = "overlay-hide-translation";

const LAYOUT_RESET_EVENT: &str = "layout-reset";

const CARDS_UPDATED_EVENT: &str = "cards-updated";

const MAX_LOG_BYTES: u64 = 1024 * 1024;

const LOG_FILE: &str = "overlay-debug.log";

const LOG_FILE_OLD: &str = "overlay-debug.log.old";

static LAST_REASSERT_STATE: AtomicU8 = AtomicU8::new(u8::MAX);

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
    let path = dir.join(LOG_FILE);
    rotate_if_large(&path, MAX_LOG_BYTES);
    match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(mut file) => {
            if let Err(e) = file.write_all(line.as_bytes()) {
                eprintln!("не записав у {}: {e}", path.display());
            }
        }
        Err(e) => eprintln!("не відкрив {}: {e}", path.display()),
    }
}

fn rotate_if_large(path: &Path, max_bytes: u64) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if meta.len() <= max_bytes {
        return false;
    }
    let old = path.with_file_name(LOG_FILE_OLD);
    std::fs::rename(path, old).is_ok()
}

fn emit_overlay<S: serde::Serialize + Clone>(app: &AppHandle, event: &str, payload: S) {
    if let Err(e) = app.emit_to("overlay", event, payload) {
        write_log(app, &format!("emit {event} ПОМИЛКА: {e}"));
    }
}

fn toggle_layout(app: &AppHandle, event: &str, change: impl FnOnce(&mut Layout) -> bool) {
    match settings::update_layout(app, change) {
        Ok((_, value)) => {
            write_log(app, &format!("{event} -> {value}"));
            emit_overlay(app, event, value);
        }
        Err(e) => write_log(app, &format!("{event} ПОМИЛКА збереження: {e}")),
    }
}

fn overlay_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    app.get_webview_window("overlay")
        .ok_or_else(|| "вікно overlay не знайдено".to_string())
}

fn outer_rect(window: &WebviewWindow) -> Result<Rect, String> {
    let pos = window.outer_position().map_err(|e| e.to_string())?;
    let size = window.outer_size().map_err(|e| e.to_string())?;
    Ok(Rect {
        x: pos.x,
        y: pos.y,
        width: size.width as i32,
        height: size.height as i32,
    })
}

#[tauri::command]
fn get_layout(app: AppHandle) -> Result<Layout, NotionError> {
    settings::load_layout(&app)
}

#[tauri::command]
fn overlay_rect(app: AppHandle, from_current: bool) -> Result<Rect, String> {
    let result = overlay_rect_impl(&app, from_current);
    match &result {
        Ok(r) => write_log(
            &app,
            &format!(
                "overlay_rect (from_current={from_current}): {},{} {}x{}",
                r.x, r.y, r.width, r.height
            ),
        ),
        Err(e) => write_log(&app, &format!("overlay_rect ПОМИЛКА: {e}")),
    }
    result
}

fn overlay_rect_impl(app: &AppHandle, from_current: bool) -> Result<Rect, String> {
    let window = overlay_window(app)?;
    let layout = settings::load_layout(app).map_err(|e| e.to_string())?;
    let monitors = window.available_monitors().map_err(|e| e.to_string())?;
    let primary = window.primary_monitor().ok().flatten();
    let primary_index = primary
        .and_then(|p| {
            monitors
                .iter()
                .position(|m| m.position() == p.position() && m.size() == p.size())
        })
        .unwrap_or(0);
    let areas: Vec<MonitorArea> = monitors
        .iter()
        .map(|m| {
            let wa = m.work_area();
            MonitorArea {
                work_area: Rect {
                    x: wa.position.x,
                    y: wa.position.y,
                    width: wa.size.width as i32,
                    height: wa.size.height as i32,
                },
                scale: m.scale_factor(),
            }
        })
        .collect();
    let current = if from_current {
        Some(outer_rect(&window)?)
    } else {
        None
    };
    write_log(
        app,
        &format!(
            "overlay_rect: моніторів {}, основний #{primary_index}, expanded={}, збережена позиція: {}",
            areas.len(),
            layout.expanded,
            layout.position.is_some()
        ),
    );
    layout::target_rect(&areas, primary_index, &layout, current)
        .ok_or_else(|| "моніторів не знайдено".to_string())
}

#[tauri::command]
fn save_overlay_rect(app: AppHandle, expanded: bool) -> Result<(), String> {
    let result = overlay_window(&app)
        .and_then(|w| {
            let scale = w.scale_factor().map_err(|e| e.to_string())?;
            outer_rect(&w).map(|rect| (rect, scale))
        })
        .and_then(|(rect, scale)| {
            settings::update_layout(&app, |l| l.remember(rect, expanded, scale))
                .map(|_| rect)
                .map_err(|e| e.to_string())
        });
    match &result {
        Ok(r) => write_log(
            &app,
            &format!(
                "позицію збережено (expanded={expanded}): {},{} {}x{}",
                r.x, r.y, r.width, r.height
            ),
        ),
        Err(e) => write_log(&app, &format!("save_overlay_rect ПОМИЛКА: {e}")),
    }
    result.map(|_| ())
}

#[tauri::command]
fn set_corner(app: AppHandle, corner: Corner) -> Result<(), NotionError> {
    settings::update_layout(&app, |l| {
        l.corner = corner;
        l.position = None;
    })?;
    write_log(&app, &format!("кут оверлея: {corner:?}"));
    emit_overlay(&app, LAYOUT_RESET_EVENT, ());
    Ok(())
}

#[tauri::command]
fn reset_overlay_position(app: AppHandle) -> Result<(), NotionError> {
    settings::update_layout(&app, Layout::reset_position)?;
    write_log(&app, "позицію оверлея скинуто");
    emit_overlay(&app, LAYOUT_RESET_EVENT, ());
    Ok(())
}

#[tauri::command]
fn set_hide_translation(app: AppHandle, hide: bool) -> Result<(), NotionError> {
    settings::update_layout(&app, |l| l.hide_translation = hide)?;
    write_log(&app, &format!("приховувати переклад: {hide}"));
    emit_overlay(&app, HIDE_TRANSLATION_EVENT, hide);
    Ok(())
}

#[tauri::command]
fn debug_log(app: AppHandle, msg: String) {
    write_log(&app, &format!("[js] {msg}"));
}

#[tauri::command]
fn reassert_topmost(app: AppHandle, window: tauri::WebviewWindow) -> Result<(), String> {
    match reassert_topmost_impl(&window) {
        Ok(had_noactivate) => {
            let state = u8::from(had_noactivate);
            if LAST_REASSERT_STATE.swap(state, Ordering::SeqCst) != state {
                write_log(
                    &app,
                    &format!("reassert_topmost ok (WS_EX_NOACTIVATE вже був: {had_noactivate})"),
                );
            }
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

#[derive(Default)]
struct AutoAdvance {
    generation: AtomicU64,
}

fn restart_auto_advance(app: &AppHandle) {
    let generation = app
        .state::<AutoAdvance>()
        .generation
        .fetch_add(1, Ordering::SeqCst)
        + 1;
    let interval = match settings::load(app) {
        Ok(current) => current.auto_advance(),
        Err(e) => {
            write_log(
                app,
                &format!("автоперемикання: налаштування не прочитано: {e}"),
            );
            return;
        }
    };
    let Some(interval) = interval else {
        write_log(app, "автоперемикання вимкнено");
        return;
    };
    write_log(
        app,
        &format!("автоперемикання кожні {} с", interval.as_secs()),
    );
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(interval).await;
            let current = handle
                .state::<AutoAdvance>()
                .generation
                .load(Ordering::SeqCst);
            if current != generation {
                return;
            }
            let hidden = settings::load_layout(&handle)
                .map(|l| !l.visible)
                .unwrap_or(false);
            if hidden {
                continue;
            }
            write_log(&handle, "автоперемикання -> наступна картка");
            emit_overlay(&handle, CYCLE_EVENT, ());
        }
    });
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
    emit_overlay(&app, CARDS_UPDATED_EVENT, ());
    restart_auto_advance(&app);
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
    emit_overlay(app, CARDS_UPDATED_EVENT, ());
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
                    let mods = Modifiers::CONTROL | Modifiers::SHIFT;
                    if shortcut.matches(mods, Code::F7) {
                        emit_overlay(app, TRANSLATION_EVENT, ());
                    } else if shortcut.matches(mods, Code::F8) {
                        open_settings(app);
                    } else if shortcut.matches(mods, Code::F9) {
                        emit_overlay(app, CYCLE_EVENT, ());
                        restart_auto_advance(app);
                    } else if shortcut.matches(mods, Code::F10) {
                        toggle_layout(app, EXPANDED_EVENT, |l| {
                            l.expanded = !l.expanded;
                            l.expanded
                        });
                    } else if shortcut.matches(mods, Code::F11) {
                        toggle_layout(app, VISIBLE_EVENT, |l| {
                            l.visible = !l.visible;
                            l.visible
                        });
                    } else if shortcut.matches(mods, Code::F12) {
                        emit_overlay(app, EDIT_EVENT, ());
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_store::Builder::new().build())
        .manage(SyncState::default())
        .manage(AutoAdvance::default())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let handle = app.handle();
            write_log(handle, "=== старт застосунку ===");

            for shortcut in [
                TRANSLATION_SHORTCUT,
                CYCLE_SHORTCUT,
                SETTINGS_SHORTCUT,
                EXPAND_SHORTCUT,
                VISIBLE_SHORTCUT,
                EDIT_SHORTCUT,
            ] {
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

            restart_auto_advance(handle);

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
            quit_app,
            get_layout,
            overlay_rect,
            save_overlay_rect,
            set_corner,
            reset_overlay_position,
            set_hide_translation
        ])
        .run(tauri::generate_context!())
        .expect("помилка під час запуску Tauri-застосунку");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_rotates_only_when_larger_than_limit() {
        let dir = std::env::temp_dir().join(format!("word-overlay-log-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(LOG_FILE);
        let old = dir.join(LOG_FILE_OLD);

        assert!(!rotate_if_large(&path, 100));

        std::fs::write(&path, vec![b'x'; 100]).unwrap();
        assert!(!rotate_if_large(&path, 100));
        assert!(path.exists() && !old.exists());

        std::fs::write(&path, vec![b'x'; 101]).unwrap();
        assert!(rotate_if_large(&path, 100));
        assert!(!path.exists() && old.exists());
        assert_eq!(std::fs::metadata(&old).unwrap().len(), 101);

        std::fs::write(&path, vec![b'y'; 200]).unwrap();
        assert!(rotate_if_large(&path, 100));
        assert_eq!(std::fs::metadata(&old).unwrap().len(), 200);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
