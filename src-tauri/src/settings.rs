use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Manager};
use tauri_plugin_store::StoreExt;

use crate::notion::{Card, Mapping, NotionError};

pub const STORE_FILE: &str = "settings.json";
const KEY_SETTINGS: &str = "settings";
const KEY_TOKEN: &str = "notion_token";
pub const CACHE_FILE: &str = "cards.json";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub database_input: String,
    pub data_source_id: Option<String>,
    pub data_source_name: Option<String>,
    pub mapping: Mapping,
    pub hidden_statuses: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            database_input: String::new(),
            data_source_id: None,
            data_source_name: None,
            mapping: Mapping::default(),
            hidden_statuses: vec!["Mastered".to_string()],
        }
    }
}

#[derive(Debug, Serialize)]
pub struct SettingsView {
    #[serde(flatten)]
    pub settings: Settings,
    pub has_token: bool,
}

fn storage_err(e: impl std::fmt::Display) -> NotionError {
    NotionError::Storage(e.to_string())
}

pub fn load(app: &AppHandle) -> Result<Settings, NotionError> {
    let store = app.store(STORE_FILE).map_err(storage_err)?;
    Ok(match store.get(KEY_SETTINGS) {
        Some(value) => serde_json::from_value(value).unwrap_or_default(),
        None => Settings::default(),
    })
}

pub fn save(app: &AppHandle, settings: &Settings) -> Result<(), NotionError> {
    let store = app.store(STORE_FILE).map_err(storage_err)?;
    let value = serde_json::to_value(settings).map_err(storage_err)?;
    store.set(KEY_SETTINGS, value);
    store.save().map_err(storage_err)
}

pub fn token(app: &AppHandle) -> Option<String> {
    let store = app.store(STORE_FILE).ok()?;
    let value = store.get(KEY_TOKEN)?;
    let token = value.as_str()?.trim().to_string();
    if token.is_empty() {
        None
    } else {
        Some(token)
    }
}

pub fn has_token(app: &AppHandle) -> bool {
    token(app).is_some()
}

pub fn set_token(app: &AppHandle, token: &str) -> Result<(), NotionError> {
    let store = app.store(STORE_FILE).map_err(storage_err)?;
    let token = token.trim();
    if token.is_empty() {
        store.delete(KEY_TOKEN);
    } else {
        store.set(KEY_TOKEN, Value::String(token.to_string()));
    }
    store.save().map_err(storage_err)
}

#[derive(Debug, Serialize, Deserialize)]
struct CardCache {
    synced_at: u64,
    cards: Vec<Card>,
}

pub fn cache_path(app: &AppHandle) -> Result<PathBuf, NotionError> {
    let dir = app.path().app_data_dir().map_err(storage_err)?;
    Ok(dir.join(CACHE_FILE))
}

pub fn write_cache(path: &Path, cards: &[Card]) -> Result<(), NotionError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(storage_err)?;
    }
    let synced_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    let cache = CardCache {
        synced_at,
        cards: cards.to_vec(),
    };
    let json = serde_json::to_string_pretty(&cache).map_err(storage_err)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json).map_err(storage_err)?;
    std::fs::rename(&tmp, path).map_err(storage_err)
}

pub fn read_cache(path: &Path) -> Result<Vec<Card>, NotionError> {
    match std::fs::read_to_string(path) {
        Ok(text) => {
            let cache: CardCache = serde_json::from_str(&text).map_err(storage_err)?;
            Ok(cache.cards)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(storage_err(e)),
    }
}

pub fn visible_cards(cards: Vec<Card>, hidden_statuses: &[String]) -> Vec<Card> {
    cards
        .into_iter()
        .filter(|card| match &card.status {
            Some(status) => !hidden_statuses
                .iter()
                .any(|h| h.trim().eq_ignore_ascii_case(status.trim())),
            None => true,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(phrase: &str, status: Option<&str>) -> Card {
        Card {
            id: phrase.to_string(),
            phrase: phrase.to_string(),
            translation: None,
            pronunciation: None,
            description: None,
            examples: vec![],
            synonym: None,
            situations: vec![],
            kind: None,
            status: status.map(str::to_string),
            added: None,
            last_reviewed: None,
        }
    }

    #[test]
    fn hides_mastered_by_default() {
        let cards = vec![
            card("a", Some("New")),
            card("b", Some("Mastered")),
            card("c", None),
            card("d", Some("mastered")),
        ];
        let visible = visible_cards(cards, &Settings::default().hidden_statuses);
        let phrases: Vec<&str> = visible.iter().map(|c| c.phrase.as_str()).collect();
        assert_eq!(phrases, vec!["a", "c"]);
    }

    #[test]
    fn empty_hidden_list_shows_everything() {
        let cards = vec![card("a", Some("Mastered")), card("b", Some("New"))];
        assert_eq!(visible_cards(cards, &[]).len(), 2);
    }

    #[test]
    fn old_or_partial_settings_json_gets_defaults() {
        let s: Settings = serde_json::from_str(r#"{"database_input":"x"}"#).unwrap();
        assert_eq!(s.database_input, "x");
        assert_eq!(s.hidden_statuses, vec!["Mastered"]);
        assert_eq!(s.mapping, Mapping::default());
    }

    #[test]
    fn settings_view_never_contains_token_field() {
        let view = SettingsView {
            settings: Settings::default(),
            has_token: true,
        };
        let json = serde_json::to_value(&view).unwrap();
        assert_eq!(json["has_token"], Value::Bool(true));
        assert!(json.get("token").is_none());
        assert!(json.get(KEY_TOKEN).is_none());
        assert!(json.get("mapping").is_some());
    }

    #[test]
    fn cache_roundtrip_and_missing_file() {
        let dir = std::env::temp_dir().join(format!("word-overlay-test-{}", std::process::id()));
        let path = dir.join(CACHE_FILE);
        assert!(read_cache(&path).unwrap().is_empty());

        let cards = vec![card("a", Some("New")), card("b", None)];
        write_cache(&path, &cards).unwrap();
        assert_eq!(read_cache(&path).unwrap(), cards);

        std::fs::write(&path, "не json").unwrap();
        assert!(matches!(read_cache(&path), Err(NotionError::Storage(_))));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
