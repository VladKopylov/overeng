use std::future::Future;
use std::time::Duration;

use serde_json::Value;

use super::parse::parse_page;
use super::{Card, DataSourceOption, Mapping, NotionError};

pub const MAX_RETRIES: u32 = 3;
const DEFAULT_RETRY_SECS: u64 = 1;
const MAX_RETRY_SECS: u64 = 60;
const MAX_PAGES: usize = 100;

pub async fn retry_on_rate_limit<T, Op, OpFut, S, SFut>(
    mut op: Op,
    sleep: &mut S,
) -> Result<T, NotionError>
where
    Op: FnMut() -> OpFut,
    OpFut: Future<Output = Result<T, NotionError>>,
    S: FnMut(Duration) -> SFut,
    SFut: Future<Output = ()>,
{
    let mut retries = 0;
    loop {
        match op().await {
            Err(NotionError::RateLimited { retry_after }) if retries < MAX_RETRIES => {
                retries += 1;
                let secs = retry_after
                    .unwrap_or(DEFAULT_RETRY_SECS)
                    .min(MAX_RETRY_SECS);
                sleep(Duration::from_secs(secs)).await;
            }
            other => return other,
        }
    }
}

pub async fn fetch_all<F, Fut, S, SFut>(
    mut fetch: F,
    mut sleep: S,
    mapping: &Mapping,
) -> Result<Vec<Card>, NotionError>
where
    F: FnMut(Option<String>) -> Fut,
    Fut: Future<Output = Result<Value, NotionError>>,
    S: FnMut(Duration) -> SFut,
    SFut: Future<Output = ()>,
{
    let mut cards = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..MAX_PAGES {
        let page = retry_on_rate_limit(|| fetch(cursor.clone()), &mut sleep).await?;
        let (mut batch, next) = parse_page(&page, mapping)?;
        cards.append(&mut batch);
        match next {
            Some(next) => cursor = Some(next),
            None => return Ok(cards),
        }
    }
    Err(NotionError::BadResponse(format!(
        "більше {MAX_PAGES} сторінок — зупинились"
    )))
}

pub async fn resolve_data_source<GD, GDFut, GS, GSFut>(
    input: &str,
    get_database: GD,
    get_data_source: GS,
) -> Result<Vec<DataSourceOption>, NotionError>
where
    GD: FnOnce(String) -> GDFut,
    GDFut: Future<Output = Result<Value, NotionError>>,
    GS: FnOnce(String) -> GSFut,
    GSFut: Future<Output = Result<Value, NotionError>>,
{
    let id =
        super::extract_id(input).ok_or_else(|| NotionError::InvalidInput(input.trim().into()))?;
    match get_database(id.clone()).await {
        Ok(db) => {
            let options = options_from_database(&db);
            if options.is_empty() {
                Err(NotionError::BadResponse("у бази немає data sources".into()))
            } else {
                Ok(options)
            }
        }
        Err(NotionError::NotFound) => match get_data_source(id).await {
            Ok(ds) => Ok(vec![option_from_data_source(&ds)?]),
            Err(NotionError::NotFound) => Err(NotionError::NotShared),
            Err(e) => Err(e),
        },
        Err(e) => Err(e),
    }
}

