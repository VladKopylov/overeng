# ADR 0011: Відписка від подій Tauri під React StrictMode

- **Status:** Accepted
- **Date:** 2026-10-17

## Context

`listen()` і `onFocusChanged()` з `@tauri-apps/api` асинхронні: функція відписки (`unlisten`) приходить у `Promise`. React StrictMode у dev монтує компонент двічі, тож cleanup ефекту може виконатися раніше, ніж повернеться `unlisten`. Без додаткової логіки лишаються подвійні обробники.

## Decision

У `useEffect` оверлея (`src/overlay/main.tsx`):

- `cleanups: Array<() => void>` збирає всі відписки (включно з `clearInterval` для таймера topmost);
- прапорець `let disposed = false`, який cleanup ставить у `true`;
- `keep(unlisten)`: якщо компонент уже демонтовано (`disposed`), **одразу** викликає `unlisten()`, інакше додає його в `cleanups`. Кожен `listen(...)` і `onFocusChanged(...)` завершується `.then(keep)`;
- асинхронні результати (`loadCards`) після `disposed` ігноруються.

Пов'язане: лог `cycle-phrase` пишемо в окремому ефекті на `[index]`, а не в updater-і `setIndex`, бо updater у StrictMode викликається двічі.

## Consequences

- У dev немає подвійних обробників і подвійних записів у лог; у production поведінка та сама.
- Патерн треба повторювати для кожного нового асинхронного listener-а.
