# ADR 0013: Збірка, CI і правила репозиторію

- **Status:** Accepted
- **Date:** 2026-10-17

## Context

Розробка йде на Mac, а запуск — на Windows ПК. Потрібні передбачуваний dev-сервер для Tauri, інсталятор Windows без ручної збірки і захист від випадкового коміту секретів чи реальних даних.

## Decision

**Vite** (`vite.config.ts`):

- Tauri очікує фіксований порт: `port: 1420`, `strictPort: true` (збігається з `devUrl` у `src-tauri/tauri.conf.json`).
- `TAURI_DEV_HOST` Tauri CLI задає лише для мобільної розробки, на десктопі він `undefined`. Якщо він заданий, вмикаємо `host` і HMR через `ws` на порту 1421.
- `src-tauri/**` виключено з watch.
- Multi-page збірка: `overlay.html` і `settings.html` (див. [0001](0001-tauri-v2-ts-ui-rust-core.md)).

**CI** (`.github/workflows/windows-build.yml`): на кожен push у `main` або вручну (`workflow_dispatch`, Actions → Run workflow) на `windows-latest` збирається Tauri-застосунок через `tauri-apps/tauri-action`. `.msi` і `.exe` (NSIS) завантажуються як artifact `word-overlay-windows`; якщо файлів нема, job падає.

**`.gitignore`:**

- Ігноруються залежності й збірки (`node_modules/`, `dist/`, `src-tauri/target/`) і стан агентів-помічників (`.omc/`).
- `src-tauri/gen/schemas` свідомо **не** ігнорується: там схеми для автодоповнення capabilities.
- Секрети: `*.env`, `.env*`, `secrets*`, `*.secret` і лише кореневий `/settings.json`. `src-tauri/capabilities/settings.json` — не секрет.
- Реальні дані з Notion не комітимо (`tests/fixtures/real/`), у фікстурах лише синтетика.
- Логи (`*.log`) і файли ОС (`.DS_Store`, `Thumbs.db`).

## Consequences

- Інсталятор Windows доступний з кожного коміту в `main` без тулчейну на ігровому ПК, але він unsigned (SmartScreen).
- Cross-compile з Mac не використовуємо.

## References

- `.omc/plans/word-overlay-plan.md`, §3 «Workflow збірки під Windows»
