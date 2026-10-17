# ADR 0017: Exclusive fullscreen не підтримується — свідомо

- **Status:** Accepted
- **Date:** 2026-10-18

## Context

Тести на Windows (`docs/spike-results.md`) показали:

- Hearthstone і Diablo 4 у своєму звичайному fullscreen — оверлей видно;
- CS2 і Deadlock в **exclusive fullscreen** — оверлей не видно; у режимі «Fullscreen Windowed» — працює;
- фокус гри не губиться в жодному з режимів.

В exclusive fullscreen гра сама володіє виводом на екран, і звичайне вікно (навіть `HWND_TOPMOST`) поверх неї не малюється. Показати щось поверх такої гри можна лише ін'єкцією в процес гри (хук DirectX/Vulkan `Present`, як у Steam / Discord overlay). Для стороннього застосунку це те, що ловлять античіти: VAC (CS2, Deadlock), Vanguard (Valorant), Warden (WoW) — з ризиком бану акаунта.

## Decision

- Exclusive fullscreen **не підтримується за задумом**. Жодних хуків, ін'єкцій у процес гри чи читання пам'яті гри.
- Оверлей лишається звичайним вікном Windows: `HWND_TOPMOST`, `WS_EX_NOACTIVATE`, click-through ([0002](0002-overlay-window-no-focus.md)).
- Рекомендація користувачу (README):
  1. перемкнути гру в **Fullscreen Windowed** / Borderless (назва пункту залежить від гри);
  2. або тримати оверлей на **другому моніторі** (режим редагування, Ctrl+Shift+F12, — перетягнути туди).
- Якщо гра в exclusive fullscreen, оверлей просто не видно. Помилкою це не вважається, і в коді на цей випадок нічого не робимо.

## Consequences

- Нуль ризику бану через оверлей: застосунок не взаємодіє з процесом гри.
- Частина гравців CS2 / Deadlock / Valorant грає в exclusive fullscreen через меншу затримку. Для них лишається другий монітор.
- WoW і Valorant ще не перевірені (`docs/spike-results.md`).

## References

- `docs/spike-results.md`
- `.omc/plans/word-overlay-plan.md`, gate-правило кроку 1
