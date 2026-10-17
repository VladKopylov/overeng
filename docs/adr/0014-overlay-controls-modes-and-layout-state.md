# ADR 0014: Керування оверлеєм: компактний/розгорнутий режим, сховати/показати, режим редагування

- **Status:** Accepted
- **Date:** 2026-10-18

## Context

Після спайка (див. `docs/spike-results.md`) користувачу потрібно: бачити всі приклади, опис і синонім картки; швидко ховати оверлей; переносити картку й міняти її розмір. Оверлей click-through і не бере фокус ([0002](0002-overlay-window-no-focus.md)), тому все керування — через hotkeys ([0004](0004-hotkeys-registered-in-rust.md)). Стан розкладки (видимість, режим, позиція, розміри, кут) має переживати перезапуск, але JS не має прав `store:*` ([0005](0005-notion-token-rust-only.md)).

## Decision

**Hotkeys і події** (усі в Rust, усі через `emit_to("overlay", ...)`):

| Hotkey | Що робить Rust | Подія → оверлей |
|---|---|---|
| Ctrl+Shift+F10 | перемикає `layout.expanded` у store | `overlay-expanded` (payload: нове значення) |
| Ctrl+Shift+F11 | перемикає `layout.visible` у store | `overlay-visible` (payload: нове значення) |
| Ctrl+Shift+F12 | нічого не зберігає | `toggle-edit` |

- Rust — **джерело правди** для `visible` і `expanded`: він сам міняє store і надсилає нове значення. Оверлей лише застосовує його, тож стан не розходиться між Rust і JS і зберігається між перезапусками.
- Режим редагування не зберігається: після перезапуску оверлей завжди click-through.

**Стан розкладки** — окремий ключ `layout` у `settings.json` (структура `Layout` у `src-tauri/src/layout.rs`), а не поле `Settings`: інакше `set_settings` з вікна налаштувань перезаписував би позицію, збережену оверлеєм. Поля:

- `visible`, `expanded`, `corner` (`top_left` / `top_right` / `bottom_left` / `bottom_right`, за замовчуванням `bottom_right`);
- `hide_translation` — ховати переклад за замовчуванням ([0019](0019-training-modes-auto-advance-and-hidden-translation.md));
- `position` — останній збережений outer rect вікна в physical px (x, y, ширина, висота);
- `compact_size` і `expanded_size` — розміри для кожного режиму окремо.
- `#[serde(default)]`: старий або неповний JSON дає значення за замовчуванням.
- Ідентифікатор монітора не зберігаємо: монітор визначається за самим прямокутником ([0015](0015-overlay-geometry-pure-rust.md)).
- Доступ лише через Rust-команди: `get_layout`, `overlay_rect`, `save_overlay_rect`, `set_corner`, `reset_overlay_position`. Читання-зміна-запис іде через `settings::update_layout`; синхронні команди й обробник hotkeys виконуються в головному потоці, тож записи не перетинаються.

**Сховати/показати (F11)** — лише вміст: обгортка `.stage` отримує `opacity: 0`, а картка не рендериться взагалі. `hide()` / `show()` вікна **ніколи** не викликаються, бо можуть активувати вікно ([0002](0002-overlay-window-no-focus.md)). Click-through і topmost при цьому лишаються як були, тож невидиме вікно не перехоплює кліки.

**Компактний/розгорнутий режим (F10)**:

- розгорнута картка показує фразу, вимову, переклад, синонім, опис і **всі** приклади (Example + More examples), мітку «Mistake fix» — верстка в [0012](0012-overlay-card-layout.md);
- новий розмір і позицію рахує Rust-команда `overlay_rect` (з `fromCurrent: true`), оверлей застосовує їх через `setSize` + `setPosition` у physical px ([0015](0015-overlay-geometry-pure-rust.md));
- якщо F10 натиснуто в режимі редагування, спершу зберігається поточний rect для попереднього режиму, щоб не загубити щойно змінений розмір.

**Одна черга для асинхронних дій** (`serial` у `src/overlay/main.tsx`): кожен обробник F10, F12, `layout-reset`, автовихід через 60 с і стартове розміщення ставиться в один ланцюжок промісів і виконується лише після попереднього. `placeOverlay` повертає свій проміс, тож задача чекає й на розміщення. Без цього дві швидкі F10 у режимі редагування могли б зберегти розмір вікна посеред зміни і записати його не тому режиму. Помилка задачі пишеться в лог і не рве чергу.

