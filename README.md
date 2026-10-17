# Word Overlay

Маленьке вікно поверх ігор у правому нижньому куті: фраза англійською + вимова + переклад.
Стек: Tauri 2 (Rust) + React + TypeScript + Vite.

**Поточний стан: крок 3 — керування оверлеєм** (розгорнута картка, сховати/показати, перетягування й розмір).
Картки беруться з вашої бази Notion (кеш `cards.json`); поки синку не було — показуються 3 вбудовані фрази.

## Структура

```
overlay.html, settings.html   # дві сторінки Vite (multi-page, без роутера)
src/overlay/main.tsx          # картка-оверлей: click-through, topmost, режими, перетягування, картки з get_cards
src/settings/main.tsx         # вікно налаштувань: токен, база, мапінг колонок, «Синхронізувати»
src-tauri/src/lib.rs          # Rust: команди, hotkeys Ctrl+Shift+F8–F12, автосинк при старті
src-tauri/src/layout.rs       # геометрія оверлея (кут, якір, притискання до екрана) + тести
src-tauri/src/settings.rs     # налаштування й токен (tauri-plugin-store, лише з Rust), кеш cards.json
src-tauri/src/notion/         # синк Notion: parse.rs, sync.rs, client.rs, error.rs + синтетичні fixtures/
src-tauri/tauri.conf.json     # конфіг вікон overlay і settings
src-tauri/capabilities/       # права для кожного вікна окремо
docs/spike-results.md         # таблиця результатів тесту в іграх + чек-лист
app-icon.png                  # вихідна іконка (npx tauri icon app-icon.png)
```

Архітектурні рішення (ADR) — у [docs/adr/](docs/adr/README.md).

## Налаштування Windows ПК (один раз)

1. **Visual Studio Build Tools** — https://visualstudio.microsoft.com/visual-cpp-build-tools/
   В інсталяторі поставити галочку **«Desktop development with C++»** (MSVC + Windows SDK).
2. **Rust** — https://rustup.rs → `rustup-init.exe`, варіант за замовчуванням (`x86_64-pc-windows-msvc`).
   Перевірка: `rustc --version`.
3. **WebView2** — вже є у Windows 10/11. Якщо ні: https://developer.microsoft.com/microsoft-edge/webview2/ (Evergreen Bootstrapper).
4. **Node.js LTS** (22 або 24) — https://nodejs.org. Перевірка: `node -v`, `npm -v`.
5. **Git** — https://git-scm.com.
6. Клонувати репо: `git clone <url> word-overlay && cd word-overlay`.

## Цикл розробки на Windows

```powershell
git pull
npm ci
npm run tauri dev
```

Перший запуск компілює Rust кілька хвилин; далі — швидко. Зміни у `src/` підхоплюються одразу (hot reload),
зміни в `src-tauri/` — перекомпіляцією автоматично.

## Тести

