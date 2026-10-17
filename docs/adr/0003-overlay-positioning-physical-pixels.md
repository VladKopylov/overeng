# ADR 0003: Позиціонування оверлея в physical pixels

- **Status:** Accepted, розрахунок позиції замінено [0015](0015-overlay-geometry-pure-rust.md)
- **Date:** 2026-10-17 (оновлено 2026-10-18)

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

**Оновлення 2026-10-18.** `positionBottomRight()` прибрано. Позицію тепер рахує Rust-команда `overlay_rect` (чиста функція `layout::target_rect`, [0015](0015-overlay-geometry-pure-rust.md)): вибраний кут, збережена позиція, притискання до робочої області, кілька моніторів. Принцип той самий: усе в physical px, `workArea` не множиться на `scaleFactor`, відступ = 16 px × `scaleFactor`, щоб на HiDPI він виглядав так само.

## Consequences

- Позиція коректна на будь-якому DPI без перерахунку.
- Кут вибирається в налаштуваннях; монітор визначається за збереженою позицією ([0015](0015-overlay-geometry-pure-rust.md)).

## References

- `.omc/plans/word-overlay-plan.md`, §2 пункт 5
