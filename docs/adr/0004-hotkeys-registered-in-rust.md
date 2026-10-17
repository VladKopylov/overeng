# ADR 0004: Глобальні hotkeys реєструються в Rust

- **Status:** Accepted
- **Date:** 2026-10-17

## Context

Оверлей click-through і не бере фокус, тож керувати ним можна лише глобальними hotkeys. `Ctrl+Alt+стрілки` на Intel-драйверах повертають екран, тому такі комбінації не підходять.

## Decision

- Hotkeys реєструє Rust через `tauri-plugin-global-shortcut` у `setup` (`src-tauri/src/lib.rs`):
  - **Ctrl+Shift+F9** (`CYCLE_SHORTCUT`) — наступна фраза;
  - **Ctrl+Shift+F8** (`SETTINGS_SHORTCUT`) — відкрити вікно налаштувань.
- Обробник реагує лише на `ShortcutState::Pressed`, а не на відпускання.
- Ctrl+Shift+F9 шле подію `cycle-phrase` через `emit_to("overlay", ...)`, тобто **лише** вікну `overlay`. Вікно не показується й не фокусується, міняється тільки текст.
- Ctrl+Shift+F8 — дія користувача, тому відкрите вікно settings бере фокус (див. [0002](0002-overlay-window-no-focus.md)).
- Якщо комбінація зайнята іншою програмою і реєстрація не вдалася, пишемо це в лог і **не панікуємо**: застосунок працює далі.
- В оверлеї індекс фрази росте без меж, а картку беремо як `index % cards.length`. Лог `cycle-phrase` пишеться в окремому `useEffect` на `[index]`, а не в updater-і `setIndex`, бо в StrictMode updater викликається двічі.

## Consequences

- Hotkeys працюють, навіть коли фокус у грі.
- Зайнята комбінація видна лише в `overlay-debug.log`. Налаштовувані hotkeys і показ помилки в settings — крок 3.
- Підказка з комбінаціями є у футері вікна налаштувань.

## References

- `.omc/plans/word-overlay-plan.md`, §2 пункт 7 і крок 3
