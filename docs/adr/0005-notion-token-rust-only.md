# ADR 0005: Токен Notion живе лише в Rust

- **Status:** Accepted
- **Date:** 2026-10-17

## Context

Для синку потрібен internal integration token Notion. Якщо токен потрапить у webview, його може прочитати будь-який JS у вікні, а також він може випадково опинитися в логах або повідомленнях про помилки.

## Decision

- Налаштування й токен зберігаються через **Rust API** `tauri-plugin-store` (`app.store(...)` з трейту `StoreExt`) у файлі `settings.json` у папці даних застосунку (наприклад, `%APPDATA%\com.vlad.wordoverlay\`). Токен лежить окремим ключем `notion_token`.
- **JS не має жодних `store:*` permissions** у capabilities. Доступ лише через наші команди в `lib.rs`.
- `set_token` — **write-only**: нічого не повертає й не логує сам токен (лише «токен збережено» / «токен видалено»). Значення обрізається (`trim`), порожній рядок видаляє ключ.
- `get_settings` повертає `SettingsView` = налаштування (через `#[serde(flatten)]`) + прапорець `has_token`. Поля з токеном там немає, і це перевіряє тест `settings_view_never_contains_token_field`.
- Команди не приймають токен аргументом: `notion_client()` бере його зі store сам.
- `NotionClient` свідомо **не** має `#[derive(Debug)]`, щоб токен не надрукувався через `{:?}`. Токен іде лише в заголовок `Authorization: Bearer`.
- Помилки store перетворюються на `NotionError::Storage` з текстом без токена (лише шлях або опис).
- Текст `reqwest::Error` містить лише URL (без заголовків), тож токен у повідомлення не потрапляє. Додатково `without_url()` прибирає адресу, щоб повідомлення було коротшим.
- У файлі `.gitignore` ігноруємо лише `/settings.json` у корені репо, бо `src-tauri/capabilities/settings.json` не є секретом.

## Consequences

- Токен ніколи не видно у webview, логах чи помилках.
- Токен лежить на диску **plaintext**. Ризик низький: інтеграція read-only і має доступ лише до однієї бази. Follow-up — `keyring`.
- Якщо колись спрацює фолбек A з [0001](0001-tauri-v2-ts-ui-rust-core.md), це рішення буде свідомо порушене.

## References

- `.omc/plans/word-overlay-plan.md`, §2 пункт 9, §6 ADR
