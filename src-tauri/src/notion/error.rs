use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum NotionError {
    #[error("Токен Notion не задано. Вставте його в налаштуваннях.")]
    NoToken,

    #[error("Базу не вибрано. Вставте посилання на базу і натисніть «Знайти».")]
    NoDataSource,

    #[error("Не бачу ID бази в «{0}». Вставте посилання на базу або її ID.")]
    InvalidInput(String),

    #[error("Невірний токен (Notion відповів 401). Перевірте токен інтеграції.")]
    Unauthorized,

    #[error("Notion не знайшов об'єкт (404).")]
    NotFound,

    #[error(
        "База не розшарена з інтеграцією. У Notion: ••• бази → Connections → додайте вашу інтеграцію."
    )]
    NotShared,

    #[error("Колонки {missing} не знайдено в базі. Доступні колонки: {available}.")]
    Mapping { missing: String, available: String },

    #[error("Notion тимчасово обмежив запити (429). Спробуйте пізніше — кеш карток збережено.")]
    RateLimited { retry_after: Option<u64> },

    #[error("Немає зв'язку з Notion ({0}). Спробуйте пізніше — кеш карток збережено.")]
    Network(String),

    #[error("Notion повернув помилку {status} ({code}): {message}")]
    Api {
        status: u16,
        code: String,
        message: String,
    },

    #[error("Неочікувана відповідь Notion: {0}")]
    BadResponse(String),

    #[error("Синхронізація вже триває, зачекайте.")]
    Busy,

    #[error("Помилка збереження на диск: {0}")]
    Storage(String),
}

impl From<reqwest::Error> for NotionError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_decode() {
            NotionError::BadResponse(e.to_string())
        } else {
            NotionError::Network(e.without_url().to_string())
        }
    }
}

impl Serialize for NotionError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_have_distinct_user_messages() {
        let unauthorized = NotionError::Unauthorized.to_string();
        let not_shared = NotionError::NotShared.to_string();
        let mapping = NotionError::Mapping {
            missing: "Translaton".into(),
            available: "Phrase, Translation".into(),
        }
        .to_string();
        let limited = NotionError::RateLimited { retry_after: None }.to_string();
        assert!(unauthorized.contains("Невірний токен"));
        assert!(not_shared.contains("Connections"));
        assert!(mapping.contains("Translaton") && mapping.contains("Phrase, Translation"));
        assert!(limited.contains("Спробуйте пізніше"));
        assert_ne!(unauthorized, not_shared);
    }

    #[test]
    fn serializes_as_plain_message() {
        let json = serde_json::to_string(&NotionError::Unauthorized).unwrap();
        assert!(json.starts_with('"') && json.contains("401"));
    }
}
