pub mod client;
pub mod error;
pub mod parse;
pub mod sync;

pub use error::NotionError;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Card {
    pub id: String,
    pub phrase: String,
    pub translation: Option<String>,
    pub pronunciation: Option<String>,
    pub description: Option<String>,
    pub examples: Vec<String>,
    pub synonym: Option<String>,
    pub situations: Vec<String>,
    pub kind: Option<String>,
    pub status: Option<String>,
    pub added: Option<String>,
    pub last_reviewed: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Mapping {
    pub translation: String,
    pub pronunciation: String,
    pub description: String,
    pub example: String,
    pub more_examples: String,
    pub synonym: String,
    pub situation: String,
    pub kind: String,
    pub status: String,
    pub added: String,
    pub last_reviewed: String,
}

impl Default for Mapping {
    fn default() -> Self {
        Mapping {
            translation: "Translation".into(),
            pronunciation: "Pronunciation".into(),
            description: "Description".into(),
            example: "Example".into(),
            more_examples: "More examples".into(),
            synonym: "Synonym".into(),
            situation: "Situation".into(),
            kind: "Type".into(),
            status: "Status".into(),
            added: "Added".into(),
            last_reviewed: "Last reviewed".into(),
        }
    }
}

impl Mapping {
    pub fn column_names(&self) -> Vec<&str> {
        [
            &self.translation,
            &self.pronunciation,
            &self.description,
            &self.example,
            &self.more_examples,
            &self.synonym,
            &self.situation,
            &self.kind,
            &self.status,
            &self.added,
            &self.last_reviewed,
        ]
        .into_iter()
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.as_str())
        .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataSourceOption {
    pub id: String,
    pub name: String,
}

pub fn extract_id(input: &str) -> Option<String> {
    let without_query = input.trim().split(['?', '#']).next()?;
    let last = without_query.trim_end_matches('/').rsplit('/').next()?;
    let compact: Vec<char> = last.chars().filter(|c| *c != '-').collect();
    if compact.len() < 32 {
        return None;
    }
    let split = compact.len() - 32;
    let (before, hex) = compact.split_at(split);
    if !hex.iter().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    if before.last().is_some_and(|c| c.is_ascii_hexdigit()) && !last.contains('-') {
        return None;
    }
    let h: String = hex.iter().collect::<String>().to_ascii_lowercase();
    Some(format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "60b3dc5f-0844-44c2-9f5e-2997b50dcb98";

    #[test]
    fn extract_id_from_url_with_view() {
        let url = "https://www.notion.so/myspace/English-Vocab-Active-Use-60b3dc5f084444c29f5e2997b50dcb98?v=1234567890abcdef1234567890abcdef";
        assert_eq!(extract_id(url).as_deref(), Some(ID));
    }

    #[test]
    fn extract_id_from_url_without_title() {
        let url = "https://www.notion.so/60b3dc5f084444c29f5e2997b50dcb98?v=abc";
        assert_eq!(extract_id(url).as_deref(), Some(ID));
    }

    #[test]
    fn extract_id_from_bare_ids() {
        assert_eq!(extract_id(ID).as_deref(), Some(ID));
        assert_eq!(
            extract_id("60b3dc5f084444c29f5e2997b50dcb98").as_deref(),
            Some(ID)
        );
        assert_eq!(
            extract_id("  60B3DC5F-0844-44C2-9F5E-2997B50DCB98 ").as_deref(),
            Some(ID)
        );
        assert_eq!(
            extract_id("6b0b8ba2-a8cc-470d-854f-8e7435a2cfbf").as_deref(),
            Some("6b0b8ba2-a8cc-470d-854f-8e7435a2cfbf")
        );
    }

    #[test]
    fn extract_id_from_url_with_hyphenated_id_and_trailing_slash() {
        let url = "https://notion.so/ws/60b3dc5f-0844-44c2-9f5e-2997b50dcb98/";
        assert_eq!(extract_id(url).as_deref(), Some(ID));
    }

    #[test]
    fn extract_id_from_url_with_cyrillic_title() {
        let url = "https://www.notion.so/Словник-60b3dc5f084444c29f5e2997b50dcb98";
        assert_eq!(extract_id(url).as_deref(), Some(ID));
    }

    #[test]
    fn extract_id_rejects_garbage() {
        assert_eq!(extract_id(""), None);
        assert_eq!(extract_id("hello world"), None);
        assert_eq!(
            extract_id("https://www.notion.so/myspace/Short-1234abcd"),
            None
        );
        assert_eq!(extract_id("060b3dc5f084444c29f5e2997b50dcb98"), None);
        assert_eq!(extract_id("60b3dc5f084444c29f5e2997b50dcbzz"), None);
    }

    #[test]
    fn default_mapping_matches_user_db() {
        let m = Mapping::default();
        assert_eq!(m.kind, "Type");
        assert_eq!(m.more_examples, "More examples");
        assert_eq!(m.last_reviewed, "Last reviewed");
        assert_eq!(m.column_names().len(), 11);
    }
}
