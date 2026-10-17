# ADR 0002: Вікно оверлея ніколи не забирає фокус у гри

- **Status:** Accepted
- **Date:** 2026-10-17 (оновлено 2026-10-18)

## Context

Оверлей висить поверх гри. Якщо він хоч раз стане активним вікном, гра втратить фокус (і input). Крім того, гра або лаунчер теж можуть ставити собі TOPMOST і «перебити» оверлей.

- `focus: false` діє лише в момент створення вікна, тому його недостатньо.
- `show()` / `hide()` можуть активувати вікно.
- Клацати по оверлею неможливо (він click-through), тож керування — лише hotkeys (і згодом трей).

## Decision

**Конфіг вікна** (`src-tauri/tauri.conf.json`, вікно `overlay`):

- `focus: false` + `focusable: false`, `alwaysOnTop: true`, `transparent: true`, `decorations: false`, `skipTaskbar: true`, `shadow: false`, `noRedirectionBitmap: true`.
- Обидва ключі `focusable` і `noRedirectionBitmap` є в схемі Tauri 2.12, тому вікна створюються прямо з конфігу і `WebviewWindowBuilder` не потрібен. У `setup` лише перевіряємо, що вікно `overlay` існує, і пишемо в лог, якщо ні.
- Для `focusable: false` Tauri (через бібліотеку tao) сам ставить розширений стиль `WS_EX_NOACTIVATE`.

**Click-through:** оверлей викликає `setIgnoreCursorEvents(true)`, тож кліки проходять у гру.

**Оверлей ніколи не показуємо і не ховаємо.** Hotkeys лише міняють вміст картки (подія `cycle-phrase`), без `show` / `hide` / `focus`. «Сховати» (Ctrl+Shift+F11) — це `opacity: 0` і порожній DOM, а не `hide()` ([0014](0014-overlay-controls-modes-and-layout-state.md)).

**Click-through вимикається лише в режимі редагування** (Ctrl+Shift+F12) і повертається при виході або автоматично через 60 с без взаємодії ([0014](0014-overlay-controls-modes-and-layout-state.md)). Перетягування не використовує `startDragging()` ([0016](0016-edit-mode-drag-noactivate-window.md)). Зміни розміру й позиції йдуть через `SetWindowPos` з `SWP_NOACTIVATE` ([0015](0015-overlay-geometry-pure-rust.md)).

**Переутвердження topmost** (`reassertTopmost` у `src/overlay/main.tsx`) — при старті, кожні 5 с (`TOPMOST_INTERVAL_MS`) і щоразу, коли оверлей раптом отримав фокус:

1. `getCurrentWindow().setAlwaysOnTop(true)` з JS;
2. Rust-команда `reassert_topmost`. На Windows вона:
   - читає `GWL_EXSTYLE`; якщо `WS_EX_NOACTIVATE` нема — додає його через `SetWindowLongPtrW` (фолбек на випадок, якщо tao його не поставив);
   - викликає `SetWindowPos(HWND_TOPMOST, SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE)`: ставить вікно на верх TOPMOST-шару, не активуючи, не рухаючи й не змінюючи розмір;
   - пише в лог, чи `WS_EX_NOACTIVATE` уже був, але лише при першому виклику і при зміні значення ([0010](0010-debug-log.md)). Очікуємо `true`. Якщо в лозі `false`, стиль додали ми, і це варто записати в `docs/spike-results.md`.
   - `unsafe` тут виправданий: це виклики Win32 C API на `hwnd` вікна, що належить нашому процесу.
   - На macOS/Linux команда нічого не робить і повертає `Ok(false)`: оверлей над іграми потрібен лише на Windows.

**Зміни фокусу логуються** (`onFocusChanged`): оверлей не повинен отримувати фокус, а якщо отримав — одразу переутверджуємо topmost.

**Вікно налаштувань** — окреме вікно `settings`, яке стартує прихованим (`visible: false`):

- його відкриває лише дія користувача (hotkey Ctrl+Shift+F8 або перший запуск без токена), і воно **свідомо** бере фокус (`show`, `unminimize`, `set_focus`). Оверлей при цьому не чіпаємо;
- хрестик його **ховає**, а не закриває (`CloseRequested` → `prevent_close()` + `hide()`). Інакше вікно знищиться, і hotkey не буде що показувати;
- поки трею нема (крок 3), у налаштуваннях є кнопка «Вийти з програми» (команда `quit_app`).

## Consequences

- Гра не втрачає фокус ні при старті, ні при зміні фрази.
- Періодичний `SetWindowPos` коштує дешево, зате повертає оверлей наверх, якщо гра його перекрила.
- Після Ctrl+Shift+F8 фокус перейде у вікно налаштувань — так задумано.
- Поведінку можна перевірити лише на Windows (результати — у `docs/spike-results.md`). Тести в Hearthstone, Diablo 4, CS2 і Deadlock: фокус не губиться.
- У режимі редагування гра може втратити фокус лише після кліку користувача по картці ([0016](0016-edit-mode-drag-noactivate-window.md)).

## References

- `.omc/plans/word-overlay-plan.md`, §2 (пункти 3, 4, 6) і крок 1
- `docs/spike-results.md`
