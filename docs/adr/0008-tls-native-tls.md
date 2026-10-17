# ADR 0008: TLS через native-tls

- **Status:** Accepted
- **Date:** 2026-10-17

## Context

`reqwest` потребує TLS-бекенд. Варіант за замовчуванням з rustls використовує `aws-lc` (`aws-lc-rs`), який компілюється з C і на Windows потребує додаткових інструментів (NASM тощо). Це ускладнює збірку на ігровому ПК і в CI.

## Decision

`reqwest` підключається з `default-features = false` і features `["json", "native-tls"]` (`src-tauri/Cargo.toml`). TLS — системний: SChannel у Windows, Security.framework у macOS. Нічого не компілюється з C, NASM і OpenSSL не потрібні.

## Consequences

- Простіша й швидша збірка на Windows і в GitHub Actions.
- Поведінка TLS (сертифікати, протоколи) залежить від ОС.

## References

- `.omc/plans/word-overlay-plan.md`, крок 2
