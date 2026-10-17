# Architecture Decision Records

Ключові архітектурні рішення Word Overlay. Кожен ADR має розділи Context, Decision, Consequences і, де є посилання, References.

| № | Рішення | Статус |
|---|---|---|
| [0001](0001-tauri-v2-ts-ui-rust-core.md) | Tauri v2, UI на TypeScript, ядро на Rust (гібрид «A+») | Accepted |
| [0002](0002-overlay-window-no-focus.md) | Вікно оверлея ніколи не забирає фокус у гри | Accepted |
| [0003](0003-overlay-positioning-physical-pixels.md) | Позиціонування оверлея в physical pixels | Accepted, розрахунок замінено 0015 |
| [0004](0004-hotkeys-registered-in-rust.md) | Глобальні hotkeys реєструються в Rust | Accepted |
| [0005](0005-notion-token-rust-only.md) | Токен Notion живе лише в Rust | Accepted |
| [0006](0006-notion-api-2025-09-03-data-sources.md) | Notion API 2025-09-03 і data sources | Accepted |
| [0007](0007-notion-property-parsing-and-mapping.md) | Парсинг властивостей Notion і мапінг колонок | Accepted |
| [0008](0008-tls-native-tls.md) | TLS через native-tls | Accepted |
| [0009](0009-cards-cache-and-sync-guard.md) | Кеш карток і захист синку | Accepted |
| [0010](0010-debug-log.md) | Debug-лог `overlay-debug.log` | Accepted |
| [0011](0011-react-strictmode-listener-cleanup.md) | Відписка від подій Tauri під React StrictMode | Accepted |
| [0012](0012-overlay-card-layout.md) | Верстка картки оверлея | Accepted |
| [0013](0013-build-ci-and-repo-hygiene.md) | Збірка, CI і правила репозиторію | Accepted |
| [0014](0014-overlay-controls-modes-and-layout-state.md) | Керування оверлеєм: компактний/розгорнутий режим, сховати/показати, режим редагування | Accepted |
| [0015](0015-overlay-geometry-pure-rust.md) | Геометрія оверлея — чиста функція в Rust | Accepted |
| [0016](0016-edit-mode-drag-noactivate-window.md) | Перетягування в режимі редагування на вікні без фокусу | Accepted |
| [0017](0017-exclusive-fullscreen-unsupported.md) | Exclusive fullscreen не підтримується — свідомо | Accepted |

Новий ADR: наступний номер, формат `NNNN-short-title.md`, ті самі розділи, і рядок у цій таблиці.
