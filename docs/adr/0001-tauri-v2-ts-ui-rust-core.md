# ADR 0001: Tauri v2, UI на TypeScript, ядро на Rust (гібрид «A+»)

- **Status:** Accepted
- **Date:** 2026-10-17

## Context

Потрібен маленький always-on-top, click-through оверлей у куті екрана, що показує фразу, вимову й переклад під час гри. Слова беруться з бази Notion; spaced repetition (FSRS) буде пізніше.

- Код пишемо на macOS, а ціль — Windows на ігровому ПК. Поведінку оверлея над іграми можна перевірити лише на Windows.
- Автор — початківець у Rust.
- Головні drivers: ризик оверлея над іграми (фокус, fullscreen, click-through, topmost), швидкість до робочого MVP і бажання вчити Rust, не блокуючи прогрес.

Розглянуті варіанти:

| Варіант | Суть | Чому ні |
|---|---|---|
| A. Tauri v2 + TS-heavy | Notion через `tauri-plugin-http`, SQLite через `tauri-plugin-sql`, `ts-fsrs` | Майже не вчить Rust, **токен живе у webview**, а Win32-дрібниці вікна однаково потребують Rust. Лишається фолбеком. |
| B. Tauri v2 + Rust-heavy | Notion, SQLite, черга й FSRS у Rust | Як стартова стратегія надто повільна для початківця. |
| C. Pure Rust (egui/winit) | Нативне вікно без webview | Прозоре click-through topmost вікно — багато ручної роботи, найвищий ризик не дійти до MVP. |
| Cross-compile з Mac (`cargo-xwin`) | — | Крихко, а тестувати все одно треба на Windows. |

Steelman (Architect): «Notion — низькоризикова задача для TS, а справжня потреба в Rust — Win32-шар вікна». Відповідь: driver «навчання Rust» виправдовує Notion у Rust як ізольовану, тестовану й безпечну задачу (до того ж токен лишається поза webview). Win32-шар однаково в Rust, тож одне не виключає іншого.

## Decision

Обрано **A+ (гібрид)**:

- Tauri v2 + React/TypeScript + Vite.
- UI, черга карток і (згодом) FSRS — на TypeScript.
- Синк із Notion, налаштування й токен — у Rust (`reqwest` + `serde`). Налаштування доступні лише через команди `get_settings` (без токена, з `has_token`), `set_settings` і `set_token`. Зберігання — через `StoreExt`, JS не має жодних `store:*` permissions (див. [0005](0005-notion-token-rust-only.md)).
- Capabilities розділено по вікнах (`src-tauri/capabilities/overlay.json`, `settings.json`).
- Vite multi-page: дві окремі HTML-сторінки `overlay.html` і `settings.html` без роутера (`build.rollupOptions.input` у `vite.config.ts`).
- Win32-шар вікна — у Rust (крейт `windows`, лише `cfg(windows)`), див. [0002](0002-overlay-window-no-focus.md).
- `main.rs` лише викликає `word_overlay_lib::run()`; уся логіка — у бібліотеці `lib.rs` (`crate-type = ["staticlib", "cdylib", "rlib"]`, як того вимагає Tauri). У release-збірці під Windows `windows_subsystem = "windows"`, щоб не відкривалося зайве консольне вікно.
- SQLite (`tauri-plugin-sql`) — після MVP; у MVP достатньо JSON-кешу.
- Розробка на Mac, перевірка оверлея на Windows ПК; інсталятор збирає CI `tauri-action`.
- Версії `tauri` і `@tauri-apps/api` запінені на конкретні 2.x (потрібен `Monitor.workArea`), оновлюємо їх свідомо.

## Consequences

- Дві мови й дубльовані типи: Rust struct ↔ TS type (наприклад, `notion::Card` ↔ `type Card` в оверлеї; поля в snake_case, як їх серіалізує serde).
- Немає CORS-проблем, токен не потрапляє у JS.
- Dev-середовище на двох машинах.
- MVP без FSRS і з plaintext-токеном (write-only з JS), інсталятор unsigned.
- Якщо спрацює фолбек A, токен опиниться у webview. Тоді capability `http:default` треба обмежити `allow: [{ "url": "https://api.notion.com/*" }]`.
- Follow-ups: запис `Last reviewed` / `Status` назад у Notion, авто-пауза для ігор зі списку винятків, `keyring`, перенос SQLite і черги в Rust, підпис коду.

## References

- `.omc/plans/word-overlay-plan.md`, §1 RALPLAN-DR і §6 ADR
