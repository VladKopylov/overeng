# ADR 0015: Геометрія оверлея — чиста функція в Rust

- **Status:** Accepted
- **Date:** 2026-10-18

## Context

Розмір картки тепер змінюється (компактний ↔ розгорнутий, ручний resize), а позиція зберігається й відновлюється. Потрібно, щоб:

- вікно росло **всередину екрана**, а не за край;
- збережена позиція після зміни роздільності чи відключення монітора не опинилася поза екраном;
- логіку можна було перевірити тестами без Windows.

## Decision

**Чисті функції** в `src-tauri/src/layout.rs`, усе в physical px (`Rect { x, y, width, height }`, `i32`):

- `clamp_into(work, rect)` — спершу обрізає ширину й висоту до робочої області, потім зсуває rect усередину неї;
- `resize_anchored(work, current, size)` — **правило якоря**: якщо центр поточного вікна в нижній половині робочої області, фіксуємо нижній край (вікно росте вгору), інакше — верхній. Так само по горизонталі: центр у правій половині → фіксуємо правий край, інакше лівий. Результат проходить через `clamp_into`. Центр порівнюємо в подвоєних координатах (`2x + w`), щоб обійтися без дробів;
- `corner_rect(work, size, corner, margin)` — розміщення в куті з відступом;
- `pick_monitor(monitors, rect)` — монітор, у робочій області якого лежить центр rect; якщо такого нема — той, з яким найбільше перетину; інакше `None`;
- `mode_size(layout, scale)` — збережений розмір поточного режиму або стандартний (компактний 380×200, розгорнутий 460×380 logical × `scaleFactor`), не менший за 180×90 logical. Збережений розмір (`SavedSize`) містить і `scale` монітора, на якому його зберегли (`save_overlay_rect` бере `window.scale_factor()`). При розміщенні на моніторі з іншим масштабом розмір перераховується: `width × scale / saved_scale`. Отже, картка має однаковий **логічний** розмір на будь-якому моніторі. Старий JSON без `scale` (`#[serde(default)]`) використовується як є;
- **Захист від нереальних значень**:
  - `Rect::right()` / `bottom()` рахуються через `saturating_add`, `resize_anchored` і `corner_rect` — через `saturating_sub`;
  - `clamp_into` рахує в `i64` і одразу повертає саму робочу область, якщо в неї ширина або висота ≤ 0. Інакше `i32::clamp(min, max)` з `min > max` панікував би;
  - `Layout::sanitize()` відкидає збережену позицію, якщо |x| або |y| > 1 000 000 чи сторона поза 1..=100 000, і розмір з такою стороною або з `scale`, що не є додатним скінченним числом. `settings::load_layout` викликає його й пише в лог, якщо щось відкинуто. Якщо ключ `layout` у `settings.json` взагалі не розбирається, у лог іде помилка serde, і береться стандартний `Layout` (раніше це відбувалося мовчки);
- `target_rect(monitors, primary, layout, current)` — збирає все докупи:
  - база = `current` (перемикання F10) або збережена `position` (старт, скидання);
  - є база → монітор через `pick_monitor`, а якщо монітора вже нема — основний; далі `resize_anchored` під розмір режиму. Отже, позиція після зміни роздільності чи монітора **притискається** всередину поточної робочої області;
  - бази нема → `corner_rect` на основному моніторі з вибраним кутом і відступом 16 px × `scaleFactor`.
- Тести (`cargo test`, модуль `layout::tests`): clamp з усіх боків і зі зміщеною робочою областю, ріст і зменшення в кожному куті, повернення compact → expanded → compact у той самий rect, притискання біля краю, кути, вибір монітора, зміна роздільності, зниклий монітор, мінімальний розмір, скидання, старий JSON, порожня робоча область, екстремальні координати (`i32::MAX` / `i32::MIN`), `sanitize`, перерахунок розміру під масштаб монітора, розмір без `scale`.
- У `tauri.conf.json` вікно `overlay` має `minWidth: 180`, `minHeight: 90` (logical), як і `MIN_LOGICAL`: нативний resize у режимі редагування не дасть зробити картку меншою.

**Хто що робить**:

