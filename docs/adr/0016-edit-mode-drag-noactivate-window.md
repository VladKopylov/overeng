# ADR 0016: Перетягування в режимі редагування на вікні без фокусу

- **Status:** Accepted
- **Date:** 2026-10-18

## Context

У режимі редагування ([0014](0014-overlay-controls-modes-and-layout-state.md)) картку треба перетягувати мишею. Стандартний шлях у Tauri — `getCurrentWindow().startDragging()`. Але оверлей має `focusable: false`, тобто `WS_EX_NOACTIVATE` ([0002](0002-overlay-window-no-focus.md)), і переносити його треба, поки гра на передньому плані. Перевірити на Windows з Mac неможливо, тому висновок зроблено з коду tao та Win32.

## Decision

**Що робить `startDragging()` на Windows** (tao 0.37.1, `src/platform_impl/windows/window.rs`, ~рядки 493–522):

- `drag_window()` → `handle_os_dragging(HTCAPTION)`: `GetCursorPos`, `ReleaseCapture`, прапорець `dragging = true` і `PostMessageW(hwnd, WM_NCLBUTTONDOWN, HTCAPTION, ...)`;
- далі все робить системний `DefWindowProc`: натискання на «заголовок» запускає модальний цикл переміщення (`WM_SYSCOMMAND` / `SC_MOVE`, `WM_ENTERSIZEMOVE` … `WM_EXITSIZEMOVE`). tao лише ловить `WM_EXITSIZEMOVE` і досилає `WM_LBUTTONUP` (`event_loop.rs`, ~рядок 977);
- сам tao нічого не активує, але й не захищає від активації: що зробить `DefWindowProc` з натисканням `HTCAPTION` на вікні з `WS_EX_NOACTIVATE`, залежить від Windows. Для звичайного вікна натискання на заголовок робить його активним (і гра втрачає фокус). Чи пропускає цей шлях активацію для `WS_EX_NOACTIVATE`, у документації Win32 не сказано. Крім того, для таких вікон трапляються скарги, що під час перетягування видно лише рамку, а вікно «стрибає» лише після відпускання.

**Рішення: власне перетягування замість `startDragging()`** (`src/overlay/main.tsx`):

- `pointerdown` по картці (лише в режимі редагування, ліва кнопка): `setPointerCapture`, запам'ятати `screenX/screenY` і `devicePixelRatio`, асинхронно прочитати `outerPosition()` (physical px);
- `pointermove`: нова позиція = початкова + (`screenX − startX`) × `devicePixelRatio`. Не частіше за кадр (`requestAnimationFrame`) викликається `setPosition(new PhysicalPosition(...))`. Це той самий `SetWindowPos` з `SWP_NOACTIVATE`, що й у [0015](0015-overlay-geometry-pure-rust.md);
- кінець жесту (`endDrag`): `pointerup`, `pointercancel`, `lostpointercapture`, а також `pointermove` з відпущеною лівою кнопкою (`(e.buttons & 1) === 0`). Останнє потрібно, бо `pointerup` може загубитися (наприклад, кнопку відпустили, поки системний цикл resize чи інше вікно забрали мишу). Без цього перетягування «прилипало» б до курсора. У лог пишеться кінцева точка. Збереження позиції — при виході з режиму редагування;
- `screenX/screenY` — координати екрана, тож вікно, що рухається під курсором, не збиває розрахунок. Коефіцієнт береться на початку жесту; при переході між моніторами з різним масштабом можливий невеликий зсув, який зникає з наступним жестом.
- Цей шлях не відправляє `WM_NCLBUTTONDOWN` і не запускає модальний цикл, тож вікно ніхто не активує, крім самого кліку (див. нижче).

**Зміна розміру — нативна, через `setResizable(true)`**:

- для вікна без рамки `tauri-runtime-wry 2.12.2` (`src/undecorated_resizing.rs`) при `SetResizable(true)` додає дочірнє вікно `TAURI_DRAG_RESIZE_BORDERS` по краях. Його `WM_NCHITTEST` повертає `HTLEFT` / `HTBOTTOMRIGHT` тощо, а `WM_NCLBUTTONDOWN` пересилає батьківському вікну. При `setResizable(false)` воно видаляється (`detach_resize_handler`);
- це той самий системний модальний цикл, тож ризик активації такий самий, як у `startDragging()`. Ми його приймаємо: розмір міняють рідко, лише свідомим кліком по краю в режимі редагування, коли гра й так не отримує input. Якщо на Windows з'явиться `focus changed: overlay focused=true` після resize, переходимо на власні ручки resize тим самим способом, що й перетягування.

**Фокус і клік по картці**: `WS_EX_NOACTIVATE` означає, що клік по вікну не робить його активним. Проте WebView2 може спробувати взяти фокус клавіатури при кліку у свій вміст. Тому правило таке: у режимі редагування гра може втратити фокус **лише** коли користувач сам клікнув по картці. Сам F12, поява рамки й автовихід фокус не чіпають. Кожна зміна фокусу оверлея пишеться в лог (`focus changed`) і викликає переутвердження topmost.

## Consequences

- Перетягування не залежить від того, як `DefWindowProc` обробляє `HTCAPTION` для вікон без активації.
- Під час перетягування на кожен кадр іде один IPC-виклик `setPosition`. Для одного маленького вікна це дешево.
- Права `core:window:allow-start-dragging` не потрібні.
- На Windows треба перевірити (`docs/spike-results.md`): плавність перетягування, чи не зникає фокус гри після кліку по картці, чи не активує вікно нативний resize.

## References

- tao 0.37.1: `src/platform_impl/windows/window.rs` (`handle_os_dragging`, `drag_window`, `drag_resize_window`), `event_loop.rs` (`WM_EXITSIZEMOVE`, `WM_NCLBUTTONDOWN`)
- tauri-runtime-wry 2.12.2: `src/undecorated_resizing.rs`, `src/lib.rs` (`WindowMessage::SetResizable`)