fn options_from_database(db: &Value) -> Vec<DataSourceOption> {
    db.get("data_sources")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|ds| {
                    Some(DataSourceOption {
                        id: ds.get("id")?.as_str()?.to_string(),
                        name: ds
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn option_from_data_source(ds: &Value) -> Result<DataSourceOption, NotionError> {
    let id = ds
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| NotionError::BadResponse("у data source немає id".into()))?;
    let name = ds
        .get("title")
        .and_then(Value::as_array)
        .map(|segs| {
            segs.iter()
                .filter_map(|s| s.get("plain_text").and_then(Value::as_str))
                .collect::<String>()
        })
        .unwrap_or_default();
    Ok(DataSourceOption {
        id: id.to_string(),
        name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::cell::RefCell;
    use std::future::ready;

    use tauri::async_runtime::block_on;

    fn page(prefix: &str, n: usize, next: Option<&str>) -> Value {
        let results: Vec<Value> = (0..n)
            .map(|i| {
                json!({
                    "object": "page",
                    "id": format!("{prefix}-{i}"),
                    "properties": {
                        "Phrase": { "type": "title", "title": [{ "plain_text": format!("{prefix} phrase {i}") }] }
                    }
                })
            })
            .collect();
        json!({ "object": "list", "results": results, "has_more": next.is_some(), "next_cursor": next })
    }

    fn phrase_only() -> Mapping {
        Mapping {
            translation: String::new(),
            pronunciation: String::new(),
            description: String::new(),
            example: String::new(),
            more_examples: String::new(),
            synonym: String::new(),
            situation: String::new(),
            kind: String::new(),
            status: String::new(),
            added: String::new(),
            last_reviewed: String::new(),
        }
    }

    fn fake_sleep(
        log: &RefCell<Vec<Duration>>,
    ) -> impl FnMut(Duration) -> std::future::Ready<()> + '_ {
        move |d| {
            log.borrow_mut().push(d);
            ready(())
        }
    }

    #[test]
    fn pagination_follows_cursors_over_three_pages() {
        let seen = RefCell::new(Vec::<Option<String>>::new());
        let slept = RefCell::new(Vec::new());
        let fetch = |cursor: Option<String>| {
            seen.borrow_mut().push(cursor.clone());
            let result = match cursor.as_deref() {
                None => Ok(page("a", 100, Some("c2"))),
                Some("c2") => Ok(page("b", 100, Some("c3"))),
                Some("c3") => Ok(page("c", 5, None)),
                Some(other) => Err(NotionError::BadResponse(format!(
                    "невідомий курсор {other}"
                ))),
            };
            ready(result)
        };
        let cards = block_on(fetch_all(fetch, fake_sleep(&slept), &phrase_only())).unwrap();
        assert_eq!(cards.len(), 205);
        assert_eq!(cards[0].phrase, "a phrase 0");
        assert_eq!(cards[204].phrase, "c phrase 4");
        assert_eq!(
            *seen.borrow(),
            vec![None, Some("c2".to_string()), Some("c3".to_string())]
        );
        assert!(slept.borrow().is_empty());
    }

    #[test]
    fn retries_after_429_honoring_retry_after() {
        let calls = RefCell::new(0);
        let slept = RefCell::new(Vec::new());
        let fetch = |_cursor: Option<String>| {
            *calls.borrow_mut() += 1;
            let n = *calls.borrow();
            ready(match n {
                1 => Err(NotionError::RateLimited {
                    retry_after: Some(2),
                }),
                2 => Err(NotionError::RateLimited { retry_after: None }),
                _ => Ok(page("a", 3, None)),
            })
        };
        let cards = block_on(fetch_all(fetch, fake_sleep(&slept), &phrase_only())).unwrap();
        assert_eq!(cards.len(), 3);
        assert_eq!(*calls.borrow(), 3);
        assert_eq!(
            *slept.borrow(),
            vec![Duration::from_secs(2), Duration::from_secs(1)]
        );
    }

    #[test]
    fn gives_up_after_max_retries_and_caps_wait() {
        let calls = RefCell::new(0u32);
        let slept = RefCell::new(Vec::new());
        let fetch = |_c: Option<String>| {
            *calls.borrow_mut() += 1;
            ready(Err::<Value, _>(NotionError::RateLimited {
                retry_after: Some(600),
            }))
        };
        let err = block_on(fetch_all(fetch, fake_sleep(&slept), &phrase_only())).unwrap_err();
        assert!(matches!(err, NotionError::RateLimited { .. }));
        assert_eq!(*calls.borrow(), 1 + MAX_RETRIES);
        assert!(slept.borrow().iter().all(|d| *d == Duration::from_secs(60)));
    }

    #[test]
    fn other_errors_are_not_retried() {
        let calls = RefCell::new(0u32);
        let slept = RefCell::new(Vec::new());
        let fetch = |_c: Option<String>| {
            *calls.borrow_mut() += 1;
            ready(Err::<Value, _>(NotionError::Unauthorized))
        };
        let err = block_on(fetch_all(fetch, fake_sleep(&slept), &phrase_only())).unwrap_err();
        assert!(matches!(err, NotionError::Unauthorized));
        assert_eq!(*calls.borrow(), 1);
    }

    const DATABASE: &str = include_str!("fixtures/database.json");
    const DATA_SOURCE: &str = include_str!("fixtures/data_source.json");
    const DB_URL: &str =
        "https://www.notion.so/ws/Synthetic-Vocab-bbbbbbbb000040008000000000000002?v=1";
    const DB_ID: &str = "bbbbbbbb-0000-4000-8000-000000000002";

    fn fixture(s: &str) -> Value {
        serde_json::from_str(s).unwrap()
    }

    #[test]
    fn resolve_database_with_two_data_sources_returns_both() {
        let requested = RefCell::new(None);
        let options = block_on(resolve_data_source(
            DB_URL,
            |id| {
                *requested.borrow_mut() = Some(id);
                ready(Ok(fixture(DATABASE)))
            },
            |_| ready(Err(NotionError::NotFound)),
        ))
        .unwrap();
        assert_eq!(requested.borrow().as_deref(), Some(DB_ID));
        assert_eq!(options.len(), 2);
        assert_eq!(options[1].name, "Archive");
    }

    #[test]
    fn resolve_falls_back_to_data_source_on_404() {
        let options = block_on(resolve_data_source(
            "aaaaaaaa-0000-4000-8000-000000000001",
            |_| ready(Err(NotionError::NotFound)),
            |_| ready(Ok(fixture(DATA_SOURCE))),
        ))
        .unwrap();
        assert_eq!(
            options,
            vec![DataSourceOption {
                id: "aaaaaaaa-0000-4000-8000-000000000001".into(),
                name: "Synthetic Vocab".into()
            }]
        );
    }

    #[test]
    fn not_shared_only_when_both_lookups_404() {
        let err = block_on(resolve_data_source(
            DB_ID,
            |_| ready(Err::<Value, _>(NotionError::NotFound)),
            |_| ready(Err::<Value, _>(NotionError::NotFound)),
        ))
        .unwrap_err();
        assert!(matches!(err, NotionError::NotShared));

        let err = block_on(resolve_data_source(
            DB_ID,
            |_| ready(Err::<Value, _>(NotionError::Unauthorized)),
            |_| ready(Err::<Value, _>(NotionError::NotFound)),
        ))
        .unwrap_err();
        assert!(matches!(err, NotionError::Unauthorized));
    }

    #[test]
    fn resolve_rejects_input_without_id() {
        let err = block_on(resolve_data_source(
            "моя база",
            |_| ready(Err::<Value, _>(NotionError::NotFound)),
            |_| ready(Err::<Value, _>(NotionError::NotFound)),
        ))
        .unwrap_err();
        assert!(matches!(err, NotionError::InvalidInput(_)));
    }
}
