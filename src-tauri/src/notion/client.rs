use reqwest::{header, Client, RequestBuilder, StatusCode};
use serde_json::{json, Value};

use super::NotionError;

pub const NOTION_VERSION: &str = "2025-09-03";
pub const NOTION_BASE_URL: &str = "https://api.notion.com";
const PAGE_SIZE: u32 = 100;

pub struct NotionClient {
    http: Client,
    token: String,
    base_url: String,
}

impl NotionClient {
    pub fn new(token: impl Into<String>, base_url: impl Into<String>) -> Result<Self, NotionError> {
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        Ok(NotionClient {
            http,
            token: token.into(),
            base_url: base_url.into(),
        })
    }

    pub async fn get_database(&self, id: &str) -> Result<Value, NotionError> {
        let url = format!("{}/v1/databases/{id}", self.base_url);
        self.send(self.http.get(url)).await
    }

    pub async fn get_data_source(&self, id: &str) -> Result<Value, NotionError> {
        let url = format!("{}/v1/data_sources/{id}", self.base_url);
        self.send(self.http.get(url)).await
    }

    pub async fn query(
        &self,
        data_source_id: &str,
        cursor: Option<String>,
    ) -> Result<Value, NotionError> {
        let url = format!("{}/v1/data_sources/{data_source_id}/query", self.base_url);
        let mut body = json!({ "page_size": PAGE_SIZE });
        if let Some(c) = cursor {
            body["start_cursor"] = Value::String(c);
        }
        self.send(self.http.post(url).json(&body)).await
    }

    async fn send(&self, request: RequestBuilder) -> Result<Value, NotionError> {
        let response = request
            .bearer_auth(&self.token)
            .header("Notion-Version", NOTION_VERSION)
            .send()
            .await?;
        let status = response.status();
        if status.is_success() {
            return Ok(response.json::<Value>().await?);
        }
        let retry_after = response
            .headers()
            .get(header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let body = response.text().await.unwrap_or_default();
        Err(error_from_status(status, retry_after.as_deref(), &body))
    }
}

pub fn error_from_status(status: StatusCode, retry_after: Option<&str>, body: &str) -> NotionError {
    match status {
        StatusCode::UNAUTHORIZED => NotionError::Unauthorized,
        StatusCode::NOT_FOUND => NotionError::NotFound,
        StatusCode::TOO_MANY_REQUESTS => NotionError::RateLimited {
            retry_after: retry_after.and_then(|s| s.trim().parse::<u64>().ok()),
        },
        _ => {
            let parsed: Value = serde_json::from_str(body).unwrap_or(Value::Null);
            let field = |name: &str| {
                parsed
                    .get(name)
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string()
            };
            NotionError::Api {
                status: status.as_u16(),
                code: field("code"),
                message: field("message"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_status_codes() {
        assert!(matches!(
            error_from_status(StatusCode::UNAUTHORIZED, None, ""),
            NotionError::Unauthorized
        ));
        assert!(matches!(
            error_from_status(StatusCode::NOT_FOUND, None, ""),
            NotionError::NotFound
        ));
        assert!(matches!(
            error_from_status(StatusCode::TOO_MANY_REQUESTS, Some("7"), ""),
            NotionError::RateLimited {
                retry_after: Some(7)
            }
        ));
        assert!(matches!(
            error_from_status(StatusCode::TOO_MANY_REQUESTS, Some("soon"), ""),
            NotionError::RateLimited { retry_after: None }
        ));
        let body =
            r#"{"object":"error","status":400,"code":"validation_error","message":"bad filter"}"#;
        match error_from_status(StatusCode::BAD_REQUEST, None, body) {
            NotionError::Api {
                status,
                code,
                message,
            } => {
                assert_eq!(
                    (status, code.as_str(), message.as_str()),
                    (400, "validation_error", "bad filter")
                );
            }
            other => panic!("очікували Api, отримали {other:?}"),
        }
    }
}