```bash
cd src-tauri
cargo test          # без мережі: парсер, пагінація, 429-retry, extract_id, фільтр, кеш, геометрія оверлея
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Фікстури (`src-tauri/src/notion/fixtures/*.json`) — синтетичні; реальні дані з Notion не комітимо
(`tests/fixtures/real/` в `.gitignore`).

## Розробка на Mac

```bash
npm install
npm run tauri dev
```

На Mac оверлей видно, але поведінку над іграми (фокус, fullscreen, topmost) можна перевірити **лише на Windows**.

## Що робить спайк (крок 1)

- Вікно `overlay`: прозоре, без рамки, поверх усіх, без іконки в taskbar, **не бере фокус** (`focusable: false`),
  кліки проходять крізь нього (`setIgnoreCursorEvents(true)`).
- Ставиться в правий нижній кут робочої області основного монітора (без taskbar), у фізичних пікселях.
- Кожні 5 с повторно стає «поверх усіх»: `setAlwaysOnTop(true)` з JS + Rust-команда `reassert_topmost`
  (на Windows — `SetWindowPos(HWND_TOPMOST, SWP_NOACTIVATE|SWP_NOMOVE|SWP_NOSIZE)`).
- **Ctrl+Shift+F9** — наступна фраза (вікно не показується/не ховається і не фокусується).

## Hotkeys

| Комбінація | Дія | Фокус |
|---|---|---|
| **Ctrl+Shift+F9** | наступна картка | оверлей не фокусується, гра лишається активною |
| **Ctrl+Shift+F10** | компактна / розгорнута картка (усі приклади, опис, синонім) | гра лишається активною |
| **Ctrl+Shift+F11** | сховати / показати оверлей (стан зберігається між запусками) | гра лишається активною |
| **Ctrl+Shift+F12** | режим редагування: рамка й підказка, перетягніть картку мишею, тягніть за край, щоб змінити розмір; ще раз F12 — готово (позиція й розмір збережуться). Через 60 с без руху режим вимкнеться сам | сам F12 фокус не чіпає; клік по картці може перевести фокус з гри |
| **Ctrl+Shift+F8** | відкрити налаштування | **вікно налаштувань забирає фокус** — так задумано: ви самі його викликали. У грі після цього треба повернутися в гру (Alt+Tab / клік) |

Компактний і розгорнутий режими мають окремі розміри. Розгорнута картка росте всередину екрана: якщо картка в нижній половині, вона росте вгору, а якщо праворуч — вліво. Позиція завжди лишається в межах робочої області монітора, навіть після зміни роздільності.
У налаштуваннях (розділ «4. Оверлей») можна вибрати кут екрана і **«Скинути позицію»** — оверлей повернеться в кут зі стандартним розміром.

### Ігри в exclusive fullscreen

Поверх exclusive fullscreen (CS2, Deadlock у режимі «Fullscreen») оверлей не видно, і це **свідомо**: показати вікно поверх такої гри можна лише ін'єкцією в процес гри, а це ризик бану від античіта (VAC, Vanguard). Перемкніть гру в **Fullscreen Windowed** / Borderless або перетягніть оверлей на **другий монітор** (Ctrl+Shift+F12). Деталі — [ADR 0017](docs/adr/0017-exclusive-fullscreen-unsupported.md), результати тестів — `docs/spike-results.md`.

## Підключення Notion (крок 2)

1. Відкрийте https://www.notion.so/my-integrations → **New integration** → тип **Internal**,
   виберіть свій workspace. У **Capabilities** лишіть тільки **Read content** (зняти Update/Insert content
   і доступ до коментарів/користувачів — застосунку вони не потрібні). Збережіть і скопіюйте
   **Internal Integration Secret** (починається з `ntn_` або `secret_`).
2. Відкрийте базу в Notion (у вас — «English Vocab — Active Use») → **•••** у правому верхньому куті →
   **Connections** → **Add connections** → виберіть свою інтеграцію. Без цього кроку Notion відповідає 404
   і застосунок напише «База не розшарена з інтеграцією».
3. Запустіть застосунок. На першому запуску (токена ще нема) вікно налаштувань відкриється само;
   пізніше — **Ctrl+Shift+F8**.
4. Вставте токен у поле «Токен Notion» → **Зберегти**. Поле write-only: токен назад не показується,
   видно лише «Токен збережено».
5. Вставте посилання на базу (••• → **Copy link**) або її ID → **Знайти**. Якщо в базі кілька
   джерел даних (data sources) — виберіть потрібне у списку.
   Для довідки, ваші ID: database `60b3dc5f-0844-44c2-9f5e-2997b50dcb98`,
   data source `6b0b8ba2-a8cc-470d-854f-8e7435a2cfbf` (підходить будь-який з них).
6. **Синхронізувати.** У рядку статусу з'явиться кількість карток. Оверлей одразу перечитає картки.

Далі синк запускається сам у фоні при кожному старті застосунку (жодне вікно при цьому не відкривається).
Якщо синк не вдався (немає інтернету, 429), лишається попередній кеш.

**Що показується:** фраза, вимова, переклад і перший приклад; у розгорнутому режимі (Ctrl+Shift+F10) — ще синонім, опис і всі приклади. Картки типу **Mistake fix** мають
червону мітку. Картки зі статусом **Mastered** приховано (список статусів змінюється в налаштуваннях,
поле «Не показувати статуси»). Порядок перемішується при кожному завантаженні карток.

**Колонки:** за замовчуванням мапінг під вашу базу — Phrase (title), Translation, Pronunciation,
Description, Example, More examples (по рядку, маркери `•`/`-`/`*` прибираються), Synonym, Situation,
Type, Status, Added, Last reviewed. Інші назви — у розділі «Колонки Notion» у налаштуваннях;
порожнє поле = колонка не використовується. Нові колонки в Notion нічого не ламають.

**Повідомлення про помилки:**

| Повідомлення | Що робити |
|---|---|
| «Невірний токен (401)» | скопіювати секрет інтеграції ще раз |
| «База не розшарена з інтеграцією» | крок 2 вище (••• → Connections) |
| «Колонки X не знайдено… Доступні: …» | виправити назву в «Колонки Notion» або очистити поле |
| «Notion тимчасово обмежив запити (429)» / «Немає зв'язку» | спробувати пізніше, кеш збережено |

### Де лежать файли

| Файл | Windows | macOS |
|---|---|---|
| Налаштування + токен + позиція оверлея (`settings.json`) | `%APPDATA%\com.vlad.wordoverlay\settings.json` | `~/Library/Application Support/com.vlad.wordoverlay/settings.json` |
| Кеш карток (`cards.json`) | `%APPDATA%\com.vlad.wordoverlay\cards.json` | `~/Library/Application Support/com.vlad.wordoverlay/cards.json` |

Токен зберігається у `settings.json` відкритим текстом (зашифроване сховище — пізніше). Тому інтеграція
має лише право читання і доступ лише до однієї бази. Щоб скинути все — закрийте застосунок і видаліть ці файли.
У лог токен ніколи не пишеться.

### Вікно налаштувань

- Хрестик **ховає** вікно, а не закриває програму. Вийти — кнопка «Вийти з програми» внизу
  (трей із пунктом Quit з'явиться в кроці 3).
- Налаштування доступні лише через Rust-команди; JS не має прав `store:*`.

## Лог

Файл `overlay-debug.log` (старт, позиція, hotkey, topmost, зміни фокусу, синк: старт/кількість/помилки —
з часовими мітками; токен туди не пишеться):

- Windows: `%LOCALAPPDATA%\com.vlad.wordoverlay\logs\overlay-debug.log`
- macOS: `~/Library/Logs/com.vlad.wordoverlay/overlay-debug.log`

Ті самі рядки видно в терміналі, де запущено `npm run tauri dev`.

## WS_EX_NOACTIVATE (крок 1d)

Стиль Windows `WS_EX_NOACTIVATE` не дає вікну забирати фокус. Tauri ставить його сам, бо в конфігу
`focusable: false`. Команда `reassert_topmost` кожні 5 с перевіряє стиль і, якщо його нема, додає.
У лозі рядок `reassert_topmost ok (WS_EX_NOACTIVATE вже був: true)` означає, що Tauri його поставив;
`false` — що ми його додали (запишіть це в `docs/spike-results.md`). Нічого перемикати не треба.

## Інсталятор через GitHub Actions

`.github/workflows/windows-build.yml` збирає `.msi` і `.exe` на `windows-latest` при push у `main`
(або вручну: вкладка **Actions → windows-build → Run workflow**). Файли — в artifact `word-overlay-windows`.
Інсталятор не підписаний: Windows SmartScreen покаже попередження → «Детальніше» → «Все одно запустити».
