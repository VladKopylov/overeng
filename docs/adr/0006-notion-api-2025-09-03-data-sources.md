# ADR 0006: Notion API 2025-09-03 і data sources

- **Status:** Accepted
- **Date:** 2026-10-17

## Context

З версії `Notion-Version: 2025-09-03` записи читаються з **data source**, а не з database. Користувач копіює з браузера URL бази, тобто `database_id`, а запиту потрібен `data_source_id`. У бази може бути кілька data sources. Ендпоінти звірено з офіційним API reference 2026-10-17.

## Decision

**Версія і клієнт** (`src-tauri/src/notion/client.rs`):

- `NOTION_VERSION = "2025-09-03"` фіксуємо, бо від неї залежить формат відповідей. Базова адреса `https://api.notion.com` (у тестах її можна підмінити).
- HTTP timeout — 30 с.

**Ендпоінти:**

| Запит | Призначення |
|---|---|
| `GET /v1/databases/{database_id}` | З 2025-09-03 повертає `data_sources: [{ id, name }]` |
| `GET /v1/data_sources/{data_source_id}` | `id` + `title` (масив rich text) |
| `POST /v1/data_sources/{data_source_id}/query` | Тіло: `page_size` + `start_cursor`; відповідь: `results`, `has_more`, `next_cursor` |

Query викликається саме методом **POST**, як у reference і в офіційному TS SDK. Коментар `// PATCH` в upgrade guide — описка документації. `page_size = 100`, це максимум Notion.

**Що вставив користувач** (`extract_id` у `notion/mod.rs`): URL бази (з `?v=...` і назвою сторінки), голий ID з дефісами чи без, або `data_source_id`.

- Відкидаємо `?...` і `#...` (там ID вигляду, а не бази), беремо останній сегмент шляху, прибираємо дефіси.
- ID — останні 32 символи, усі hex. Результат нормалізується до нижнього регістру у форматі 8-4-4-4-12.
- Працюємо з `Vec<char>`, а не з байтами: у назві сторінки може бути кирилиця, а різати `&str` посеред символу в Rust не можна (паніка).
- Якщо перед 32 hex стоїть ще hex-символ без дефіса-розділювача (наприклад, 33 hex поспіль), це не ID.

**Резолв data source** (`notion::sync::resolve_data_source`):

1. `GET /v1/databases/{id}` → `data_sources[]`. Якщо їх кілька, UI показує picker; якщо список порожній, повертаємо `BadResponse`.
2. Якщо 404, пробуємо той самий ID як data source: `GET /v1/data_sources/{id}`.
3. Помилку «база не розшарена» (`NotShared`) показуємо лише тоді, коли **обидва** запити дали 404. Будь-яка інша помилка (наприклад, 401) повертається як є.

**404 під час синку** означає, що базу перестали шерити з інтеграцією або видалили. Тому `run_sync` перетворює `NotFound` на `NotShared` з підказкою, що робити (••• бази → Connections).

**Ліміти й 429** (`retry_on_rate_limit`):

- 429 → `RateLimited { retry_after }`. `Retry-After` — ціле число секунд; якщо його не вдалося розібрати, буде `None`.
- До `MAX_RETRIES = 3` повторів (разом 1 + 3 спроби). Без `Retry-After` чекаємо 1 с, але ніколи не довше 60 с.
- Інші помилки не повторюємо.
- `sleep` передається ззовні: у програмі це `tokio::time::sleep` (крейт `tokio` лише з feature `time`, бо tokio вже є всередині Tauri), а в тестах — миттєвий фейк.

**Пагінація** (`fetch_all`): беремо сторінку, парсимо, йдемо за `next_cursor`. Запобіжник `MAX_PAGES = 100` (100 × 100 = 10 000 карток) захищає від нескінченного циклу, після нього повертаємо `BadResponse`.

**Тестованість:** `fetch_all` і `resolve_data_source` не роблять HTTP самі, а отримують замикання (dependency injection). У програмі це `NotionClient`, у тестах — фейки з фікстурами, тож пагінацію й 429 тестуємо без мережі. Асинхронні тести запускаються через `tauri::async_runtime::block_on`.

**Помилки** (`notion/error.rs`): один `enum NotionError` на `thiserror`. Тексти українською, бо їх бачить користувач. У JS помилка серіалізується як рядок (людський текст). Тіло помилки Notion має формат `{ "code": "...", "message": "..." }` і для інших статусів стає `NotionError::Api`.

## Consequences

- Користувач може вставити будь-який із трьох форматів, а «не розшарена» не з'являється хибно, коли ID — це data source.
- Прив'язка до версії API 2025-09-03; оновлення версії — свідоме рішення.
- Довгий rate limit (понад 3 повтори) завершує синк помилкою, але кеш лишається (див. [0009](0009-cards-cache-and-sync-guard.md)).

## References

- Retrieve a database: https://developers.notion.com/reference/retrieve-a-database
- Upgrade guide 2025-09-03: https://developers.notion.com/docs/upgrade-guide-2025-09-03
- Retrieve a data source: https://developers.notion.com/reference/retrieve-a-data-source
- Query a data source: https://developers.notion.com/reference/query-a-data-source
- Request limits: https://developers.notion.com/reference/request-limits
- Status codes: https://developers.notion.com/reference/status-codes