**Порядок старту**: спершу підписка на `overlay-expanded` і `overlay-visible`, і лише після неї (`Promise.allSettled(...).finally`) — `get_layout` і перше розміщення. Це той самий принцип, що й для `cards-updated` ([0009](0009-cards-cache-and-sync-guard.md)): F10/F11, натиснуті під час старту, не губляться.

**Режим редагування (F12)**:

- вхід: `setIgnoreCursorEvents(false)`, `setResizable(true)`, рамка `.card.editing` (пунктир кольору акценту) і підказка «Перетягніть картку. Ctrl+Shift+F12 — готово»;
- перетягування — власне, на pointer events, а не `startDragging()` (чому — [0016](0016-edit-mode-drag-noactivate-window.md)); зміна розміру — нативна, за краї вікна;
- вихід (F12 ще раз): `setResizable(false)`, `save_overlay_rect({ expanded })` (Rust сам читає `outer_position` / `outer_size` вікна й зберігає `position` і розмір поточного режиму), а потім **у `finally`** `setIgnoreCursorEvents(true)`: click-through повертається навіть якщо збереження не вдалося;
- перед збереженням скасовується запланований кадр перетягування (`cancelAnimationFrame`) і скидається `pending`, а колбек кадру додатково перевіряє, що режим редагування ще триває. Тож жоден `setPosition` не приходить після збереження чи після повернення click-through;
- **автовихід через 60 с без взаємодії** (`EDIT_IDLE_MS`) з тим самим збереженням. Таймер скидають `pointerdown` / `pointermove` по картці та події вікна `onMoved` / `onResized` (під час нативного resize webview не отримує pointer events). Таймер спершу вмикає click-through напряму (`setIgnoreCursorEvents(true)` поза чергою `serial`), а вже потім ставить у чергу вихід зі збереженням — тож навіть якщо попередня задача в черзі зависла на IPC, click-through не може випадково лишитися вимкненим під час гри;
- поки триває редагування, картку видно навіть якщо оверлей схований (F11): інакше невидиме вікно без click-through перехоплювало б кліки. Після виходу з редагування схований стан повертається;
- гра втрачає фокус лише тоді, коли користувач сам клікає по картці в режимі редагування (див. [0016](0016-edit-mode-drag-noactivate-window.md)). Сам F12 фокус не чіпає.

**Вікно налаштувань**: розділ «4. Оверлей» — вибір кута (4 кути) і кнопка «Скинути позицію», плюс таблиця hotkeys.

- `set_corner` зберігає кут і очищає `position` (вибір кута означає «постав туди»), розміри лишаються;
- `reset_overlay_position` очищає `position`, `compact_size` і `expanded_size`;
- обидві команди шлють `layout-reset` оверлею, і він перераховує позицію (`overlay_rect` з `fromCurrent: false`);
- якщо `layout-reset` прийшов у режимі редагування, оверлей спершу виходить з нього **без збереження** (`leaveEdit(..., false)`), бо інакше збереження поточного rect одразу скасувало б скидання. Click-through при цьому повертається як завжди.

**Права оверлея** (`capabilities/overlay.json`): додано `allow-set-size`, `allow-set-resizable`, `allow-outer-position` (початкова точка перетягування). `allow-primary-monitor` і `allow-outer-size` прибрано: монітори й розмір вікна тепер читає Rust. `allow-start-dragging` не потрібен. Прав `store:*` як не було, так і нема.

## Consequences

- Стан розкладки переживає перезапуск і не залежить від JS.
- Позиція зберігається лише при виході з режиму редагування (вручну, автоматично або перед F10 у режимі редагування), а не на кожен рух.
- Будь-яка зміна вікна в режимі редагування, як і раніше, йде через `SetWindowPos` з `SWP_NOACTIVATE` ([0015](0015-overlay-geometry-pure-rust.md)).
- Поведінку режиму редагування в іграх ще треба перевірити на Windows (`docs/spike-results.md`).

## References

- `.omc/plans/word-overlay-plan.md`, §2 пункт 3, крок 3
- `src-tauri/src/layout.rs`, `src-tauri/src/lib.rs`, `src/overlay/main.tsx`, `src/settings/main.tsx`
