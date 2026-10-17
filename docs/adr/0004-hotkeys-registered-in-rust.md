# ADR 0004: Глобальні hotkeys реєструються в Rust

- **Status:** Accepted
- **Date:** 2026-10-17 (оновлено 2026-10-18)

## Context

Оверлей click-through і не бере фокус, тож керувати ним можна лише глобальними hotkeys. `Ctrl+Alt+стрілки` на Intel-драйверах повертають екран, тому такі комбінації не підходять.

## Decision

- Hotkeys реєструє Rust через `tauri-plugin-global-shortcut` у `setup` (`src-tauri/src/lib.rs`):
  - **Ctrl+Shift+F7** (`TRANSLATION_SHORTCUT`) — показати/приховати переклад поточної картки ([0019](0019-training-modes-auto-advance-and-hidden-translation.md));
  - **Ctrl+Shift+F9** (`CYCLE_SHORTCUT`) — наступна фраза (і перезапуск таймера автоперемикання);
  - **Ctrl+Shift+F8** (`SETTINGS_SHORTCUT`) — відкрити вікно налаштувань;
  - **Ctrl+Shift+F10** (`EXPAND_SHORTCUT`) — компактна / розгорнута картка;
  - **Ctrl+Shift+F11** (`VISIBLE_SHORTCUT`) — сховати / показати оверлей;
  - **Ctrl+Shift+F12** (`EDIT_SHORTCUT`) — режим редагування (перетягнути, змінити розмір).
- Обробник явно перевіряє кожну комбінацію (`shortcut.matches(Ctrl+Shift, F7..F12)`); невідома комбінація нічого не робить.
- F10 і F11 спершу міняють стан у store (Rust — джерело правди), а потім шлють нове значення оверлею (`overlay-expanded`, `overlay-visible`). F12 шле `toggle-edit`. Деталі — [0014](0014-overlay-controls-modes-and-layout-state.md).
- Обробник реагує лише на `ShortcutState::Pressed`, а не на відпускання.
- Усі події йдуть через `emit_to("overlay", ...)` (хелпер `emit_overlay`), тобто **лише** вікну `overlay`; помилка emit пишеться в лог. Вікно не показується й не фокусується.
- Ctrl+Shift+F8 — дія користувача, тому відкрите вікно settings бере фокус (див. [0002](0002-overlay-window-no-focus.md)).
- Якщо комбінація зайнята іншою програмою і реєстрація не вдалася, пишемо це в лог і **не панікуємо**: застосунок працює далі.
- В оверлеї індекс фрази росте без меж, а картку беремо як `index % cards.length`. Лог `cycle-phrase` пишеться в окремому `useEffect` на `[index]`, а не в updater-і `setIndex`, бо в StrictMode updater викликається двічі.

## Consequences

- Hotkeys працюють, навіть коли фокус у грі.
- Зайнята комбінація видна лише в `overlay-debug.log`. Налаштовувані hotkeys і показ помилки в settings — крок 3.
- Таблиця комбінацій є у вікні налаштувань (розділ «4. Оверлей») і в README.
- План пропонував F12 для «паузи в матчі». Цю роль тепер виконує F11 (сховати), а F12 зайнято режимом редагування.

## References

- `.omc/plans/word-overlay-plan.md`, §2 пункт 7 і крок 3