- Rust-команда `overlay_rect(fromCurrent)` сама читає монітори (`available_monitors`, `primary_monitor`, `Monitor::work_area`, `scale_factor`) і поточний outer rect вікна, викликає `target_rect` і пише вхід і результат у лог. Тому JS не потрібні права `allow-current-monitor` / `allow-available-monitors` / `allow-primary-monitor`;
- JS лише застосовує результат у порядку `setPosition` → `setSize` → `setPosition` (ті самі physical значення). Чому так — див. Consequences про DPI;
- усі операції з геометрією й режимом редагування в оверлеї йдуть через одну чергу промісів (`serial` у `src/overlay/main.tsx`), тож дві швидкі F10 не перемішують «зберегти розмір» і «поставити новий розмір» ([0014](0014-overlay-controls-modes-and-layout-state.md));
- topmost-переутвердження ([0002](0002-overlay-window-no-focus.md)) лишається без змін.

**Чому `setSize` / `setPosition` безпечні для фокусу** (перевірено в `~/.cargo/registry/.../tao-0.37.1/src/platform_impl/windows/`, версія з `Cargo.lock`):

- `setPosition` → `Window::set_outer_position` (`window.rs`, ~рядок 193): `SetWindowPos(..., SWP_ASYNCWINDOWPOS | SWP_NOZORDER | SWP_NOSIZE | SWP_NOACTIVATE)`;
- `setSize` → `Window::set_inner_size` → `util::set_inner_size_physical` (`util.rs`, ~рядок 91): `SetWindowPos(..., SWP_ASYNCWINDOWPOS | SWP_NOZORDER | SWP_NOREPOSITION | SWP_NOMOVE | SWP_NOACTIVATE)`;
- `setResizable` і `setIgnoreCursorEvents` → `WindowState::set_window_flags` (`window_state.rs`, ~рядки 412–443): нові стилі + `SetWindowPos(SWP_NOZORDER | SWP_NOMOVE | SWP_NOSIZE | SWP_FRAMECHANGED)` і `SWP_NOACTIVATE`, якщо вікно не fullscreen (наше — ні);
- `WS_EX_NOACTIVATE` при цьому не губиться: `to_window_styles` додає його щоразу, коли нема прапорця `FOCUSABLE` (`window_state.rs`, ~рядок 293).
- `setSize` задає inner size, а ми зберігаємо outer. Для вікна без рамки й тіні (`decorations: false`, `shadow: false`) вони однакові: `WM_NCCALCSIZE` у tao прибирає неклієнтську область.

## Consequences

- Вся математика позиції покрита unit-тестами й не залежить від Windows.
- Кожна зміна геометрії коштує один IPC-виклик до Rust і два до вікна.
- **DPI і перехід між моніторами.** Коли `setPosition` переносить вікно на монітор з іншим масштабом, Windows шле `WM_DPICHANGED`, і tao (`event_loop.rs`, ~рядки 1837–2060) сам масштабує розмір вікна під новий DPI. Якщо спершу поставити розмір, а потім позицію, розмір після переходу роздувався б (або зменшувався) у `new_dpi / old_dpi` разів, а кожне збереження закріплювало б це й накопичувало. Тому порядок такий: `setPosition` (вікно вже на цільовому моніторі, DPI-масштабування відбулося) → `setSize` (точний physical розмір для цього монітора) → `setPosition` ще раз (ті самі координати, на випадок якщо tao зсунув вікно під час DPI-зміни). Другий `setPosition` уже не міняє монітор, тож нового `WM_DPICHANGED` нема.
- При перетягуванні між моніторами з різним масштабом (режим редагування) tao так само масштабує розмір. Це очікувано: збережений розмір містить `scale`, тож при наступному розміщенні він перераховується коректно, а не накопичує помилку.
- Вікно на мить може мати проміжний розмір або позицію (один-два кадри). Для оверлея це прийнятно.
- [0003](0003-overlay-positioning-physical-pixels.md) лишається чинним щодо одиниць (physical px), але формулу кута тепер рахує Rust.

## References

- `src-tauri/src/layout.rs`, `src-tauri/src/lib.rs` (`overlay_rect`, `save_overlay_rect`)
- tao 0.37.1: `src/platform_impl/windows/window.rs`, `util.rs`, `window_state.rs`
