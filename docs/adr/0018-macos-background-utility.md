# ADR 0018: macOS — фонова утиліта без іконки в Dock, оверлей на всіх робочих столах

- **Status:** Accepted
- **Date:** 2026-10-18

## Context

Окрім ігор на Windows, оверлей потрібен на Mac під час роботи: картка висить у кутку поверх браузера та IDE і час від часу змінюється. Код уже збирався й запускався на macOS, але поведінка кількох речей відрізняється від Windows:

- `transparent: true` на macOS раніше вимагав `macOSPrivateApi`; з Tauri 2.12.1 це завжди увімкнено (tauri-utils `config.rs`: «No-op in Tauri 2.12.1+»). Білого фону під карткою нема.
- `focusable: false`, click-through і `alwaysOnTop` мають рідні реалізації в tao (`set_focusable`, `setIgnoresMouseEvents`, `NSFloatingWindowLevel`).
- Hotkeys ідуть через Carbon `RegisterEventHotKey`, дозвіл Accessibility не потрібен.
- Вікно за замовчуванням належить одному Space: при перемиканні робочого столу (Ctrl+←/→) оверлей лишається на попередньому.
- `skipTaskbar` на macOS ігнорується: застосунок видно в Dock і Cmd+Tab як звичайний, хоча керується він лише hotkeys.
- Windows-специфіка (`reassert_topmost`, `noRedirectionBitmap`) або no-op, або ігнорується ([0002](0002-overlay-window-no-focus.md)).

## Decision

- **`visibleOnAllWorkspaces: true`** для вікна `overlay` у `tauri.conf.json`: tao ставить `NSWindowCollectionBehaviorCanJoinAllSpaces`, і оверлей слідує за користувачем між робочими столами. На Windows і Linux ключ нічого не змінює.
- **`ActivationPolicy::Accessory`** на старті (`app.set_activation_policy`, лише під `cfg(target_os = "macos")`): застосунок не має іконки в Dock і не бере участі в Cmd+Tab. Вікно налаштувань усе одно відкривається й бере фокус по Ctrl+Shift+F8, бо `set_focus` активує застосунок явно.
- **Fullscreen-застосунки (зелена кнопка) не підтримуються.** Це окремий Space, і для показу поверх нього потрібен `NSWindowCollectionBehaviorFullScreenAuxiliary`, якого tao не виставляє. Викликати його напряму через `ns_window()` і objc2 можна, але це `unsafe` заради одного сценарію; поки що робочі вікна варто розгортати без fullscreen-режиму. Якщо сценарій стане важливим, це окремий ADR.
- **F-клавіші.** На клавіатурі MacBook F7–F12 за замовчуванням медіа-клавіші, тому hotkeys треба натискати з Fn або увімкнути System Settings → Keyboard → «Use F1, F2, etc. keys as standard function keys». Самі комбінації не міняємо, щоб README і код були однакові для обох систем.
- Нічого платформозалежного в JS не додаємо: оверлей не знає, на якій системі працює.

## Consequences

- На Mac застосунок живе непомітно: немає Dock-іконки, оверлей є на кожному робочому столі, керування лише hotkeys і Ctrl+Shift+F8.
- Вихід із програми, як і раніше, кнопкою «Вийти з програми» в налаштуваннях: Cmd+Q без активного вікна нікуди не доходить.
- Над fullscreen-Space оверлея не видно, і це задокументоване обмеження, а не помилка.
- Поведінку в іграх це не змінює: Windows-шлях той самий, що й до цього ADR.

## References

- tauri-utils 2.10.2 `src/config.rs` (`macos_private_api`, `visible_on_all_workspaces`), tauri 2.12.4 `src/app.rs` (`set_activation_policy`)
- tao 0.37 `src/platform_impl/macos/window.rs` (`set_focusable`, `set_always_on_top`, `set_visible_on_all_workspaces`)
- `src-tauri/tauri.conf.json`, `src-tauri/src/lib.rs`
