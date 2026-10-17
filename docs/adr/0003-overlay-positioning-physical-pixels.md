# ADR 0003: Позиціонування оверлея в physical pixels

- **Status:** Accepted
- **Date:** 2026-10-17

## Context

Оверлей має стояти в правому нижньому куті робочої області (без taskbar) основного монітора. На моніторах із масштабом ≠ 100% легко помилитися з одиницями: у Tauri v2 `Monitor.workArea`, `size` і `position` уже повертаються в physical px.

## Decision

`positionBottomRight()` у `src/overlay/main.tsx`:

- бере монітор через `primaryMonitor()`. Якщо він `null`, пише в лог і нічого не робить;
- використовує `m.workArea` як є, **не множачи на `scaleFactor`**;
- розмір вікна бере через `win.outerSize()`, що теж повертає physical px;
- рахує `x = workArea.x + workArea.width - size.width - MARGIN_PX` (і так само `y`), де `MARGIN_PX = 16` — відступ від краю робочої області в physical px;
- ставить позицію через `setPosition(new PhysicalPosition(x, y))`;
- пише в лог монітор, `scaleFactor`, `workArea`, розмір вікна й результат.

## Consequences

- Позиція коректна на будь-якому DPI без перерахунку.
- Поки що підтримується лише основний монітор; вибір монітора й кута — follow-up.
- Відступ 16 physical px візуально менший на HiDPI-моніторах.

## References

- `.omc/plans/word-overlay-plan.md`, §2 пункт 5
