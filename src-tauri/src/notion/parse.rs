use serde_json::{Map, Value};

use super::{Card, Mapping, NotionError};

pub fn parse_page(
    json: &Value,
    mapping: &Mapping,
) -> Result<(Vec<Card>, Option<String>), NotionError> {
    let results = json
        .get("results")
        .and_then(Value::as_array)
        .ok_or_else(|| NotionError::BadResponse("немає поля results".into()))?;

    let mut cards = Vec::new();
    for page in results {
        if page.get("object").and_then(Value::as_str) != Some("page") {
            continue;
        }
        let props = page
            .get("properties")
            .and_then(Value::as_object)
            .ok_or_else(|| NotionError::BadResponse("у сторінки немає properties".into()))?;

        check_mapping(props, mapping)?;

        let Some(phrase) = title_text(props) else {
            continue;
        };

        let mut examples = Vec::new();
        if let Some(first) = text(props, &mapping.example) {
            examples.push(first);
        }
        if let Some(more) = text(props, &mapping.more_examples) {
            examples.extend(split_lines(&more));
        }

        cards.push(Card {
            id: page
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            phrase,
            translation: text(props, &mapping.translation),
            pronunciation: text(props, &mapping.pronunciation),
            description: text(props, &mapping.description),
            examples,
            synonym: text(props, &mapping.synonym),
            situations: names(props, &mapping.situation),
            kind: text(props, &mapping.kind),
            status: text(props, &mapping.status),
            added: text(props, &mapping.added),
            last_reviewed: text(props, &mapping.last_reviewed),
        });
    }

    let has_more = json
        .get("has_more")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let next_cursor = if has_more {
        let cursor = json
            .get("next_cursor")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                NotionError::BadResponse("has_more=true, але немає next_cursor".into())
            })?;
        Some(cursor.to_string())
    } else {
        None
    };
    Ok((cards, next_cursor))
}

fn check_mapping(props: &Map<String, Value>, mapping: &Mapping) -> Result<(), NotionError> {
    let missing: Vec<&str> = mapping
        .column_names()
        .into_iter()
        .filter(|name| !props.contains_key(*name))
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    let mut available: Vec<&str> = props.keys().map(String::as_str).collect();
    available.sort();
    Err(NotionError::Mapping {
        missing: missing.join(", "),
        available: available.join(", "),
    })
}

fn title_text(props: &Map<String, Value>) -> Option<String> {
    props
        .values()
        .find(|p| p.get("type").and_then(Value::as_str) == Some("title"))
        .and_then(value_text)
}

fn text(props: &Map<String, Value>, column: &str) -> Option<String> {
    if column.trim().is_empty() {
        return None;
    }
    props.get(column).and_then(value_text)
}

fn value_text(prop: &Value) -> Option<String> {
    let kind = prop.get("type")?.as_str()?;
    let value = prop.get(kind)?;
    let text = match kind {
        "title" | "rich_text" => value
            .as_array()?
            .iter()
            .filter_map(|seg| seg.get("plain_text").and_then(Value::as_str))
            .collect::<String>(),
        "select" | "status" => value.get("name")?.as_str()?.to_string(),
        "multi_select" => option_names(value).join(", "),
        "date" => value.get("start")?.as_str()?.to_string(),
        _ => return None,
    };
    let text = text.trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn names(props: &Map<String, Value>, column: &str) -> Vec<String> {
    if column.trim().is_empty() {
        return Vec::new();
    }
    let Some(prop) = props.get(column) else {
        return Vec::new();
    };
    match prop.get("type").and_then(Value::as_str) {
        Some("multi_select") => prop
            .get("multi_select")
            .map(option_names)
            .unwrap_or_default(),
        Some("select") | Some("status") => value_text(prop).into_iter().collect(),
        _ => Vec::new(),
    }
}

fn option_names(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|o| o.get("name").and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn split_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(|line| {
            line.trim()
                .trim_start_matches(['•', '-', '*'])
                .trim()
                .to_string()
        })
        .filter(|line| !line.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const PAGE_1: &str = include_str!("fixtures/query_page_1.json");
    const PAGE_2: &str = include_str!("fixtures/query_page_2.json");

    fn load(s: &str) -> Value {
        serde_json::from_str(s).expect("фікстура — валідний JSON")
    }

    #[test]
    fn parses_card_with_bullets_situations_and_empty_date() {
        let (cards, cursor) = parse_page(&load(PAGE_1), &Mapping::default()).unwrap();
        assert_eq!(cursor.as_deref(), Some("cursor-page-2"));
        assert_eq!(cards.len(), 2);

        let c = &cards[0];
        assert_eq!(c.id, "11111111-1111-4111-8111-111111111111");
        assert_eq!(c.phrase, "Whether we like it or not, …");
        assert_eq!(
            c.pronunciation.as_deref(),
            Some("/ˈweðər wi laɪk ɪt ɔːr nɑːt/")
        );
        assert_eq!(c.translation.as_deref(), Some("Подобається нам чи ні, …"));
        assert_eq!(
            c.examples,
            vec![
                "Whether we like it or not, the deadline is Friday.",
                "Whether we like it or not, the tests have to pass first.",
                "Whether we like it or not, we need to talk about scope.",
                "Whether we like it or not, the client wants a demo.",
            ]
        );
        assert_eq!(c.situations, vec!["Stand-up", "Technical discussion"]);
        assert_eq!(c.kind.as_deref(), Some("Starter"));
        assert_eq!(c.status.as_deref(), Some("Practicing"));
        assert_eq!(c.added.as_deref(), Some("2026-09-20"));
        assert_eq!(c.last_reviewed, None);
        assert_eq!(c.synonym, None);
    }

    #[test]
    fn concatenates_multi_segment_rich_text() {
        let (cards, _) = parse_page(&load(PAGE_1), &Mapping::default()).unwrap();
        let c = &cards[1];
        assert_eq!(c.phrase, "Where it gets tricky is …");
        assert_eq!(
            c.description.as_deref(),
            Some("Use it to point at the hard part of a problem.")
        );
        assert_eq!(c.kind.as_deref(), Some("Mistake fix"));
        assert_eq!(c.status.as_deref(), Some("New"));
        assert_eq!(
            c.examples,
            vec!["The UI part is easy. Where it gets tricky is the sync."]
        );
    }

    #[test]
    fn last_page_has_no_cursor_and_status_type_and_blank_rows_skipped() {
        let (cards, cursor) = parse_page(&load(PAGE_2), &Mapping::default()).unwrap();
        assert_eq!(cursor, None);
        assert_eq!(cards.len(), 1);
        let c = &cards[0];
        assert_eq!(c.phrase, "Fair point");
        assert_eq!(c.status.as_deref(), Some("Mastered"));
        assert!(c.examples.is_empty());
        assert!(c.situations.is_empty());
        assert_eq!(c.kind, None);
        assert_eq!(c.synonym.as_deref(), Some("Good point"));
    }

    #[test]
    fn unknown_columns_are_ignored() {
        let page = load(PAGE_1);
        assert!(page["results"][0]["properties"].get("Score").is_some());
        assert!(parse_page(&page, &Mapping::default()).is_ok());
    }

    #[test]
    fn mapped_column_with_unknown_type_becomes_none() {
        let mapping = Mapping {
            translation: "Notes".into(),
            ..Mapping::default()
        };
        let (cards, _) = parse_page(&load(PAGE_1), &mapping).unwrap();
        assert_eq!(cards[0].translation, None);
    }

    #[test]
    fn missing_mapped_column_lists_available() {
        let mapping = Mapping {
            translation: "Переклад".into(),
            ..Mapping::default()
        };
        let err = parse_page(&load(PAGE_1), &mapping).unwrap_err();
        match err {
            NotionError::Mapping { missing, available } => {
                assert_eq!(missing, "Переклад");
                assert!(available.contains("Translation"));
                assert!(available.contains("Phrase"));
            }
            other => panic!("очікували Mapping, отримали {other:?}"),
        }
    }

    #[test]
    fn empty_mapping_field_disables_column() {
        let mapping = Mapping {
            synonym: String::new(),
            ..Mapping::default()
        };
        let (cards, _) = parse_page(&load(PAGE_2), &mapping).unwrap();
        assert_eq!(cards[0].synonym, None);
    }

    #[test]
    fn bad_response_is_error_not_panic() {
        assert!(matches!(
            parse_page(&json!({"object": "error"}), &Mapping::default()),
            Err(NotionError::BadResponse(_))
        ));
        assert!(matches!(
            parse_page(
                &json!({"results": [], "has_more": true}),
                &Mapping::default()
            ),
            Err(NotionError::BadResponse(_))
        ));
    }

    #[test]
    fn split_lines_trims_bullets() {
        assert_eq!(
            split_lines("• one\n\n - two \r\n* three\n   \n•"),
            vec!["one", "two", "three"]
        );
    }
}
